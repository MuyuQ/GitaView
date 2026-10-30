//! 目录扫描：发现 Git 仓库候选（N05）。
//!
//! 预算规则保持一致：深度 8、条目 200000、15 秒 deadline、跳过依赖缓存目录、
//! 不跟随符号链接。新增：预算 / 死线 / 停止任一触发时 `truncated = true`，
//! 调用方不能把部分结果当成"已找全"；停止标志在遍历点检查。

use std::fs;
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Arc;
use std::time::{Duration, Instant};

const MAX_SCAN_DEPTH: usize = 8;
/// 扫描条目预算：指向磁盘根目录等超大树时避免无界遍历。
const MAX_SCAN_ENTRIES: usize = 200_000;
/// 扫描时间预算。
const SCAN_DEADLINE: Duration = Duration::from_secs(15);

/// 停止标志：引擎 Stopping 时置位；扫描与命令派发点检查。
#[derive(Clone, Debug, Default)]
pub struct StopFlag(Arc<AtomicBool>);

impl StopFlag {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn stop(&self) {
        self.0.store(true, Ordering::SeqCst);
    }

    pub fn is_stopped(&self) -> bool {
        self.0.load(Ordering::SeqCst)
    }
}

/// 扫描结果：truncated=true 表示因预算 / 死线 / 停止未完成全量遍历。
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ScanOutcome {
    pub paths: Vec<PathBuf>,
    pub truncated: bool,
}

pub fn is_git_repo(path: &Path) -> bool {
    path.join(".git").exists()
}

fn should_skip_dir(path: &Path) -> bool {
    matches!(
        path.file_name().and_then(|name| name.to_str()),
        Some(
            "node_modules"
                | "target"
                | ".venv"
                | "venv"
                | "dist"
                | "build"
                | ".next"
                | ".cache"
                | ".turbo"
                | ".gradle"
                | "vendor"
                | ".git"
                | ".svn"
                | ".hg"
                | "Library"
                | "AppData"
                | ".Trash"
                | ".cargo"
                | ".rustup"
                | ".npm"
                | ".pnpm-store"
                | ".yarn"
                | "$RECYCLE.BIN"
                | "System Volume Information",
        )
    )
}

/// 扫描预算：条目计数 + 死线，二者任一耗尽即停止深入。
struct ScanBudget {
    deadline: Instant,
    visited: usize,
    /// 预算耗尽时记录原因；外部停止单独通过 stop 标志表达。
    exhausted: bool,
}

impl ScanBudget {
    fn new() -> Self {
        Self {
            deadline: Instant::now() + SCAN_DEADLINE,
            visited: 0,
            exhausted: false,
        }
    }

    fn exhausted(&mut self) -> bool {
        self.visited += 1;
        if self.visited > MAX_SCAN_ENTRIES || Instant::now() >= self.deadline {
            self.exhausted = true;
        }
        self.exhausted
    }
}

/// 检查路径是否在转换字符串时发生了有损替换（U+FFFD）。
/// 含未配对代理字符的 Windows 路径若不拦截，入库后将成为无法使用的坏路径。
pub fn contains_lossy_replacement(path: &Path) -> bool {
    path.to_string_lossy().contains('\u{FFFD}')
}

/// 扫描目录下的 Git 仓库候选。候选不自动添加；UI 让用户选择。
pub fn scan_repositories(root: &Path, stop: &StopFlag) -> ScanOutcome {
    let mut budget = ScanBudget::new();
    let mut found = Vec::new();
    scan_inner(root, &mut found, 0, &mut budget, stop);
    found.sort();
    ScanOutcome {
        truncated: budget.exhausted || stop.is_stopped(),
        paths: found,
    }
}

fn scan_inner(
    path: &Path,
    found: &mut Vec<PathBuf>,
    depth: usize,
    budget: &mut ScanBudget,
    stop: &StopFlag,
) {
    // 停止检查在遍历点：15 秒不是单个文件系统调用的硬保证，
    // 但停止后不再进入新目录。
    if stop.is_stopped() || depth > MAX_SCAN_DEPTH || budget.exhausted() {
        return;
    }
    if is_git_repo(path) {
        found.push(path.to_path_buf());
        return;
    }

    let Ok(entries) = fs::read_dir(path) else {
        return;
    };

    for entry in entries.flatten() {
        // 平铺的大目录（海量文件）同样受预算约束，避免无界元数据调用。
        if stop.is_stopped() || budget.exhausted() {
            return;
        }
        let child = entry.path();
        let Ok(metadata) = fs::symlink_metadata(&child) else {
            continue;
        };
        if metadata.file_type().is_symlink() {
            continue;
        }
        if metadata.is_dir() && !should_skip_dir(&child) {
            scan_inner(&child, found, depth + 1, budget, stop);
            if Instant::now() >= budget.deadline {
                budget.exhausted = true;
                return;
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::time::{SystemTime, UNIX_EPOCH};

    fn unique_temp_dir(name: &str) -> PathBuf {
        let suffix = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap()
            .as_nanos();
        std::env::temp_dir().join(format!("{name}_{suffix}"))
    }

    #[test]
    fn detects_git_repo_by_dot_git_directory_and_file() {
        let temp = unique_temp_dir("gitaview_core_detect");
        fs::create_dir_all(temp.join("dir-repo/.git")).unwrap();
        fs::create_dir_all(temp.join("worktree")).unwrap();
        fs::write(
            temp.join("worktree/.git"),
            "gitdir: ../dir-repo/.git/worktrees/example",
        )
        .unwrap();
        assert!(is_git_repo(&temp.join("dir-repo")));
        assert!(is_git_repo(&temp.join("worktree")));
        assert!(!is_git_repo(&temp));
        fs::remove_dir_all(temp).unwrap();
    }

    #[test]
    fn scan_skips_heavy_directories() {
        let temp = unique_temp_dir("gitaview_core_scan_skip");
        fs::create_dir_all(temp.join("real-repo/.git")).unwrap();
        for skipped in [
            "node_modules",
            ".cache",
            ".turbo",
            ".gradle",
            "vendor",
            "target",
        ] {
            fs::create_dir_all(temp.join(format!("project/{skipped}/fake-repo/.git"))).unwrap();
        }
        let outcome = scan_repositories(&temp, &StopFlag::new());
        assert_eq!(outcome.paths.len(), 1);
        assert!(outcome.paths[0].ends_with("real-repo"));
        assert!(!outcome.truncated);
        fs::remove_dir_all(temp).unwrap();
    }

    #[test]
    fn scan_stops_at_max_depth() {
        let temp = unique_temp_dir("gitaview_core_scan_depth");
        let mut deep = temp.clone();
        for index in 0..=MAX_SCAN_DEPTH + 1 {
            deep = deep.join(format!("level-{index}"));
        }
        fs::create_dir_all(deep.join(".git")).unwrap();
        let outcome = scan_repositories(&temp, &StopFlag::new());
        // 深度上限是常规边界，不算预算中止。
        assert!(outcome.paths.is_empty());
        assert!(!outcome.truncated);
        fs::remove_dir_all(temp).unwrap();
    }

    #[test]
    fn entry_budget_exhaustion_reports_truncated() {
        let temp = unique_temp_dir("gitaview_core_scan_budget");
        for index in 0..20 {
            fs::create_dir_all(temp.join(format!("repo-{index}/.git"))).unwrap();
        }
        let mut budget = ScanBudget {
            deadline: Instant::now() + SCAN_DEADLINE,
            visited: MAX_SCAN_ENTRIES.saturating_sub(4),
            exhausted: false,
        };
        let mut found = Vec::new();
        scan_inner(&temp, &mut found, 0, &mut budget, &StopFlag::new());
        assert!(
            found.len() < 20,
            "budget must stop early, found {}",
            found.len()
        );
        assert!(budget.exhausted);
        fs::remove_dir_all(temp).unwrap();
    }

    #[test]
    fn deadline_exhaustion_reports_truncated() {
        let temp = unique_temp_dir("gitaview_core_scan_deadline");
        for index in 0..5 {
            fs::create_dir_all(temp.join(format!("repo-{index}/.git"))).unwrap();
        }
        let mut budget = ScanBudget {
            deadline: Instant::now() - Duration::from_secs(1),
            visited: 0,
            exhausted: false,
        };
        let mut found = Vec::new();
        scan_inner(&temp, &mut found, 0, &mut budget, &StopFlag::new());
        assert!(found.is_empty());
        assert!(budget.exhausted);
        fs::remove_dir_all(temp).unwrap();
    }

    #[test]
    fn stop_flag_halts_scan_and_reports_truncated() {
        let temp = unique_temp_dir("gitaview_core_scan_stop");
        for index in 0..10 {
            fs::create_dir_all(temp.join(format!("repo-{index}/.git"))).unwrap();
        }
        let stop = StopFlag::new();
        stop.stop();
        let outcome = scan_repositories(&temp, &stop);
        assert!(outcome.paths.is_empty());
        assert!(outcome.truncated);
        fs::remove_dir_all(temp).unwrap();
    }
}
