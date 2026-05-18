use crate::domain::status::RemoteRelation;

#[derive(Debug, Clone, Copy)]
pub(crate) enum RepoGitOperation {
    Fetch,
    Pull,
    Push,
}

pub(crate) fn validate_repo_git_operation(
    operation: RepoGitOperation,
    relation: RemoteRelation,
    has_remote: bool,
) -> Result<(), String> {
    match (operation, relation) {
        (_, RemoteRelation::Error) => Err("当前仓库状态异常，请先刷新或检查仓库路径".to_string()),
        (RepoGitOperation::Fetch, RemoteRelation::NoRemote) if has_remote => Ok(()),
        (_, RemoteRelation::NoRemote) => Err("当前仓库没有可操作的远端分支".to_string()),
        (RepoGitOperation::Fetch, _) => Ok(()),
        (RepoGitOperation::Pull, RemoteRelation::RemoteAhead | RemoteRelation::Diverged) => Ok(()),
        (RepoGitOperation::Pull, RemoteRelation::Synced) => {
            Err("当前仓库已同步，无需 Pull".to_string())
        }
        (RepoGitOperation::Pull, RemoteRelation::LocalAhead) => {
            Err("当前仓库只有本地提交，无需 Pull".to_string())
        }
        (RepoGitOperation::Push, RemoteRelation::LocalAhead | RemoteRelation::Diverged) => Ok(()),
        (RepoGitOperation::Push, RemoteRelation::Synced) => {
            Err("当前仓库已同步，无需 Push".to_string())
        }
        (RepoGitOperation::Push, RemoteRelation::RemoteAhead) => {
            Err("当前仓库落后远端，请先 Pull 或处理分叉".to_string())
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn fetch_allows_repositories_with_remote_even_without_upstream() {
        assert!(
            validate_repo_git_operation(RepoGitOperation::Fetch, RemoteRelation::Synced, true)
                .is_ok()
        );
        assert!(validate_repo_git_operation(
            RepoGitOperation::Fetch,
            RemoteRelation::LocalAhead,
            true
        )
        .is_ok());
        assert!(validate_repo_git_operation(
            RepoGitOperation::Fetch,
            RemoteRelation::RemoteAhead,
            true
        )
        .is_ok());
        assert!(validate_repo_git_operation(
            RepoGitOperation::Fetch,
            RemoteRelation::Diverged,
            true
        )
        .is_ok());
        assert!(validate_repo_git_operation(
            RepoGitOperation::Fetch,
            RemoteRelation::NoRemote,
            true
        )
        .is_ok());
        assert!(validate_repo_git_operation(
            RepoGitOperation::Fetch,
            RemoteRelation::NoRemote,
            false
        )
        .is_err());
        assert!(
            validate_repo_git_operation(RepoGitOperation::Fetch, RemoteRelation::Error, true)
                .is_err()
        );
    }

    #[test]
    fn pull_only_allows_remote_changes_that_need_user_action() {
        assert!(validate_repo_git_operation(
            RepoGitOperation::Pull,
            RemoteRelation::RemoteAhead,
            true
        )
        .is_ok());
        assert!(validate_repo_git_operation(
            RepoGitOperation::Pull,
            RemoteRelation::Diverged,
            true
        )
        .is_ok());
        assert!(
            validate_repo_git_operation(RepoGitOperation::Pull, RemoteRelation::Synced, true)
                .is_err()
        );
        assert!(validate_repo_git_operation(
            RepoGitOperation::Pull,
            RemoteRelation::LocalAhead,
            true
        )
        .is_err());
        assert!(validate_repo_git_operation(
            RepoGitOperation::Pull,
            RemoteRelation::NoRemote,
            true
        )
        .is_err());
        assert!(
            validate_repo_git_operation(RepoGitOperation::Pull, RemoteRelation::Error, true)
                .is_err()
        );
    }

    #[test]
    fn push_only_allows_local_changes_that_need_user_action() {
        assert!(validate_repo_git_operation(
            RepoGitOperation::Push,
            RemoteRelation::LocalAhead,
            true
        )
        .is_ok());
        assert!(validate_repo_git_operation(
            RepoGitOperation::Push,
            RemoteRelation::Diverged,
            true
        )
        .is_ok());
        assert!(
            validate_repo_git_operation(RepoGitOperation::Push, RemoteRelation::Synced, true)
                .is_err()
        );
        assert!(validate_repo_git_operation(
            RepoGitOperation::Push,
            RemoteRelation::RemoteAhead,
            true
        )
        .is_err());
        assert!(validate_repo_git_operation(
            RepoGitOperation::Push,
            RemoteRelation::NoRemote,
            true
        )
        .is_err());
        assert!(
            validate_repo_git_operation(RepoGitOperation::Push, RemoteRelation::Error, true)
                .is_err()
        );
    }
}
