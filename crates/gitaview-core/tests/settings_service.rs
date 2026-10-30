//! N06 配置集成测试：事务语义、冲突、损坏恢复、去重、忙移除与并发一致性。
//! 仓库目录用带 `.git` 的临时目录（设置层不执行 Git，无需真实仓库）。

use gitaview_core::error::CoreError;
use gitaview_core::git::operation_lock::OperationLocks;
use gitaview_core::model::RefreshPolicy;
use gitaview_core::settings::{
    GroupSettings, RepoGroupAssignment, SettingsEditable, SettingsPersist, SettingsSaveInput,
    SettingsService, StartupWarning,
};
use std::fs;
use std::path::{Path, PathBuf};
use std::sync::Arc;
use std::time::{Duration, SystemTime, UNIX_EPOCH};

fn unique_temp_dir(name: &str) -> PathBuf {
    let suffix = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap()
        .as_nanos();
    std::env::temp_dir().join(format!("{name}_{suffix}"))
}

/// 带 `.git` 的假仓库目录：设置层只验证路径，不执行 Git。
fn fake_repo(temp: &Path, name: &str) -> PathBuf {
    let repo = temp.join(name);
    fs::create_dir_all(repo.join(".git")).unwrap();
    repo
}

fn open(temp: &Path) -> SettingsService {
    SettingsService::open(temp).unwrap().service
}

fn group(id: &str, name: &str) -> GroupSettings {
    GroupSettings {
        id: id.into(),
        name: name.into(),
    }
}

fn editable(groups: Vec<GroupSettings>) -> SettingsEditable {
    SettingsEditable {
        groups,
        default_group_id: None,
        refresh: RefreshPolicy::default(),
        repo_groups: Vec::new(),
    }
}

/// 注入失败的落盘器。
struct FailingPersist;

impl SettingsPersist for FailingPersist {
    fn persist(&self, _path: &Path, _text: &str) -> Result<(), CoreError> {
        Err(CoreError::SettingsIo)
    }
}

#[test]
fn fresh_open_returns_defaults_and_change_persists() {
    let temp = unique_temp_dir("gitaview_n06_fresh");
    let service = open(&temp);

    let settings = service.get();
    assert_eq!(settings.revision, 1);
    assert!(settings.repos.is_empty() && settings.groups.is_empty());

    let repo = fake_repo(&temp, "My Repo");
    let (record, saved) = service.repo_add(&repo, None).unwrap();
    assert_eq!(record.id, "my-repo");
    assert_eq!(record.name, "My Repo");
    assert_eq!(saved.revision, 2);
    assert_eq!(saved.repos.len(), 1);

    // 重启读回一致。
    let reopened = SettingsService::open(&temp).unwrap();
    assert!(reopened.warnings.is_empty());
    assert_eq!(reopened.service.get(), saved);

    fs::remove_dir_all(temp).unwrap();
}

#[test]
fn save_requires_expected_revision_and_persists_editable_fields() {
    let temp = unique_temp_dir("gitaview_n06_save");
    let service = open(&temp);
    let repo = fake_repo(&temp, "repo");
    service.repo_add(&repo, None).unwrap();
    let current = service.get();

    // 过期 revision → 冲突。
    let stale = SettingsSaveInput {
        expected_revision: current.revision - 1,
        settings: editable(vec![group("g1", "后端")]),
    };
    assert_eq!(service.save(stale), Err(CoreError::SettingsConflict));

    // 正确 revision：建组、归属、默认组、刷新偏好。
    let input = SettingsSaveInput {
        expected_revision: current.revision,
        settings: SettingsEditable {
            groups: vec![group("g1", "后端"), group("g2", "前端")],
            default_group_id: Some("g1".into()),
            refresh: RefreshPolicy {
                enabled: false,
                interval_minutes: 30,
            },
            repo_groups: vec![RepoGroupAssignment {
                repo_id: "repo".into(),
                group_id: Some("g2".into()),
            }],
        },
    };
    let saved = service.save(input).unwrap();
    assert_eq!(saved.revision, current.revision + 1);
    assert_eq!(saved.default_group_id.as_deref(), Some("g1"));
    assert_eq!(saved.repos[0].group_id.as_deref(), Some("g2"));
    assert!(!saved.refresh.enabled);
    assert_eq!(saved.refresh.interval_minutes, 30);

    let reopened = SettingsService::open(&temp).unwrap();
    assert_eq!(reopened.service.get(), saved);
    fs::remove_dir_all(temp).unwrap();
}

#[test]
fn write_failure_keeps_memory_revision_and_disk_untouched() {
    let temp = unique_temp_dir("gitaview_n06_write_fail");
    // 先用真实落盘建立基线。
    let baseline = open(&temp);
    let repo = fake_repo(&temp, "repo");
    baseline.repo_add(&repo, None).unwrap();
    let before = baseline.get();
    assert_eq!(before.revision, 2);

    // 注入失败落盘器重新打开：后续任何变更都无法持久化。
    let opened = SettingsService::open_with(&temp, Arc::new(FailingPersist))
        .unwrap()
        .service;
    assert_eq!(opened.get(), before);

    let repo2 = fake_repo(&temp, "repo2");
    // 落盘失败：添加与保存都返回 SETTINGS_IO。
    assert!(matches!(
        opened.repo_add(&repo2, None),
        Err(CoreError::SettingsIo)
    ));
    let input = SettingsSaveInput {
        expected_revision: before.revision,
        settings: editable(vec![group("g1", "后端")]),
    };
    assert!(matches!(opened.save(input), Err(CoreError::SettingsIo)));

    // 内存 / revision 不变；磁盘读回仍是失败前版本。
    assert_eq!(opened.get(), before);
    let reopened = SettingsService::open(&temp).unwrap();
    assert_eq!(reopened.service.get(), before);
    fs::remove_dir_all(temp).unwrap();
}

#[test]
fn corrupt_file_is_quarantined_with_recovery_warning() {
    let temp = unique_temp_dir("gitaview_n06_corrupt");
    fs::create_dir_all(&temp).unwrap();
    fs::write(temp.join("settings.json"), "{ broken json").unwrap();
    // 隔离只针对 settings.json；同目录其他文件不受影响（也不读旧版数据）。
    fs::write(temp.join("decoy.txt"), "keep me").unwrap();

    let opened = SettingsService::open(&temp).unwrap();

    assert_eq!(
        opened.warnings,
        vec![StartupWarning::SettingsRecovered],
        "UI must be able to show SETTINGS_RECOVERED"
    );
    assert_eq!(
        opened.service.get(),
        gitaview_core::settings::SettingsData::default()
    );
    assert!(!temp.join("settings.json").exists());
    assert!(fs::read_dir(&temp).unwrap().any(|entry| entry
        .unwrap()
        .file_name()
        .to_string_lossy()
        .contains("corrupt")));
    assert_eq!(
        fs::read_to_string(temp.join("decoy.txt")).unwrap(),
        "keep me"
    );

    // 恢复后继续可用。
    let repo = fake_repo(&temp, "repo");
    opened.service.repo_add(&repo, None).unwrap();
    let reopened = SettingsService::open(&temp).unwrap();
    assert!(reopened.warnings.is_empty());
    assert_eq!(reopened.service.get().repos.len(), 1);
    fs::remove_dir_all(temp).unwrap();
}

#[test]
fn legacy_schema_counts_as_corrupt_not_migrated() {
    let temp = unique_temp_dir("gitaview_n06_legacy");
    fs::create_dir_all(&temp).unwrap();
    fs::write(
        temp.join("settings.json"),
        r#"{"schemaVersion":1,"revision":9,"repos":[{"id":"x","name":"x","path":"C:/x"}],
            "groups":[],"refresh":{"enabled":true,"intervalMinutes":5},
            "safety":{"confirmPull":true}}"#,
    )
    .unwrap();

    let opened = SettingsService::open(&temp).unwrap();

    // 旧字段不解析、不迁移：隔离 + 恢复默认 + 警告。
    assert_eq!(opened.warnings, vec![StartupWarning::SettingsRecovered]);
    assert_eq!(opened.service.get().revision, 1);
    assert!(opened.service.get().repos.is_empty());
    fs::remove_dir_all(temp).unwrap();
}

fn assert_invalid_encoding_recovers(name: &str, bytes: &[u8]) {
    let temp = unique_temp_dir(name);
    fs::create_dir_all(&temp).unwrap();
    fs::write(temp.join("settings.json"), bytes).unwrap();
    let opened = match SettingsService::open(&temp) {
        Ok(opened) => opened,
        Err(error) => {
            fs::remove_dir_all(&temp).unwrap();
            panic!("encoding corruption must recover, got {error:?}");
        }
    };
    let backups: Vec<_> = fs::read_dir(&temp)
        .unwrap()
        .map(|entry| entry.unwrap().path())
        .collect();
    assert_eq!(opened.warnings, vec![StartupWarning::SettingsRecovered]);
    assert_eq!(
        opened.service.get(),
        gitaview_core::settings::SettingsData::default()
    );
    assert_eq!(backups.len(), 1);
    assert!(backups[0].to_string_lossy().contains(".corrupt-"));
    assert_eq!(fs::read(&backups[0]).unwrap(), bytes);
    assert!(!temp.join("settings.json").exists());

    let saved = opened
        .service
        .save(SettingsSaveInput {
            expected_revision: 1,
            settings: editable(vec![group("recovered", "Recovered")]),
        })
        .unwrap();
    let reopened = SettingsService::open(&temp).unwrap();
    assert!(reopened.warnings.is_empty());
    assert_eq!(reopened.service.get(), saved);
    assert_eq!(fs::read(&backups[0]).unwrap(), bytes);
    fs::remove_dir_all(temp).unwrap();
}

#[test]
fn invalid_utf8_settings_are_quarantined_and_recover() {
    assert_invalid_encoding_recovers("gitaview_settings_invalid_utf8", b"{\xff\xfe}");
}

#[test]
fn utf16_settings_are_quarantined_and_recover() {
    let mut bytes = vec![0xff, 0xfe];
    for unit in "{\"schemaVersion\":1}".encode_utf16() {
        bytes.extend_from_slice(&unit.to_le_bytes());
    }
    assert_invalid_encoding_recovers("gitaview_settings_utf16", &bytes);
}

#[test]
fn unreadable_settings_fails_initialization() {
    let temp = unique_temp_dir("gitaview_n06_unreadable");
    fs::create_dir_all(temp.join("settings.json")).unwrap();

    // settings.json 是目录：读取失败 → 初始化失败，不静默当首次空配置。
    assert!(matches!(
        SettingsService::open(&temp),
        Err(CoreError::SettingsIo)
    ));
    fs::remove_dir_all(temp).unwrap();
}

#[test]
fn repo_add_dedupes_canonical_paths_without_revision_bump() {
    let temp = unique_temp_dir("gitaview_n06_dedupe");
    let service = open(&temp);
    let repo = fake_repo(&temp, "repo");
    let (first, saved) = service.repo_add(&repo, None).unwrap();
    assert_eq!(saved.revision, 2);

    // 大小写不同的写法 → 规范化后命中同一路径。
    let mut variant = repo.clone();
    variant.pop();
    variant.push("REPO");
    let (again, unchanged) = service.repo_add(&variant, None).unwrap();
    assert_eq!(again.id, first.id);
    assert_eq!(
        unchanged.revision, 2,
        "duplicate path must not bump revision"
    );
    assert_eq!(unchanged.repos.len(), 1);

    // 同名目录（不同路径）→ 追加 -2 后缀 ID。
    let sibling_root = temp.join("sibling");
    let sibling = fake_repo(&sibling_root, "repo");
    let (second, bumped) = service.repo_add(&sibling, None).unwrap();
    assert_eq!(second.id, "repo-2");
    assert_eq!(bumped.revision, 3);
    fs::remove_dir_all(temp).unwrap();
}

#[test]
fn repo_add_validates_group_membership() {
    let temp = unique_temp_dir("gitaview_n06_add_group");
    let service = open(&temp);
    let repo = fake_repo(&temp, "repo");

    assert_eq!(
        service.repo_add(&repo, Some("missing")),
        Err(CoreError::InvalidArgument)
    );

    let saved = service
        .save(SettingsSaveInput {
            expected_revision: service.get().revision,
            settings: editable(vec![group("g1", "后端")]),
        })
        .unwrap();
    assert_eq!(saved.revision, 2);

    let (record, saved) = service.repo_add(&repo, Some("g1")).unwrap();
    assert_eq!(record.group_id.as_deref(), Some("g1"));
    assert_eq!(saved.revision, 3);
    fs::remove_dir_all(temp).unwrap();
}

#[test]
fn repo_remove_respects_busy_lock_and_missing_id() {
    let temp = unique_temp_dir("gitaview_n06_remove");
    let service = open(&temp);
    fake_repo(&temp, "repo");
    fs::create_dir_all(temp.join("path-alias")).unwrap();
    let repo = temp.join("path-alias").join("..").join("repo");
    let (registered, _) = service.repo_add(&repo, None).unwrap();

    let locks = OperationLocks::new();
    let guard = locks.try_acquire(&registered.path_buf()).unwrap();
    assert_eq!(
        service.repo_remove("repo", &locks),
        Err(CoreError::RepoBusy),
        "network operation in flight must block removal"
    );
    drop(guard);

    let removed = service.repo_remove("repo", &locks).unwrap();
    assert!(removed.repos.is_empty());
    assert_eq!(
        service.repo_remove("repo", &locks),
        Err(CoreError::RepoNotFound)
    );
    // 已删除的仓库在重启后不复活。
    let reopened = SettingsService::open(&temp).unwrap();
    assert!(reopened.service.get().repos.is_empty());
    fs::remove_dir_all(temp).unwrap();
}

#[test]
fn group_rules_normalize_deletions_and_default_group() {
    let temp = unique_temp_dir("gitaview_n06_groups");
    let service = open(&temp);
    let repo = fake_repo(&temp, "repo");
    service.repo_add(&repo, None).unwrap();
    service
        .save(SettingsSaveInput {
            expected_revision: service.get().revision,
            settings: SettingsEditable {
                groups: vec![group("g1", "后端")],
                default_group_id: Some("g1".into()),
                refresh: RefreshPolicy::default(),
                repo_groups: vec![RepoGroupAssignment {
                    repo_id: "repo".into(),
                    group_id: Some("g1".into()),
                }],
            },
        })
        .unwrap();

    // 删除分组：成员归未分组，默认分组归回全部；悬挂归属也归 null。
    let saved = service
        .save(SettingsSaveInput {
            expected_revision: service.get().revision,
            settings: SettingsEditable {
                groups: vec![],
                default_group_id: Some("g1".into()),
                refresh: RefreshPolicy::default(),
                repo_groups: vec![RepoGroupAssignment {
                    repo_id: "repo".into(),
                    group_id: Some("g1".into()),
                }],
            },
        })
        .unwrap();
    assert!(saved.groups.is_empty());
    assert_eq!(saved.default_group_id, None);
    assert_eq!(saved.repos[0].group_id, None);

    // 组名唯一且非空；ID 唯一。
    for bad in [
        vec![group("a", "x"), group("b", "x")],
        vec![group("a", "")],
        vec![group("  ", "x")],
        vec![group("a", "x"), group("a", "y")],
    ] {
        assert_eq!(
            service.save(SettingsSaveInput {
                expected_revision: service.get().revision,
                settings: editable(bad),
            }),
            Err(CoreError::InvalidArgument)
        );
    }
    fs::remove_dir_all(temp).unwrap();
}

#[test]
fn save_rejects_unknown_repo_and_refresh_out_of_range() {
    let temp = unique_temp_dir("gitaview_n06_validate");
    let service = open(&temp);

    // 未知仓库的归属 → 不能通过 save 偷渡增删仓库。
    assert_eq!(
        service.save(SettingsSaveInput {
            expected_revision: 1,
            settings: SettingsEditable {
                groups: vec![],
                default_group_id: None,
                refresh: RefreshPolicy::default(),
                repo_groups: vec![RepoGroupAssignment {
                    repo_id: "ghost".into(),
                    group_id: None,
                }],
            },
        }),
        Err(CoreError::InvalidArgument)
    );

    for interval in [0u32, 61] {
        assert_eq!(
            service.save(SettingsSaveInput {
                expected_revision: 1,
                settings: SettingsEditable {
                    groups: vec![],
                    default_group_id: None,
                    refresh: RefreshPolicy {
                        enabled: true,
                        interval_minutes: interval,
                    },
                    repo_groups: vec![],
                },
            }),
            Err(CoreError::InvalidArgument),
            "interval {interval} must be rejected, not clamped"
        );
    }
    fs::remove_dir_all(temp).unwrap();
}

#[test]
fn concurrent_adds_and_saves_keep_consistent_state() {
    let temp = unique_temp_dir("gitaview_n06_concurrent");
    let service = Arc::new(open(&temp));
    let mut repos = Vec::new();
    for index in 0..6 {
        repos.push(fake_repo(&temp, &format!("repo-{index}")));
    }

    let writer = Arc::clone(&service);
    let add_handles: Vec<_> = repos
        .chunks(2)
        .map(|chunk| {
            let writer = Arc::clone(&writer);
            let chunk = chunk.to_vec();
            std::thread::spawn(move || {
                for repo in chunk {
                    writer.repo_add(&repo, None).unwrap();
                }
            })
        })
        .collect();

    // 同一 expectedRevision 的并发保存：恰好一个成功，其余冲突。
    let expected = 1;
    let save_handles: Vec<_> = (0..4)
        .map(|_| {
            let writer = Arc::clone(&service);
            std::thread::spawn(move || {
                writer
                    .save(SettingsSaveInput {
                        expected_revision: expected,
                        settings: editable(vec![group("g", "组")]),
                    })
                    .map(|saved| saved.revision)
            })
        })
        .collect();

    for handle in add_handles {
        handle.join().unwrap();
    }
    let mut successful_saves = 0;
    let mut conflicts = 0;
    for handle in save_handles {
        match handle.join().unwrap() {
            Ok(_) => successful_saves += 1,
            Err(CoreError::SettingsConflict) => conflicts += 1,
            Err(other) => panic!("unexpected error: {other:?}"),
        }
    }
    // 同一 revision 的并发保存至多一个成功；其余全部冲突。
    // 是否有保存赶在第一次添加之前提交取决于调度，0 或 1 个成功都合法。
    assert!(successful_saves <= 1);
    assert_eq!(successful_saves + conflicts, 4);

    let final_settings = service.get();
    assert_eq!(final_settings.repos.len(), 6, "no concurrent add is lost");
    assert_eq!(final_settings.groups.len(), successful_saves);
    // revision：1 起点 + 6 次添加 + 成功保存次数。
    assert_eq!(final_settings.revision, 7 + successful_saves as u64);

    let reopened = SettingsService::open(&temp).unwrap();
    assert_eq!(reopened.service.get(), final_settings);
    fs::remove_dir_all(temp).unwrap();
}

#[test]
fn concurrent_duplicate_adds_insert_exactly_once() {
    let temp = unique_temp_dir("gitaview_n06_dupe_race");
    let service = Arc::new(open(&temp));
    let repo = fake_repo(&temp, "repo");

    let mut handles = Vec::new();
    for _ in 0..4 {
        let writer = Arc::clone(&service);
        let repo = repo.clone();
        handles.push(std::thread::spawn(move || {
            writer.repo_add(&repo, None).unwrap().0.id
        }));
    }
    let ids: Vec<String> = handles.into_iter().map(|h| h.join().unwrap()).collect();
    assert!(ids.iter().all(|id| id == &ids[0]));
    assert_eq!(service.get().repos.len(), 1);
    fs::remove_dir_all(temp).unwrap();
}

#[test]
fn repeated_mutations_do_not_lose_completed_writes() {
    let temp = unique_temp_dir("gitaview_n06_serial");
    let service = open(&temp);
    let repo = fake_repo(&temp, "repo");
    service.repo_add(&repo, None).unwrap();

    // 交替：保存 → 添加 → 保存 → 移除，全部按最新 revision 提交，无一丢失。
    let saved = service
        .save(SettingsSaveInput {
            expected_revision: service.get().revision,
            settings: editable(vec![group("g1", "一")]),
        })
        .unwrap();
    let repo2 = fake_repo(&temp, "repo2");
    let (_, with_second) = service.repo_add(&repo2, Some("g1")).unwrap();
    let saved2 = service
        .save(SettingsSaveInput {
            expected_revision: with_second.revision,
            settings: SettingsEditable {
                groups: vec![group("g1", "一"), group("g2", "二")],
                default_group_id: Some("g2".into()),
                refresh: RefreshPolicy::default(),
                repo_groups: vec![],
            },
        })
        .unwrap();
    let after_remove = service.repo_remove("repo", &OperationLocks::new()).unwrap();

    assert_eq!(saved.revision + 1, with_second.revision);
    assert_eq!(saved2.groups.len(), 2);
    assert_eq!(after_remove.repos.len(), 1);
    assert_eq!(after_remove.revision, saved2.revision + 1);

    let reopened = SettingsService::open(&temp).unwrap();
    assert_eq!(reopened.service.get(), after_remove);
    fs::remove_dir_all(temp).unwrap();
}

#[test]
fn startup_warnings_are_not_persisted() {
    let temp = unique_temp_dir("gitaview_n06_warning");
    fs::create_dir_all(&temp).unwrap();
    fs::write(temp.join("settings.json"), "garbage").unwrap();

    let opened = SettingsService::open(&temp).unwrap();
    assert!(!opened.warnings.is_empty());
    let repo = fake_repo(&temp, "repo");
    opened.service.repo_add(&repo, None).unwrap();

    // 警告只属于本次启动；重新打开（文件已有效）不再出现。
    let reopened = SettingsService::open(&temp).unwrap();
    assert!(reopened.warnings.is_empty());
    fs::remove_dir_all(temp).unwrap();
}

#[test]
fn mutation_latency_stays_bounded_for_callers() {
    // 写入串行器不阻塞读取：落盘进行中 get() 能立即返回。
    // 用 400ms 假落盘延迟：若 get() 串行等待写入器，添加线程结束后才能读到。
    struct SlowPersist;
    impl SettingsPersist for SlowPersist {
        fn persist(&self, _path: &Path, _text: &str) -> Result<(), CoreError> {
            std::thread::sleep(Duration::from_millis(400));
            Ok(())
        }
    }
    let temp = unique_temp_dir("gitaview_n06_slowwrite");
    let service = Arc::new(
        SettingsService::open_with(&temp, Arc::new(SlowPersist))
            .unwrap()
            .service,
    );
    let repo = fake_repo(&temp, "repo");
    let writer = Arc::clone(&service);
    let adder = std::thread::spawn(move || writer.repo_add(&repo, None).unwrap());
    // 等待写入器进入 persist 阶段（远小于 400ms）。
    std::thread::sleep(Duration::from_millis(50));
    let _snapshot = service.get();
    // 读取返回时写入仍在进行：证明 get() 没有排在写入器后面。
    assert!(
        !adder.is_finished(),
        "get() returned while the writer was still persisting"
    );
    assert!(adder.join().is_ok());
    fs::remove_dir_all(temp).unwrap();
}

// —— Review 回归（R09 / R04）——

fn valid_settings_json(patch: &str) -> String {
    // 结构合法的最小基线；patch 用于注入单个非法业务值。
    format!(
        r#"{{"schemaVersion":1,"revision":1,"repos":[],"groups":[],
             "refresh":{{"enabled":true,"intervalMinutes":5}}{patch}}}"#
    )
}

/// R09：解码成功但业务值非法（未知 schema、超范围刷新、重复 ID、失效引用、
/// revision 0）→ 与坏 JSON 同等对待：隔离 + SETTINGS_RECOVERED + 默认值。
#[test]
fn invalid_business_values_are_quarantined_like_corrupt_files() {
    let cases: Vec<(&str, String)> = vec![
        (
            "schema999",
            valid_settings_json("").replace("1,\"revision\"", "999,\"revision\""),
        ),
        (
            "interval0",
            valid_settings_json("").replace("intervalMinutes\":5", "intervalMinutes\":0"),
        ),
        (
            "interval61",
            valid_settings_json("").replace("intervalMinutes\":5", "intervalMinutes\":61"),
        ),
        (
            "revision0",
            valid_settings_json("").replace(
                "schemaVersion\":1,\"revision\":1",
                "schemaVersion\":1,\"revision\":0",
            ),
        ),
        (
            "duplicate_group_id",
            r#"{"schemaVersion":1,"revision":1,"repos":[],"groups":[
                {"id":"g","name":"A"},{"id":"g","name":"B"}],
                "refresh":{"enabled":true,"intervalMinutes":5}}"#
                .to_string(),
        ),
        (
            "dangling_repo_group",
            r#"{"schemaVersion":1,"revision":1,
                "repos":[{"id":"r","name":"r","path":"/tmp/r","groupId":"gone"}],
                "groups":[],"refresh":{"enabled":true,"intervalMinutes":5}}"#
                .to_string(),
        ),
        (
            "dangling_default_group",
            r#"{"schemaVersion":1,"revision":1,"repos":[],"groups":[],
                "defaultGroupId":"gone","refresh":{"enabled":true,"intervalMinutes":5}}"#
                .to_string(),
        ),
    ];
    for (name, payload) in cases {
        let temp = unique_temp_dir(&format!("gitaview_r09_{name}"));
        fs::create_dir_all(&temp).unwrap();
        fs::write(temp.join("settings.json"), payload).unwrap();
        let opened = SettingsService::open(&temp).unwrap();
        assert_eq!(
            opened.warnings,
            vec![StartupWarning::SettingsRecovered],
            "case {name} must be treated as corrupt"
        );
        let settings = opened.service.get();
        assert!(
            settings.repos.is_empty() && settings.groups.is_empty(),
            "case {name}"
        );
        assert_eq!(settings.revision, 1, "case {name} resets to defaults");
        // 原文件保留为隔离副本，供手动恢复。
        let quarantined = std::fs::read_dir(&temp)
            .unwrap()
            .filter_map(Result::ok)
            .any(|entry| entry.file_name().to_string_lossy().contains("corrupt"));
        assert!(quarantined, "case {name} keeps a quarantine copy");
        fs::remove_dir_all(temp).unwrap();
    }
}

/// R04（服务层）：删除分组提交空 repoGroups 时，未列出仓库的悬挂归属同样清除，
/// 且磁盘与内存一致。
#[test]
fn delete_group_sweeps_ownership_for_repos_not_listed() {
    let temp = unique_temp_dir("gitaview_r04_service_sweep");
    let service = open(&temp);
    service
        .save(SettingsSaveInput {
            expected_revision: 1,
            settings: editable(vec![group("g", "G")]),
        })
        .unwrap();
    let repo_dir = fake_repo(&temp, "r1");
    let (record, saved) = service.repo_add(&repo_dir, Some("g")).unwrap();
    assert_eq!(record.group_id.as_deref(), Some("g"));

    // 真实 UI 删除 payload：groups=[]、repoGroups=[]。
    let after = service
        .save(SettingsSaveInput {
            expected_revision: saved.revision,
            settings: editable(vec![]),
        })
        .unwrap();
    let current = service.get();
    let owned = current
        .repos
        .iter()
        .find(|repo| repo.id == record.id)
        .unwrap();
    assert_eq!(owned.group_id, None);
    assert_eq!(after.repos[0].group_id, None);

    let reopened = SettingsService::open(&temp).unwrap();
    assert!(reopened.warnings.is_empty());
    assert_eq!(reopened.service.get(), current);
    fs::remove_dir_all(temp).unwrap();
}
