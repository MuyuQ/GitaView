use crate::domain::repo::{RepoRecord, RepoStatusDto};
use crate::domain::status::RemoteRelation;
use crate::git::commands::{branch_state, GitBranchState};
use crate::git::status_text::{change_label, state_hint};
use std::time::Instant;

const STATUS_REFRESH_BATCH_SIZE: usize = 4;

pub fn repo_status_from_branch_result(
    repo: RepoRecord,
    result: Result<GitBranchState, String>,
) -> RepoStatusDto {
    match result {
        Ok(state) => RepoStatusDto {
            id: repo.id,
            name: repo.name,
            path: repo.path.to_string_lossy().to_string(),
            group: repo.group,
            branch: state.branch.clone(),
            relation: state.relation,
            change_label: change_label(&state),
            hint: state_hint(&state),
            has_remote: state.has_remote,
            remote_url: state.remote_url,
        },
        Err(err) => RepoStatusDto {
            id: repo.id,
            name: repo.name,
            path: repo.path.to_string_lossy().to_string(),
            group: repo.group,
            branch: "未知".to_string(),
            relation: RemoteRelation::Error,
            change_label: "!".to_string(),
            hint: format!("读取失败：{err}"),
            has_remote: false,
            remote_url: None,
        },
    }
}

pub fn sort_repo_statuses(statuses: &mut [RepoStatusDto]) {
    statuses.sort_by_key(|repo| repo.relation.sort_rank());
}

pub fn collect_repo_statuses(repos: Vec<RepoRecord>) -> Result<Vec<RepoStatusDto>, String> {
    let started = Instant::now();
    crate::diagnostics::log(
        "repo_status.collect.start",
        format!("repos={}", repos.len()),
    );
    let mut statuses = Vec::with_capacity(repos.len());
    for batch in repos.chunks(STATUS_REFRESH_BATCH_SIZE) {
        crate::diagnostics::log(
            "repo_status.collect.batch",
            format!("batch_size={}", batch.len()),
        );
        let handles = batch
            .iter()
            .cloned()
            .map(|repo| {
                std::thread::spawn(move || {
                    let repo_started = Instant::now();
                    let repo_id = repo.id.clone();
                    let repo_path = repo.path.display().to_string();
                    crate::diagnostics::log(
                        "repo_status.repo.start",
                        format!("repo_id={repo_id} path={repo_path}"),
                    );
                    let state = branch_state(&repo.path);
                    let status = repo_status_from_branch_result(repo, state);
                    crate::diagnostics::log_duration(
                        "repo_status.repo.end",
                        repo_started.elapsed(),
                        format!(
                            "repo_id={} relation={:?} hint={}",
                            status.id, status.relation, status.hint
                        ),
                    );
                    status
                })
            })
            .collect::<Vec<_>>();

        for handle in handles {
            statuses.push(
                handle
                    .join()
                    .map_err(|_| "刷新仓库状态线程异常退出".to_string())?,
            );
        }
    }
    sort_repo_statuses(&mut statuses);
    crate::diagnostics::log_duration(
        "repo_status.collect.ok",
        started.elapsed(),
        format!("statuses={}", statuses.len()),
    );
    Ok(statuses)
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::path::PathBuf;

    fn sample_repo(id: &str) -> RepoRecord {
        RepoRecord {
            id: id.to_string(),
            name: id.to_string(),
            path: PathBuf::from(format!("C:/{id}")),
            group: "全部分组".to_string(),
        }
    }

    #[test]
    fn repo_status_from_branch_error_marks_repository_as_error() {
        let status =
            repo_status_from_branch_result(sample_repo("broken"), Err("missing".to_string()));

        assert_eq!(status.id, "broken");
        assert_eq!(status.branch, "未知");
        assert_eq!(status.relation, RemoteRelation::Error);
        assert_eq!(status.change_label, "!");
        assert!(status.hint.contains("missing"));
    }

    #[test]
    fn repo_status_from_branch_state_preserves_remote_url_and_labels() {
        let status = repo_status_from_branch_result(
            sample_repo("ready"),
            Ok(GitBranchState {
                branch: "main".to_string(),
                relation: RemoteRelation::RemoteAhead,
                ahead: 0,
                behind: 2,
                has_remote: true,
                remote_url: Some("https://github.com/owner/repo".to_string()),
            }),
        );

        assert_eq!(status.branch, "main");
        assert_eq!(status.relation, RemoteRelation::RemoteAhead);
        assert_eq!(status.change_label, "↓ 2");
        assert!(status.has_remote);
        assert_eq!(
            status.remote_url.as_deref(),
            Some("https://github.com/owner/repo")
        );
    }

    #[test]
    fn sort_repo_statuses_keeps_no_remote_last() {
        let mut statuses = vec![
            repo_status_from_branch_result(
                sample_repo("no-remote"),
                Ok(GitBranchState {
                    branch: "main".to_string(),
                    relation: RemoteRelation::NoRemote,
                    ahead: 0,
                    behind: 0,
                    has_remote: false,
                    remote_url: None,
                }),
            ),
            repo_status_from_branch_result(sample_repo("broken"), Err("missing".to_string())),
            repo_status_from_branch_result(
                sample_repo("synced"),
                Ok(GitBranchState {
                    branch: "main".to_string(),
                    relation: RemoteRelation::Synced,
                    ahead: 0,
                    behind: 0,
                    has_remote: true,
                    remote_url: Some("https://github.com/owner/repo".to_string()),
                }),
            ),
        ];

        sort_repo_statuses(&mut statuses);

        assert_eq!(
            statuses
                .iter()
                .map(|status| status.id.as_str())
                .collect::<Vec<_>>(),
            vec!["broken", "synced", "no-remote"],
        );
    }
}
