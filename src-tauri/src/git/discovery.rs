use std::fs;
use std::path::{Path, PathBuf};

pub fn is_git_repo(path: &Path) -> bool {
    path.join(".git").exists()
}

fn should_skip_dir(path: &Path) -> bool {
    matches!(
        path.file_name().and_then(|name| name.to_str()),
        Some("node_modules" | "target" | ".venv" | "dist" | "build" | ".next")
    )
}

pub fn scan_repositories(root: &Path) -> Vec<PathBuf> {
    let mut found = Vec::new();
    scan_inner(root, &mut found);
    found.sort();
    found
}

fn scan_inner(path: &Path, found: &mut Vec<PathBuf>) {
    if is_git_repo(path) {
        found.push(path.to_path_buf());
        return;
    }

    let Ok(entries) = fs::read_dir(path) else {
        return;
    };

    for entry in entries.flatten() {
        let child = entry.path();
        if child.is_dir() && !should_skip_dir(&child) {
            scan_inner(&child, found);
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
        // Create a fake repo inside node_modules
        fs::create_dir_all(temp.join("project/node_modules/fake-repo/.git")).unwrap();
        let found = scan_repositories(&temp);
        assert_eq!(found.len(), 1);
        assert!(found[0].ends_with("real-repo"));
        let _ = fs::remove_dir_all(&temp);
    }
}
