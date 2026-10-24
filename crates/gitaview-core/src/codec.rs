//! 契约解码入口：宿主（Swift / C#）镜像同一规则。
//! 未知 schemaVersion 必须显式拒绝；不允许解码默认值隐藏不匹配。

use crate::model::{AppSnapshot, WidgetSnapshot, SCHEMA_VERSION};
use serde::Deserialize;
use std::fmt;

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum DecodeError {
    /// schemaVersion 不是本版本支持的值。
    UnknownSchema,
    /// JSON 结构与契约不符（缺字段、未知字段、类型错误）。
    Malformed,
}

impl std::fmt::Display for DecodeError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::UnknownSchema => f.write_str("UNKNOWN_SCHEMA"),
            Self::Malformed => f.write_str("MALFORMED"),
        }
    }
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
struct VersionOnly {
    schema_version: u64,
}

fn check_schema_version(value: &serde_json::Value) -> Result<(), DecodeError> {
    match VersionOnly::deserialize(value) {
        Ok(version) if version.schema_version == SCHEMA_VERSION => Ok(()),
        Ok(_) => Err(DecodeError::UnknownSchema),
        Err(_) => Err(DecodeError::Malformed),
    }
}

/// 先校验 schemaVersion，再解码 Widget 快照。
pub fn decode_widget_snapshot(bytes: &[u8]) -> Result<WidgetSnapshot, DecodeError> {
    let value: serde_json::Value =
        serde_json::from_slice(bytes).map_err(|_| DecodeError::Malformed)?;
    check_schema_version(&value)?;
    WidgetSnapshot::deserialize(value).map_err(|_| DecodeError::Malformed)
}

/// 同规则解码应用快照。
pub fn decode_app_snapshot(bytes: &[u8]) -> Result<AppSnapshot, DecodeError> {
    let value: serde_json::Value =
        serde_json::from_slice(bytes).map_err(|_| DecodeError::Malformed)?;
    check_schema_version(&value)?;
    AppSnapshot::deserialize(value).map_err(|_| DecodeError::Malformed)
}
