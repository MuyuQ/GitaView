use crate::domain::repo::RepoRecord;
use crate::domain::settings::AppSettings;
use std::path::Path;

pub(crate) fn repo_id_from_path(path: &Path, settings: &AppSettings) -> String {
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
    if !settings.repos.iter().any(|repo| repo.id == base) {
        return base;
    }
    let mut suffix = 2;
    loop {
        let candidate = format!("{base}-{suffix}");
        if !settings.repos.iter().any(|repo| repo.id == candidate) {
            return candidate;
        }
        suffix += 1;
    }
}

pub(crate) fn find_repo<'a>(
    settings: &'a AppSettings,
    repo_id: &str,
) -> Result<&'a RepoRecord, String> {
    settings
        .repos
        .iter()
        .find(|repo| repo.id == repo_id)
        .ok_or_else(|| format!("未找到仓库：{repo_id}"))
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::path::PathBuf;

    fn sample_repo(id: &str, path: &str) -> RepoRecord {
        RepoRecord {
            id: id.to_string(),
            name: id.to_string(),
            path: PathBuf::from(path),
            group: "全部分组".to_string(),
        }
    }

    fn settings_with_repos(repos: Vec<RepoRecord>) -> AppSettings {
        AppSettings {
            repos,
            ..AppSettings::default()
        }
        .normalized()
    }

    #[test]
    fn repo_id_from_path_sanitizes_file_name() {
        let settings = settings_with_repos(Vec::new());

        assert_eq!(
            repo_id_from_path(Path::new("/Users/me/My Repo.View"), &settings),
            "my-repo-view"
        );
    }

    #[test]
    fn repo_id_from_path_uses_suffix_when_base_exists() {
        let settings = settings_with_repos(vec![
            sample_repo("my-repo", "/tmp/a"),
            sample_repo("my-repo-2", "/tmp/b"),
        ]);

        assert_eq!(
            repo_id_from_path(Path::new("/Users/me/My Repo"), &settings),
            "my-repo-3"
        );
    }

    #[test]
    fn repo_id_from_path_falls_back_for_empty_names() {
        let settings = settings_with_repos(vec![sample_repo("repo", "/tmp/a")]);

        assert_eq!(repo_id_from_path(Path::new("/"), &settings), "repo-2");
    }

    #[test]
    fn find_repo_returns_matching_repository_or_clear_error() {
        let settings = settings_with_repos(vec![sample_repo("alpha", "/tmp/alpha")]);

        assert_eq!(find_repo(&settings, "alpha").unwrap().name, "alpha");
        assert_eq!(
            find_repo(&settings, "missing").unwrap_err(),
            "未找到仓库：missing"
        );
    }
}
