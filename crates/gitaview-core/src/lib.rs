//! GitaView 原生应用共享核心。
//!
//! N02：契约模型、错误码、Widget 投影、Git 探针。
//! N05：目录扫描（truncated / stop）、网络操作（确认 + 锁 + 前置校验）、
//! 仓库记录基础（稳定 ID / 路径规范化 / 去重）。
//! N06：新设置 schema 与应用服务（revision 事务、原子落盘、损坏隔离）。
//! N07：引擎生命周期、统一刷新调度（requestId / completedRequestId / generation /
//! stateRevision）与可见状态发布。
//! N08：共享视图查询（组计数 / 状态计数 / 筛选 / 搜索 / 排序的唯一实现）。
//! 正式 FFI 在 N09 冻结。

pub mod codec;
pub mod diagnostics;
pub mod engine;
pub mod error;
pub mod git;
pub mod model;
pub mod refresh;
pub mod registry;
pub mod settings;
pub mod storage;
pub mod view;
pub mod widget;

pub use error::CoreError;
