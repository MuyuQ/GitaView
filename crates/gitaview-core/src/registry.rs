//! 仓库记录基础（N05）：稳定机器 ID、路径规范化与去重。
//! 设置持久化、分组归属规则与 revision 属于 N06；此处只提供无状态工具。

use crate::error::CoreError;
use crate::git::discovery::{contains_lossy_replacement, is_git_repo};
use std::collections::HashSet;
use std::path::{Path, PathBuf};

/// 仓库记录：ID 为稳定机器标识；路径保存规范形式。
/// name 是界面文字，可以与 ID 不同；不把界面文字当标识。
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RepoRecord {
    pub id: String,
    pub name: String,
    pub path: PathBuf,
    pub group_id: Option<String>,
}

/// 由路径生成稳定 ID：取文件名，非 ASCII 字母数字折叠为 '-'，
/// 与现有 ID 冲突时追加 -2、-3…（逻辑与参考实现一致，不改变既有语义）。
pub fn generate_repo_id(path: &Path, existing_ids: &HashSet<String>) -> String {
    let base = path
        .file_name()
        .and_then(|name| name.to_str())
        .unwrap_or("repo")
        .chars()
        .map(|ch| {
            if ch.is_ascii_alphanumeric() {
                ch.to_ascii_lowercase()
            } else {
                '-'
            }
        })
        .collect::<String>()
        .trim_matches('-')
        .to_string();
    let base = if base.is_empty() {
        "repo".to_string()
    } else {
        base
    };
    if !existing_ids.contains(&base) {
        return base;
    }
    let mut suffix = 2;
    loop {
        let candidate = format!("{base}-{suffix}");
        if !existing_ids.contains(&candidate) {
            return candidate;
        }
        suffix += 1;
    }
}

/// 规范化仓库路径：必须是存在的绝对路径、指向 Git 工作区、无有损字符。
/// 返回 canonical 形式（Windows 上避免 UNC 前缀，dunce 处理）。
pub fn normalize_repo_path(raw: &Path) -> Result<PathBuf, CoreError> {
    if !raw.is_absolute() {
        return Err(CoreError::InvalidPath);
    }
    if contains_lossy_replacement(raw) {
        return Err(CoreError::InvalidPath);
    }
    if !raw.is_dir() || !is_git_repo(raw) {
        return Err(CoreError::InvalidPath);
    }
    dunce::canonicalize(raw).map_err(|_| CoreError::InvalidPath)
}

/// 按规范路径查找已有记录（添加时去重：重复路径返回既有记录，不重复插入）。
pub fn find_by_canonical_path<'a>(
    records: &'a [RepoRecord],
    canonical: &Path,
) -> Option<&'a RepoRecord> {
    records.iter().find(|record| record.path == canonical)
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs;
    use std::time::{SystemTime, UNIX_EPOCH};

    fn unique_temp_dir(name: &str) -> PathBuf {
        let suffix = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap()
            .as_nanos();
        std::env::temp_dir().join(format!("{name}_{suffix}"))
    }

    fn ids(values: &[&str]) -> HashSet<String> {
        values.iter().map(|value| value.to_string()).collect()
    }

    #[test]
    fn repo_id_sanitizes_file_name_and_appends_suffixes() {
        assert_eq!(
            generate_repo_id(Path::new("/Users/me/My Repo.View"), &ids(&[])),
            "my-repo-view"
        );
        assert_eq!(
            generate_repo_id(
                Path::new("/Users/me/My Repo"),
                &ids(&["my-repo", "my-repo-2"])
            ),
            "my-repo-3"
        );
        assert_eq!(generate_repo_id(Path::new("/"), &ids(&["repo"])), "repo-2");
    }

    #[test]
    fn normalize_rejects_missing_relative_and_non_git_paths() {
        assert_eq!(
            normalize_repo_path(Path::new("relative/repo")),
            Err(CoreError::InvalidPath)
        );
        assert_eq!(
            normalize_repo_path(Path::new("Z:/gitaview/core/definitely-missing")),
            Err(CoreError::InvalidPath)
        );

        let temp = unique_temp_dir("gitaview_core_registry_plain");
        fs::create_dir_all(&temp).unwrap();
        assert_eq!(normalize_repo_path(&temp), Err(CoreError::InvalidPath));
        fs::remove_dir_all(temp).unwrap();
    }

    #[test]
    fn normalize_returns_canonical_path_for_real_repo() {
        let temp = unique_temp_dir("gitaview_core_registry_repo");
        let repo = temp.join("repo");
        fs::create_dir_all(repo.join(".git")).unwrap();
        let normalized = normalize_repo_path(&repo).expect("valid repo normalizes");
        assert!(normalized.is_absolute());
        assert!(normalized.ends_with("repo"));
        fs::remove_dir_all(temp).unwrap();
    }

    #[test]
    fn duplicate_detection_uses_canonical_paths() {
        let temp = unique_temp_dir("gitaview_core_registry_dupe");
        let repo = temp.join("repo");
        fs::create_dir_all(repo.join(".git")).unwrap();
        let canonical = normalize_repo_path(&repo).unwrap();
        // Windows 大小写不同的写法规范化后应命中同一条记录。
        let mut variant = canonical.clone();
        if let Some(name) = canonical.file_name().and_then(|n| n.to_str()) {
            variant.pop();
            variant.push(name.to_uppercase());
        }
        let variant_canonical = normalize_repo_path(&variant).unwrap();
        let records = vec![RepoRecord {
            id: "repo".into(),
            name: "repo".into(),
            path: canonical,
            group_id: None,
        }];
        assert!(find_by_canonical_path(&records, &variant_canonical).is_some());
        fs::remove_dir_all(temp).unwrap();
    }
}
