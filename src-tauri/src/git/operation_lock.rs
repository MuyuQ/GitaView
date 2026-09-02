//! 按仓库路径互斥网络 git 操作（fetch/pull/push）。
//!
//! 重复触发同一仓库的 pull/push 会争抢 `index.lock`，
//! 这里在应用层提供"每个仓库同时只允许一个网络操作"的互斥，
//! 重复触发立即返回错误而不是排队等待。

use std::collections::HashMap;
use std::path::{Path, PathBuf};
use std::sync::{Mutex, OnceLock};

struct OperationSlot {
    busy: bool,
}

static REPO_OPERATION_LOCKS: OnceLock<Mutex<HashMap<PathBuf, OperationSlot>>> = OnceLock::new();

fn locks() -> &'static Mutex<HashMap<PathBuf, OperationSlot>> {
    REPO_OPERATION_LOCKS.get_or_init(|| Mutex::new(HashMap::new()))
}

/// 持有期间该仓库不允许第二个网络操作；Drop 时释放。
pub struct RepoOperationGuard {
    key: PathBuf,
}

impl Drop for RepoOperationGuard {
    fn drop(&mut self) {
        if let Ok(mut locks) = locks().lock() {
            if let Some(slot) = locks.get_mut(&self.key) {
                slot.busy = false;
            }
            // 防御路径条目无限增长（理论上以仓库数为上限）
            if locks.len() > 256 {
                locks.retain(|_, slot| !slot.busy);
            }
        }
    }
}

/// 尝试占用仓库操作槽位；已被占用时返回错误而不是阻塞。
pub fn try_acquire_repo_operation(repo_path: &Path) -> Result<RepoOperationGuard, String> {
    let key = repo_path.to_path_buf();
    let mut locks = locks().lock().map_err(|_| "操作锁状态异常".to_string())?;
    let slot = locks
        .entry(key.clone())
        .or_insert(OperationSlot { busy: false });
    if slot.busy {
        return Err("该仓库已有网络操作正在进行，请等待完成后再试".to_string());
    }
    slot.busy = true;
    Ok(RepoOperationGuard { key })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn second_acquire_for_same_repo_is_rejected_until_release() {
        let repo = std::env::temp_dir().join("gitaview_lock_same_repo_test");

        let guard = try_acquire_repo_operation(&repo).expect("first acquire should succeed");
        assert!(
            try_acquire_repo_operation(&repo).is_err(),
            "second concurrent acquire must fail"
        );

        drop(guard);
        let reacquired = try_acquire_repo_operation(&repo);
        assert!(reacquired.is_ok(), "guard drop should release the slot");
    }

    #[test]
    fn different_repos_are_independent() {
        let repo_a = std::env::temp_dir().join("gitaview_lock_repo_a_test");
        let repo_b = std::env::temp_dir().join("gitaview_lock_repo_b_test");

        let _guard_a = try_acquire_repo_operation(&repo_a).expect("repo A acquire");
        let _guard_b = try_acquire_repo_operation(&repo_b).expect("repo B acquire");
    }
}
