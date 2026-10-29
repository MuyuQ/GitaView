//! 按仓库互斥网络 Git 操作（N05）。
//!
//! 每个仓库同时只允许一个网络操作（fetch / pull / push）；重复请求立即返回
//! REPO_BUSY，不排队。不同仓库互不影响。相对参考实现的调整：锁表是实例而非
//! 全局静态，由引擎（N07）持有并随生命周期销毁；键使用仓库的规范路径，
//! 与设置锁固定顺序（先设置后仓库槽），网络调用期间不持有任何设置锁。

use crate::error::CoreError;
use std::collections::HashMap;
use std::path::{Path, PathBuf};
use std::sync::{Arc, Mutex};

#[derive(Default)]
struct OperationSlot {
    busy: bool,
}

/// 仓库网络操作槽位表。可克隆共享（内部 Mutex），引擎负责创建与回收。
#[derive(Clone, Default)]
pub struct OperationLocks {
    slots: Arc<Mutex<HashMap<PathBuf, OperationSlot>>>,
}

/// 持有期间该仓库不允许第二个网络操作；Drop 时释放。
pub struct RepoOperationGuard {
    slots: Arc<Mutex<HashMap<PathBuf, OperationSlot>>>,
    key: PathBuf,
}

impl Drop for RepoOperationGuard {
    fn drop(&mut self) {
        if let Ok(mut slots) = self.slots.lock() {
            if let Some(slot) = slots.get_mut(&self.key) {
                slot.busy = false;
            }
            // 防御条目无限增长（理论上以仓库数为上限）。
            if slots.len() > 256 {
                slots.retain(|_, slot| !slot.busy);
            }
        }
    }
}

impl OperationLocks {
    pub fn new() -> Self {
        Self::default()
    }

    /// 尝试占用仓库操作槽位；已被占用时返回 REPO_BUSY 而不是阻塞。
    pub fn try_acquire(&self, repo_path: &Path) -> Result<RepoOperationGuard, CoreError> {
        let key = repo_path.to_path_buf();
        let mut slots = self
            .slots
            .lock()
            .map_err(|_| CoreError::OperationUnavailable)?;
        let slot = slots.entry(key.clone()).or_default();
        if slot.busy {
            return Err(CoreError::RepoBusy);
        }
        slot.busy = true;
        Ok(RepoOperationGuard {
            slots: Arc::clone(&self.slots),
            key,
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn second_acquire_for_same_repo_is_rejected_until_release() {
        let locks = OperationLocks::new();
        let repo = std::env::temp_dir().join("gitaview_core_lock_same");

        let guard = locks.try_acquire(&repo).expect("first acquire succeeds");
        assert!(matches!(locks.try_acquire(&repo), Err(CoreError::RepoBusy)));

        drop(guard);
        assert!(locks.try_acquire(&repo).is_ok(), "drop releases the slot");
    }

    #[test]
    fn different_repos_are_independent() {
        let locks = OperationLocks::new();
        let repo_a = std::env::temp_dir().join("gitaview_core_lock_a");
        let repo_b = std::env::temp_dir().join("gitaview_core_lock_b");

        let _guard_a = locks.try_acquire(&repo_a).expect("repo A");
        let _guard_b = locks.try_acquire(&repo_b).expect("repo B");
    }

    #[test]
    fn cross_thread_acquire_maps_to_core_error() {
        let locks = std::sync::Arc::new(OperationLocks::new());
        let repo = std::env::temp_dir().join("gitaview_core_lock_thread");
        let guard = locks.try_acquire(&repo).unwrap();

        let other = std::sync::Arc::clone(&locks);
        let handled = std::thread::spawn(move || {
            matches!(other.try_acquire(&repo), Err(CoreError::RepoBusy))
        });
        assert!(handled.join().unwrap());
        drop(guard);
    }
}
