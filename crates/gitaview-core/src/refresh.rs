//! 仓库状态采集（N07）。
//!
//! `RepoCollector` 把"读取全部仓库状态"从引擎调度中分离：引擎用假采集器与
//! 假时钟做状态机测试；生产使用 `GitCollector`（8 worker 动态工作池，
//! 只读本地 Git，不 Fetch；停止时不再从队列取新仓库）。

use crate::git::commands::{branch_state_monitored, repo_status_from_state, ExecPolicy};
use crate::git::discovery::StopFlag;
use crate::model::RepoStatus;
use crate::registry::RepoRecord;
use crate::settings::RepoSettings;
use std::collections::{HashMap, VecDeque};
use std::sync::{Arc, Mutex};

/// 单个慢仓库（挂死的网络驱动器等）只占用一个 worker。
pub const COLLECT_WORKERS: usize = 8;

/// 采集结果：单仓库失败作为该仓库的 error 行返回；全局失败（线程崩溃 /
/// 引擎无法进入采集）用 Err 表达，由引擎按全局失败处理。
pub trait RepoCollector: Send + Sync {
    fn collect(
        &self,
        repos: &[RepoSettings],
        stop: &StopFlag,
        policy: &ExecPolicy,
    ) -> Result<Vec<RepoStatus>, String>;
}

/// 真实采集器：8 worker 读本地 Git 状态。
#[derive(Default)]
pub struct GitCollector;

impl RepoCollector for GitCollector {
    fn collect(
        &self,
        repos: &[RepoSettings],
        stop: &StopFlag,
        policy: &ExecPolicy,
    ) -> Result<Vec<RepoStatus>, String> {
        let records: Vec<RepoRecord> = repos
            .iter()
            .map(|repo| RepoRecord {
                id: repo.id.clone(),
                name: repo.name.clone(),
                path: repo.path_buf(),
                group_id: repo.group_id.clone(),
            })
            .collect();
        collect_statuses(&records, stop, policy).ok_or_else(|| "仓库状态收集意外中断".to_string())
    }
}

/// 动态工作池：队列共享，worker 取完即止；停止时 worker 不再取新仓库。
pub fn collect_statuses(
    repos: &[RepoRecord],
    stop: &StopFlag,
    policy: &ExecPolicy,
) -> Option<Vec<RepoStatus>> {
    if repos.is_empty() {
        return Some(Vec::new());
    }
    let worker_count = repos.len().min(COLLECT_WORKERS);
    let queue = Arc::new(Mutex::new(VecDeque::from(repos.to_vec())));
    let results = Arc::new(Mutex::new(Vec::with_capacity(repos.len())));

    let mut handles = Vec::with_capacity(worker_count);
    for _ in 0..worker_count {
        let queue = Arc::clone(&queue);
        let results = Arc::clone(&results);
        let stop = stop.clone();
        let policy = *policy;
        handles.push(std::thread::spawn(move || loop {
            if stop.is_stopped() {
                return;
            }
            let repo = {
                let mut queue = match queue.lock() {
                    Ok(queue) => queue,
                    Err(poisoned) => poisoned.into_inner(),
                };
                match queue.pop_front() {
                    Some(repo) => repo,
                    None => return,
                }
            };
            let result = branch_state_monitored(&repo.path, &policy, &stop);
            let status = repo_status_from_state(&repo, result);
            let mut results = match results.lock() {
                Ok(results) => results,
                Err(poisoned) => poisoned.into_inner(),
            };
            results.push(status);
        }));
    }

    for handle in handles {
        handle.join().ok()?; // None = worker panic → 全局失败
    }
    let mut statuses = match Arc::try_unwrap(results) {
        Ok(mutex) => match mutex.into_inner() {
            Ok(statuses) => statuses,
            Err(poisoned) => poisoned.into_inner(),
        },
        Err(_) => return None, // 仍有 worker 持有：理论不可达（全部 join 完成）
    };
    statuses.sort_by(|left, right| left.id.cmp(&right.id));
    Some(statuses)
}

/// 引擎内部：把采集行按仓库 ID 建索引，供配置重投影复用。
pub fn index_by_id(statuses: Vec<RepoStatus>) -> HashMap<String, RepoStatus> {
    statuses
        .into_iter()
        .map(|status| (status.id.clone(), status))
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::model::{ReadState, Relation};
    use std::fs;
    use std::path::{Path, PathBuf};
    use std::process::Command;
    use std::time::{SystemTime, UNIX_EPOCH};

    fn unique_temp_dir(name: &str) -> PathBuf {
        let suffix = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap()
            .as_nanos();
        std::env::temp_dir().join(format!("{name}_{suffix}"))
    }

    fn test_git(repo_path: &Path, args: &[&str]) {
        let output = Command::new("git")
            .args(args)
            .current_dir(repo_path)
            .output()
            .unwrap();
        assert!(
            output.status.success(),
            "git {} failed: {}",
            args.join(" "),
            String::from_utf8_lossy(&output.stderr)
        );
    }

    fn repo_settings(id: &str, path: &Path) -> RepoSettings {
        RepoSettings {
            id: id.into(),
            name: id.into(),
            path: path.to_string_lossy().into_owned(),
            group_id: None,
        }
    }

    #[test]
    fn collects_real_repos_with_worker_pool() {
        let temp = unique_temp_dir("gitaview_n07_collect");
        let mut repos = Vec::new();
        // 超过 worker 数的仓库全部读取成功。
        for index in 0..COLLECT_WORKERS + 3 {
            let repo = temp.join(format!("repo-{index}"));
            fs::create_dir_all(&repo).unwrap();
            test_git(&repo, &["init", "-b", "main"]);
            test_git(&repo, &["config", "user.email", "test@example.test"]);
            test_git(&repo, &["config", "user.name", "Test"]);
            test_git(&repo, &["commit", "--allow-empty", "-m", "initial"]);
            repos.push(repo_settings(&format!("repo-{index}"), &repo));
        }
        // 一个坏路径仓库 → error 行，不拖垮其他仓库。
        repos.push(repo_settings("broken", &temp.join("missing")));

        let statuses = GitCollector
            .collect(&repos, &StopFlag::new(), &ExecPolicy::default())
            .unwrap();

        assert_eq!(statuses.len(), repos.len());
        assert_eq!(statuses[0].id, "broken");
        assert_eq!(statuses[0].read_state, ReadState::Error);
        for status in &statuses[1..] {
            assert_eq!(status.read_state, ReadState::Ok);
            assert_eq!(status.relation, Some(Relation::NoRemote));
        }
        // 输出按 ID 稳定排序。
        let ids: Vec<&str> = statuses.iter().map(|s| s.id.as_str()).collect();
        let mut sorted = ids.clone();
        sorted.sort();
        assert_eq!(ids, sorted);
        fs::remove_dir_all(temp).unwrap();
    }

    #[test]
    fn stop_flag_halts_dispatch_of_new_repos() {
        let temp = unique_temp_dir("gitaview_n07_collect_stop");
        let mut records = Vec::new();
        for index in 0..4 {
            let repo = temp.join(format!("repo-{index}"));
            fs::create_dir_all(repo.join(".git")).unwrap();
            records.push(RepoRecord {
                id: format!("repo-{index}"),
                name: format!("repo-{index}"),
                path: repo,
                group_id: None,
            });
        }
        let stop = StopFlag::new();
        stop.stop();
        let statuses = collect_statuses(&records, &stop, &ExecPolicy::default()).unwrap();
        assert!(
            statuses.is_empty(),
            "stopped engine must not dispatch new repos"
        );
        fs::remove_dir_all(temp).unwrap();
    }
}
