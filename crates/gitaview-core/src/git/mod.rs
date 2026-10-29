//! Git 子进程执行模块：固定参数、deadline、进程树终止与 origin URL 安全转换。

pub mod commands;
pub mod discovery;
pub mod operation_lock;
pub mod process;
pub mod remote;

pub use commands::{
    branch_state, branch_state_monitored, execute_repo_operation, git_program, probe_repository,
    repo_status_from_state, validate_repo_operation, ExecPolicy, GitBranchState, GitCommandError,
    ProbeRequest, ProbeResult, RepoGitOperation, RepoOperationInput, RepoOperationOutcome,
    GIT_NETWORK_TIMEOUT, GIT_STATUS_TIMEOUT,
};
pub use discovery::{
    contains_lossy_replacement, is_git_repo, scan_repositories, ScanOutcome, StopFlag,
};
pub use operation_lock::{OperationLocks, RepoOperationGuard};
