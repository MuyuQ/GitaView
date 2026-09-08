use crate::domain::repo::{RepoRecord, RepoStatusDto};
use crate::domain::status::RemoteRelation;
use crate::git::commands::{branch_state, GitBranchState};
use crate::git::status_text::{change_label, state_hint};
use std::collections::VecDeque;
use std::sync::{Arc, Mutex};
use std::time::Instant;

/// 动态工作池大小：单个慢仓库（如挂死的网络驱动器）只占用一个 worker，
/// 不再像旧的批处理 + join 模式那样拖慢整批仓库。
const STATUS_REFRESH_WORKERS: usize = 8;

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
    let repo_count = repos.len();
    let worker_count = repo_count.clamp(1, STATUS_REFRESH_WORKERS);
    let queue = Arc::new(Mutex::new(VecDeque::from(repos)));
    let results = Arc::new(Mutex::new(Vec::with_capacity(repo_count)));

    let mut handles = Vec::with_capacity(worker_count);
    for _ in 0..worker_count {
        let queue = Arc::clone(&queue);
        let results = Arc::clone(&results);
        handles.push(std::thread::spawn(move || {
            while let Some(repo) = pop_repo(&queue) {
                let repo_started = Instant::now();
                let repo_id = repo.id.clone();
                let repo_path = crate::diagnostics::redact_path(&repo.path);
                crate::diagnostics::log(
                    "repo_status.repo.start",
                    format!("repo_id={repo_id} path={repo_path}"),
                );
                let state = branch_state(&repo.path);
                let status = repo_status_from_branch_result(repo, state);
                crate::diagnostics::log_duration(
                    "repo_status.repo.end",
                    repo_started.elapsed(),
                    format!("repo_id={} relation={:?}", status.id, status.relation),
                );
                push_result(&results, status);
            }
        }));
    }

    for handle in handles {
        handle
            .join()
            .map_err(|_| "刷新仓库状态线程异常退出".to_string())?;
    }

    let mut statuses = take_results(results);
    sort_repo_statuses(&mut statuses);
    crate::diagnostics::log_duration(
        "repo_status.collect.ok",
        started.elapsed(),
        format!("statuses={}", statuses.len()),
    );
    Ok(statuses)
}

fn pop_repo(queue: &Mutex<VecDeque<RepoRecord>>) -> Option<RepoRecord> {
    let mut guard = match queue.lock() {
        Ok(guard) => guard,
        Err(poisoned) => poisoned.into_inner(),
    };
    guard.pop_front()
}

fn push_result(results: &Mutex<Vec<RepoStatusDto>>, status: RepoStatusDto) {
    let mut guard = match results.lock() {
        Ok(guard) => guard,
        Err(poisoned) => poisoned.into_inner(),
    };
    guard.push(status);
}

fn take_results(results: Arc<Mutex<Vec<RepoStatusDto>>>) -> Vec<RepoStatusDto> {
    match Arc::try_unwrap(results) {
        Ok(mutex) => match mutex.into_inner() {
            Ok(statuses) => statuses,
            Err(poisoned) => poisoned.into_inner(),
        },
        Err(arc) => match arc.lock() {
            Ok(guard) => guard.clone(),
            Err(poisoned) => poisoned.into_inner().clone(),
        },
    }
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
                remote_branch: Some("main".to_string()),
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
                    remote_branch: Some("main".to_string()),
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
                    remote_branch: Some("main".to_string()),
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

    #[test]
    fn collect_handles_more_repos_than_workers_without_failing() {
        let repos: Vec<RepoRecord> = (0..12)
            .map(|index| {
                let mut repo = sample_repo(&format!("missing-{index}"));
                repo.path = std::path::PathBuf::from(format!("Z:/definitely-missing-{index}"));
                repo
            })
            .collect();

        let statuses =
            collect_repo_statuses(repos).expect("worker pool should always produce statuses");

        assert_eq!(statuses.len(), 12);
        assert!(statuses
            .iter()
            .all(|status| status.relation == RemoteRelation::Error));
        // no_remote 永远排最后：这里全是 error，排序后 error 在最前
        assert_eq!(statuses[0].relation, RemoteRelation::Error);
    }

    #[test]
    fn collect_returns_sorted_statuses() {
        let repos = vec![
            {
                let mut repo = sample_repo("missing-a");
                repo.path = std::path::PathBuf::from("Z:/collect-missing-a");
                repo
            },
            {
                let mut repo = sample_repo("missing-b");
                repo.path = std::path::PathBuf::from("Z:/collect-missing-b");
                repo
            },
        ];

        let statuses = collect_repo_statuses(repos).expect("collect should succeed");

        assert_eq!(statuses.len(), 2);
        assert!(statuses
            .iter()
            .all(|status| status.relation == RemoteRelation::Error));
    }
}
