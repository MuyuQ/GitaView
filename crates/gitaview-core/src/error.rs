//! 稳定业务错误码。code 与 UI 文案分离：宿主按 code 映射本地化文字，
//! 不得解析错误消息文本决定控制流。

use serde::{Deserialize, Serialize};
use std::fmt;

/// 错误对象：稳定 code + 可选安全 message。
/// message 不包含用户路径、带凭据 URL 或原始 Git stderr。
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ErrorObject {
    pub code: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub message: Option<String>,
}

impl ErrorObject {
    pub fn new(code: impl Into<String>) -> Self {
        Self {
            code: code.into(),
            message: None,
        }
    }

    pub fn with_message(code: impl Into<String>, message: impl Into<String>) -> Self {
        Self {
            code: code.into(),
            message: Some(message.into()),
        }
    }
}

/// 核心业务错误。`code()` 的返回值写入契约，只增不改语义。
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum CoreError {
    /// 参数结构 / 枚举非法。
    InvalidArgument,
    /// 路径不存在或不是绝对路径。
    InvalidPath,
    /// 找不到用户安装的 Git。
    GitNotFound,
    /// 仓库 ID 不存在。
    RepoNotFound,
    /// 同仓库已有网络操作在执行。
    RepoBusy,
    /// 操作需要 origin 远端。
    OriginRequired,
    /// 操作需要可比较的 origin 分支。
    ComparisonRequired,
    /// Pull / Push 缺少用户确认。
    ConfirmationRequired,
    /// 当前状态不允许该操作。
    OperationUnavailable,
    /// 设置 revision 冲突。
    SettingsConflict,
    /// 设置读写失败。
    SettingsIo,
    /// Git 命令以非零退出结束。
    GitFailed,
    /// Git 命令超时。
    GitTimeout,
    /// 引擎正在停止。
    EngineStopping,
}

impl CoreError {
    pub fn code(&self) -> &'static str {
        match self {
            Self::InvalidArgument => "INVALID_ARGUMENT",
            Self::InvalidPath => "INVALID_PATH",
            Self::GitNotFound => "GIT_NOT_FOUND",
            Self::RepoNotFound => "REPO_NOT_FOUND",
            Self::RepoBusy => "REPO_BUSY",
            Self::OriginRequired => "ORIGIN_REQUIRED",
            Self::ComparisonRequired => "COMPARISON_REQUIRED",
            Self::ConfirmationRequired => "CONFIRMATION_REQUIRED",
            Self::OperationUnavailable => "OPERATION_UNAVAILABLE",
            Self::SettingsConflict => "SETTINGS_CONFLICT",
            Self::SettingsIo => "SETTINGS_IO",
            Self::GitFailed => "GIT_FAILED",
            Self::GitTimeout => "GIT_TIMEOUT",
            Self::EngineStopping => "ENGINE_STOPPING",
        }
    }

    /// 转成契约错误对象；message 为可选的安全描述。
    pub fn to_error_object(&self) -> ErrorObject {
        ErrorObject {
            code: self.code().to_string(),
            message: None,
        }
    }
}

impl fmt::Display for CoreError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(self.code())
    }
}

impl std::error::Error for CoreError {}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn error_codes_are_stable_strings() {
        assert_eq!(CoreError::InvalidPath.code(), "INVALID_PATH");
        assert_eq!(CoreError::GitTimeout.code(), "GIT_TIMEOUT");
        assert_eq!(
            CoreError::ConfirmationRequired.code(),
            "CONFIRMATION_REQUIRED"
        );
    }

    #[test]
    fn error_object_serializes_without_optional_message() {
        let json = serde_json::to_value(CoreError::RepoBusy.to_error_object()).unwrap();
        assert_eq!(json, serde_json::json!({"code": "REPO_BUSY"}));
    }
}
