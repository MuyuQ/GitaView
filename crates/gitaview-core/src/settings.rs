//! 新设置模型与应用服务（N06）。
//!
//! 规则（实施说明 §6.4）：
//! - 新 schema：schemaVersion、revision、仓库记录、分组、defaultGroupId、刷新偏好。
//!   默认仓库 / 分组为空，defaultGroupId=null，定时启用 5 分钟（1–60 校验，不静默夹取）。
//! - 不存储 Pull / Push 确认开关、allowWidgetDrag、compactMode、alwaysOnTop 等旧字段。
//! - 路径由宿主注入；绝不查询或导入旧 Tauri 设置目录。
//! - 事务顺序：校验候选 → 原子落盘 → 提交内存与 revision。写入失败不改变内存 / revision /
//!   对外投影；写入串行化，不丢失已完成的写入。
//! - settings_save 只改分组、归属、默认组与刷新偏好；添加 / 移除走专用入口，
//!   expectedRevision 不匹配 → SETTINGS_CONFLICT。
//! - 移除仓库按统一顺序：短暂设置读定位路径 → 仓库操作槽（忙 → REPO_BUSY）→
//!   再复核仍存在后修改；网络忙时不移除。

use crate::error::CoreError;
use crate::git::operation_lock::OperationLocks;
use crate::model::RefreshPolicy;
use crate::registry::{generate_repo_id, normalize_repo_path};
use crate::storage::store;
use serde::{Deserialize, Serialize};
use std::collections::HashSet;
use std::path::{Path, PathBuf};
use std::sync::{Arc, Mutex};

pub const SETTINGS_SCHEMA_VERSION: u64 = 1;
pub const SETTINGS_FILE_NAME: &str = "settings.json";
/// 首次创建的 revision；每次成功变更 +1。
pub const INITIAL_REVISION: u64 = 1;

/// 持久化后的仓库记录。
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct RepoSettings {
    pub id: String,
    pub name: String,
    /// 规范化绝对路径的字符串形式。
    pub path: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub group_id: Option<String>,
}

impl RepoSettings {
    pub fn path_buf(&self) -> PathBuf {
        PathBuf::from(&self.path)
    }
}

/// 分组记录。名称唯一且非空；删除分组时成员归为未分组（null）。
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct GroupSettings {
    pub id: String,
    pub name: String,
}

/// 完整设置（契约 Settings DTO）。
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct SettingsData {
    pub schema_version: u64,
    pub revision: u64,
    pub repos: Vec<RepoSettings>,
    pub groups: Vec<GroupSettings>,
    /// null 表示未选择默认分组（= 全部仓库）。
    #[serde(default)]
    pub default_group_id: Option<String>,
    pub refresh: RefreshPolicy,
}

impl Default for SettingsData {
    fn default() -> Self {
        Self {
            schema_version: SETTINGS_SCHEMA_VERSION,
            revision: INITIAL_REVISION,
            repos: Vec::new(),
            groups: Vec::new(),
            default_group_id: None,
            refresh: RefreshPolicy::default(),
        }
    }
}

/// settings_save 可编辑字段的完整快照：分组列表（增删改）、仓库归属覆盖、
/// 默认分组与刷新偏好。不包含仓库增删与路径。
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct SettingsEditable {
    pub groups: Vec<GroupSettings>,
    #[serde(default)]
    pub default_group_id: Option<String>,
    pub refresh: RefreshPolicy,
    /// 仅覆盖列出的仓库的归属；未列出的仓库保持现状。
    #[serde(default)]
    pub repo_groups: Vec<RepoGroupAssignment>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct RepoGroupAssignment {
    pub repo_id: String,
    /// null = 未分组。
    #[serde(default)]
    pub group_id: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct SettingsSaveInput {
    pub expected_revision: u64,
    pub settings: SettingsEditable,
}

/// 启动警告：本次启动的结果，不写入设置。
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum StartupWarning {
    /// 损坏配置已隔离并恢复为默认值，UI 应提示。
    SettingsRecovered,
}

impl StartupWarning {
    pub fn as_str(self) -> &'static str {
        match self {
            Self::SettingsRecovered => "SETTINGS_RECOVERED",
        }
    }
}

pub struct OpenedSettings {
    pub service: SettingsService,
    pub warnings: Vec<StartupWarning>,
}

/// 落盘步骤抽象：测试注入失败验证事务语义；生产使用原子文件写。
/// 实现不得持有刷新状态锁或阻塞调用方 UI 线程。
pub trait SettingsPersist: Send + Sync {
    fn persist(&self, path: &Path, text: &str) -> Result<(), CoreError>;
}

/// 默认落盘：store::save_json_atomic。
#[derive(Default)]
pub struct FilePersist;

impl SettingsPersist for FilePersist {
    fn persist(&self, path: &Path, text: &str) -> Result<(), CoreError> {
        store::save_json_atomic(path, text)
    }
}

pub struct SettingsService {
    path: PathBuf,
    state: Mutex<SettingsData>,
    /// 写入串行器：保证"构造候选 → 落盘 → 提交"整体互斥，
    /// 不丢失已完成的写入；持久化期间不持有 state 锁。
    writer: Mutex<()>,
    persist: Arc<dyn SettingsPersist>,
}

impl SettingsService {
    /// 打开（或创建）数据目录中的设置。建立目录、读取失败 → SETTINGS_IO（初始化失败，
    /// 不静默当作首次空配置）；损坏 → 隔离 + 默认值 + SETTINGS_RECOVERED。
    pub fn open(data_dir: &Path) -> Result<OpenedSettings, CoreError> {
        Self::open_with(data_dir, Arc::new(FilePersist))
    }

    pub fn open_with(
        data_dir: &Path,
        persist: Arc<dyn SettingsPersist>,
    ) -> Result<OpenedSettings, CoreError> {
        std::fs::create_dir_all(data_dir).map_err(|_| CoreError::SettingsIo)?;
        let path = data_dir.join(SETTINGS_FILE_NAME);
        let mut warnings = Vec::new();
        let settings = match store::load_json(&path)? {
            store::LoadOutcome::Missing => SettingsData::default(),
            store::LoadOutcome::Present(text) => {
                // 解码成功 ≠ 合法：未知 schema、非法刷新范围、重复 ID、失效引用
                // 都按损坏隔离后恢复默认值，不解析旧字段、不写 migration marker。
                match serde_json::from_str::<SettingsData>(&text)
                    .ok()
                    .filter(|settings| validate_settings_data(settings).is_ok())
                {
                    Some(settings) => settings,
                    None => {
                        store::quarantine(&path)?;
                        warnings.push(StartupWarning::SettingsRecovered);
                        SettingsData::default()
                    }
                }
            }
        };
        let service = Self {
            path,
            state: Mutex::new(settings),
            writer: Mutex::new(()),
            persist,
        };
        Ok(OpenedSettings { service, warnings })
    }

    /// 当前完整设置（含 revision）。
    pub fn get(&self) -> SettingsData {
        match self.state.lock() {
            Ok(state) => state.clone(),
            Err(poisoned) => poisoned.into_inner().clone(),
        }
    }

    /// 通用保存：只允许可编辑字段。revision 不匹配 → SETTINGS_CONFLICT；
    /// 校验失败（组名 / ID / 刷新范围 / 未知仓库）→ INVALID_ARGUMENT。
    /// 落盘失败 → SETTINGS_IO，内存保持提交前版本。
    pub fn save(&self, input: SettingsSaveInput) -> Result<SettingsData, CoreError> {
        self.mutate(|current| {
            if input.expected_revision != current.revision {
                return Err(CoreError::SettingsConflict);
            }
            let editable = &input.settings;
            validate_groups(&editable.groups)?;
            validate_refresh(&editable.refresh)?;

            let mut candidate = current.clone();
            candidate.groups = editable.groups.clone();
            // 默认分组不存在时归回全部（null）。
            candidate.default_group_id = editable
                .default_group_id
                .clone()
                .filter(|id| candidate.groups.iter().any(|group| &group.id == id));
            for assignment in &editable.repo_groups {
                let repo = candidate
                    .repos
                    .iter_mut()
                    .find(|repo| repo.id == assignment.repo_id)
                    .ok_or(CoreError::InvalidArgument)?;
                // 归属指向不存在的分组（分组被删除）→ 归为未分组。
                repo.group_id = assignment
                    .group_id
                    .clone()
                    .filter(|id| candidate.groups.iter().any(|group| group.id == *id));
            }
            // 参照完整性对全部仓库生效：删除分组时设置页只提交空 repoGroups，
            // 未列出的仓库也不能保留指向已删分组的悬挂归属。
            for repo in &mut candidate.repos {
                repo.group_id = repo
                    .group_id
                    .take()
                    .filter(|id| candidate.groups.iter().any(|group| &group.id == id));
            }
            candidate.refresh = editable.refresh;
            Ok(candidate)
        })
    }

    /// 添加仓库：验证 + 规范化 + 去重 + 稳定 ID。重复规范路径返回既有记录，
    /// 不重复插入、不递增 revision。groupId 必须指向已存在分组。
    pub fn repo_add(
        &self,
        raw_path: &Path,
        group_id: Option<&str>,
    ) -> Result<(RepoSettings, SettingsData), CoreError> {
        let canonical = normalize_repo_path(raw_path)?;
        let mut result: Option<RepoSettings> = None;
        let saved = self.mutate(|current| {
            // 写入串行器内再次去重，防并发重复插入。
            if let Some(existing) = current
                .repos
                .iter()
                .find(|repo| Path::new(&repo.path) == canonical)
            {
                result = Some(existing.clone());
                // 不变更：返回 current 原样，mutate 跳过落盘。
                return Ok(current.clone());
            }
            if let Some(group_id) = group_id {
                if !current.groups.iter().any(|group| group.id == group_id) {
                    return Err(CoreError::InvalidArgument);
                }
            }
            let existing_ids: HashSet<String> =
                current.repos.iter().map(|repo| repo.id.clone()).collect();
            let id = generate_repo_id(&canonical, &existing_ids);
            let record = RepoSettings {
                id,
                name: canonical
                    .file_name()
                    .and_then(|name| name.to_str())
                    .unwrap_or("repo")
                    .to_string(),
                path: canonical.to_string_lossy().into_owned(),
                group_id: group_id.map(str::to_string),
            };
            let mut candidate = current.clone();
            candidate.repos.push(record.clone());
            result = Some(record);
            Ok(candidate)
        })?;
        Ok((result.expect("repo_add always produces a record"), saved))
    }

    /// 移除仓库。统一顺序：短暂设置读定位路径 → 仓库操作槽（忙 → REPO_BUSY）→
    /// 复核仍存在 → 修改。不存在 → REPO_NOT_FOUND。
    pub fn repo_remove(
        &self,
        repo_id: &str,
        locks: &OperationLocks,
    ) -> Result<SettingsData, CoreError> {
        // 1) 短暂读取定位路径（不在持有设置锁时等待仓库槽）。
        let path = self
            .get()
            .repos
            .iter()
            .find(|repo| repo.id == repo_id)
            .ok_or(CoreError::RepoNotFound)?
            .path_buf();
        // 2) 仓库操作槽：网络操作进行中不允许移除。
        let _guard = locks.try_acquire(&path)?;
        // 3) 复核仍存在后修改。
        self.mutate(|current| {
            if !current.repos.iter().any(|repo| repo.id == repo_id) {
                return Err(CoreError::RepoNotFound);
            }
            let mut candidate = current.clone();
            candidate.repos.retain(|repo| repo.id != repo_id);
            Ok(candidate)
        })
    }

    /// 事务步骤：构造候选（revision+1）→ 落盘 → 提交内存。
    /// 候选与当前一致（无变更）时跳过落盘，revision 不变。
    fn mutate(
        &self,
        build: impl FnOnce(&SettingsData) -> Result<SettingsData, CoreError>,
    ) -> Result<SettingsData, CoreError> {
        let _writer = self.writer.lock().map_err(|_| CoreError::SettingsIo)?;
        let current = self.get();
        let candidate = build(&current)?;
        // 写入候选与文件读入共用同一校验：任何入口都不能把非法数据落盘。
        validate_settings_data(&candidate)?;
        if candidate == current {
            return Ok(current);
        }
        let mut next = candidate;
        next.revision = current.revision + 1;
        let text = serde_json::to_string_pretty(&next).map_err(|_| CoreError::SettingsIo)?;
        // 落盘失败：内存 / revision / 磁盘保持提交前版本。
        self.persist.persist(&self.path, &text)?;
        let mut state = match self.state.lock() {
            Ok(state) => state,
            Err(poisoned) => poisoned.into_inner(),
        };
        *state = next.clone();
        Ok(next)
    }
}

fn validate_groups(groups: &[GroupSettings]) -> Result<(), CoreError> {
    let mut ids = HashSet::new();
    let mut names = HashSet::new();
    for group in groups {
        if group.id.trim().is_empty() || group.name.trim().is_empty() {
            return Err(CoreError::InvalidArgument);
        }
        if !ids.insert(group.id.clone()) {
            return Err(CoreError::InvalidArgument);
        }
        if !names.insert(group.name.clone()) {
            return Err(CoreError::InvalidArgument);
        }
    }
    Ok(())
}

/// 刷新范围 1–60 分钟：显式拒绝，不静默夹取（与旧实现的 clamp 行为不同，
/// 以新契约为准）。
fn validate_refresh(refresh: &RefreshPolicy) -> Result<(), CoreError> {
    if refresh.interval_minutes == 0 || refresh.interval_minutes > 60 {
        return Err(CoreError::InvalidArgument);
    }
    Ok(())
}

/// 完整业务校验（R09）：serde 解码成功不代表数据合法。文件读入与写入候选
/// 共用同一规则；未知 schema / 非法值 / 重复 ID / 失效引用都判损坏。
fn validate_settings_data(settings: &SettingsData) -> Result<(), CoreError> {
    if settings.schema_version != SETTINGS_SCHEMA_VERSION {
        return Err(CoreError::InvalidArgument);
    }
    if settings.revision < INITIAL_REVISION {
        return Err(CoreError::InvalidArgument);
    }
    validate_groups(&settings.groups)?;
    validate_refresh(&settings.refresh)?;
    let mut repo_ids = HashSet::new();
    let mut paths = HashSet::new();
    for repo in &settings.repos {
        if repo.id.trim().is_empty() || repo.path.trim().is_empty() {
            return Err(CoreError::InvalidArgument);
        }
        if !repo_ids.insert(repo.id.clone()) || !paths.insert(repo.path.clone()) {
            return Err(CoreError::InvalidArgument);
        }
        if let Some(group_id) = &repo.group_id {
            if !settings.groups.iter().any(|group| &group.id == group_id) {
                return Err(CoreError::InvalidArgument);
            }
        }
    }
    if let Some(default_group_id) = &settings.default_group_id {
        if !settings
            .groups
            .iter()
            .any(|group| &group.id == default_group_id)
        {
            return Err(CoreError::InvalidArgument);
        }
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn default_settings_match_contract_defaults() {
        let settings = SettingsData::default();
        assert_eq!(settings.schema_version, 1);
        assert_eq!(settings.revision, INITIAL_REVISION);
        assert!(settings.repos.is_empty());
        assert!(settings.groups.is_empty());
        assert_eq!(settings.default_group_id, None);
        assert_eq!(
            settings.refresh,
            RefreshPolicy {
                enabled: true,
                interval_minutes: 5,
            }
        );
    }

    #[test]
    fn settings_json_uses_camel_case_and_rejects_legacy_fields() {
        let json = serde_json::to_value(SettingsData::default()).unwrap();
        for key in [
            "schemaVersion",
            "revision",
            "repos",
            "groups",
            "defaultGroupId",
            "refresh",
        ] {
            assert!(json.get(key).is_some(), "missing {key}");
        }
        // 旧字段出现 = 损坏，而不是被忽略或迁移。
        let legacy = serde_json::json!({
            "schemaVersion": 1,
            "revision": 1,
            "repos": [],
            "groups": [],
            "refresh": { "enabled": true, "intervalMinutes": 5 },
            "safety": { "confirmPull": true }
        });
        assert!(serde_json::from_value::<SettingsData>(legacy).is_err());
        let legacy_widget = serde_json::json!({
            "schemaVersion": 1,
            "revision": 1,
            "repos": [],
            "groups": [],
            "refresh": { "enabled": true, "intervalMinutes": 5 },
            "appearance": { "allowWidgetDrag": true }
        });
        assert!(serde_json::from_value::<SettingsData>(legacy_widget).is_err());
    }
}
