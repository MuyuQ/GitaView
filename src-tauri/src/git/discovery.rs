use std::fs;
use std::path::{Path, PathBuf};
use std::time::{Duration, Instant};

const MAX_SCAN_DEPTH: usize = 8;
/// 扫描条目预算：指向磁盘根目录等超大树时避免无界遍历
const MAX_SCAN_ENTRIES: usize = 200_000;
/// 扫描时间预算
const SCAN_DEADLINE: Duration = Duration::from_secs(15);

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

/// 扫描预算：条目计数 + 死线，二者任一耗尽即停止深入
struct ScanBudget {
    deadline: Instant,
    visited: usize,
}

impl ScanBudget {
    fn new() -> Self {
        Self {
            deadline: Instant::now() + SCAN_DEADLINE,
            visited: 0,
        }
    }

    fn exhausted(&mut self) -> bool {
        self.visited += 1;
        self.visited > MAX_SCAN_ENTRIES || Instant::now() >= self.deadline
    }
}

/// 检查路径是否在转换字符串时发生了有损替换（U+FFFD）。
/// 含未配对代理字符的 Windows 路径若不拦截，入库后将成为无法使用的坏路径。
pub fn contains_lossy_replacement(path: &Path) -> bool {
    path.to_string_lossy().contains('\u{FFFD}')
}

pub fn scan_repositories(root: &Path) -> Vec<PathBuf> {
    scan_with_budget(root, ScanBudget::new())
}

fn scan_with_budget(root: &Path, mut budget: ScanBudget) -> Vec<PathBuf> {
    let mut found = Vec::new();
    scan_inner(root, &mut found, 0, &mut budget);
    found.sort();
    found
}

fn scan_inner(path: &Path, found: &mut Vec<PathBuf>, depth: usize, budget: &mut ScanBudget) {
    if depth > MAX_SCAN_DEPTH || budget.exhausted() {
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
        let child = entry.path();
        let Ok(metadata) = fs::symlink_metadata(&child) else {
            continue;
        };
        if metadata.file_type().is_symlink() {
            continue;
        }
        if metadata.is_dir() && !should_skip_dir(&child) {
            scan_inner(&child, found, depth + 1, budget);
            if Instant::now() >= budget.deadline {
                return;
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn detects_git_repo_by_dot_git_directory() {
        let temp = std::env::temp_dir().join("gitaview_detect_repo_test");
        let _ = fs::remove_dir_all(&temp);
        fs::create_dir_all(temp.join(".git")).unwrap();
        assert!(is_git_repo(&temp));
        let _ = fs::remove_dir_all(&temp);
    }

    #[test]
    fn detects_git_repo_by_dot_git_file() {
        let temp = std::env::temp_dir().join("gitaview_detect_worktree_test");
        let _ = fs::remove_dir_all(&temp);
        fs::create_dir_all(&temp).unwrap();
        fs::write(temp.join(".git"), "gitdir: ../real/.git/worktrees/example").unwrap();
        assert!(is_git_repo(&temp));
        let _ = fs::remove_dir_all(&temp);
    }

    #[test]
    fn scan_skips_heavy_directories() {
        let temp = std::env::temp_dir().join("gitaview_scan_skip_test");
        let _ = fs::remove_dir_all(&temp);
        // Create a real repo
        fs::create_dir_all(temp.join("real-repo/.git")).unwrap();
        // Create fake repos inside generated/dependency directories.
        for skipped in ["node_modules", ".cache", ".turbo", ".gradle", "vendor"] {
            fs::create_dir_all(temp.join(format!("project/{skipped}/fake-repo/.git"))).unwrap();
        }
        let found = scan_repositories(&temp);
        assert_eq!(found.len(), 1);
        assert!(found[0].ends_with("real-repo"));
        let _ = fs::remove_dir_all(&temp);
    }

    #[test]
    fn scan_stops_at_max_depth() {
        let temp = std::env::temp_dir().join("gitaview_scan_depth_test");
        let _ = fs::remove_dir_all(&temp);
        let mut deep = temp.clone();
        for index in 0..=MAX_SCAN_DEPTH + 1 {
            deep = deep.join(format!("level-{index}"));
        }
        fs::create_dir_all(deep.join(".git")).unwrap();
        let found = scan_repositories(&temp);
        assert!(found.is_empty());
        let _ = fs::remove_dir_all(&temp);
    }

    #[test]
    fn scan_stops_when_entry_budget_is_exhausted() {
        let temp = std::env::temp_dir().join("gitaview_scan_budget_test");
        let _ = fs::remove_dir_all(&temp);
        for index in 0..20 {
            fs::create_dir_all(temp.join(format!("repo-{index}/.git"))).unwrap();
        }
        // 预算几乎耗尽：只能再访问极少数目录
        let budget = ScanBudget {
            deadline: Instant::now() + SCAN_DEADLINE,
            visited: MAX_SCAN_ENTRIES.saturating_sub(4),
        };
        let found = scan_with_budget(&temp, budget);
        assert!(
            found.len() < 20,
            "budget exhaustion must stop the scan early (found {})",
            found.len()
        );
        let _ = fs::remove_dir_all(&temp);
    }

    #[test]
    fn scan_stops_when_deadline_has_passed() {
        let temp = std::env::temp_dir().join("gitaview_scan_deadline_test");
        let _ = fs::remove_dir_all(&temp);
        for index in 0..5 {
            fs::create_dir_all(temp.join(format!("repo-{index}/.git"))).unwrap();
        }
        let budget = ScanBudget {
            deadline: Instant::now() - Duration::from_secs(1),
            visited: 0,
        };
        let found = scan_with_budget(&temp, budget);
        assert!(found.is_empty(), "expired deadline must stop the scan");
        let _ = fs::remove_dir_all(&temp);
    }

    #[cfg(unix)]
    #[test]
    fn lossy_path_detection_flags_unpaired_bytes() {
        use std::os::unix::ffi::OsStrExt;
        let lossy = PathBuf::from(std::ffi::OsStr::from_bytes(b"bad-\xff-path"));
        assert!(contains_lossy_replacement(&lossy));
        assert!(!contains_lossy_replacement(Path::new("good-path")));
    }
}
