use std::fs;
use std::path::{Path, PathBuf};

pub fn is_git_repo(path: &Path) -> bool {
    path.join(".git").exists()
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
        if child.is_dir() {
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
}
