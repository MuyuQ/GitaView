//! 契约 fixture 集成测试（N02）：以 contracts/native/v1/fixtures 为唯一期望来源。
//! Rust 序列化 / 投影结果必须与 expectations.json 一致；两端解码测试（N10）复用同一文件。

use gitaview_core::codec::{decode_app_snapshot, decode_widget_snapshot, DecodeError};
use gitaview_core::model::{CompactCounts, WidgetCounts};
use gitaview_core::widget::{project_widget, Staleness};
use serde_json::Value;
use std::collections::BTreeMap;
use std::fs;
use std::path::{Path, PathBuf};

fn contracts_dir() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR")).join("../../contracts/native/v1")
}

fn load_fixture(name: &str) -> Vec<u8> {
    let path = contracts_dir().join("fixtures").join(name);
    fs::read(&path).unwrap_or_else(|error| panic!("read {}: {error}", path.display()))
}

#[derive(serde::Deserialize)]
struct Expectations {
    snapshots: BTreeMap<String, SnapshotExpectation>,
    staleness: BTreeMap<String, Value>,
    rejects: Vec<String>,
}

#[derive(serde::Deserialize)]
#[serde(rename_all = "camelCase")]
struct SnapshotExpectation {
    counts: WidgetCounts,
    compact: CompactCounts,
    attention_ids: Vec<String>,
    #[serde(default)]
    absent_repo_ids: Vec<String>,
    #[serde(default)]
    last_success_at_ms_preserved: Option<u64>,
    #[serde(default)]
    refresh_error_code: Option<String>,
}

#[test]
fn schema_json_parses_and_lists_prototype_definitions() {
    let bytes = fs::read(contracts_dir().join("schema.json")).unwrap();
    let schema: Value = serde_json::from_slice(&bytes).unwrap();
    assert_eq!(
        schema["title"],
        "GitaView native contract v1 (frozen at N09)"
    );
    for definition in [
        "envelope",
        "appSnapshot",
        "widgetSnapshot",
        "probeRequest",
        "probeResult",
        "relation",
        "readState",
        "refreshState",
        "errorObject",
        "repoActions",
        "repoStatus",
        "relationFilter",
        "viewQuery",
        "viewResult",
        "groupCount",
        "settingsData",
        "settingsSaveInput",
        "engineCreateInput",
        "engineCreateData",
        "scanDirectoryInput",
        "scanDirectoryData",
        "repoAddInput",
        "repoAddData",
        "repoRemoveData",
        "repoOperationData",
        "refreshRequestData",
        "snapshotWaitInput",
        "repoTargetInput",
        "repoTargetData",
        "lifecycleData",
    ] {
        assert!(
            schema["definitions"].get(definition).is_some(),
            "schema missing definition {definition}"
        );
    }
}

#[test]
fn every_expected_snapshot_projects_to_declared_counts_and_attention() {
    let expectations: Expectations =
        serde_json::from_slice(&load_fixture("expectations.json")).unwrap();
    assert!(!expectations.snapshots.is_empty());

    for (fixture, expected) in &expectations.snapshots {
        let bytes = load_fixture(&format!("{fixture}.json"));
        let snapshot = decode_app_snapshot(&bytes).unwrap_or_else(|e| panic!("{fixture}: {e:?}"));
        let widget = project_widget(&snapshot);

        assert_eq!(widget.counts, expected.counts, "{fixture} counts");
        assert_eq!(widget.compact, expected.compact, "{fixture} compact");
        let attention: Vec<&str> = widget.attention.iter().map(|r| r.id.as_str()).collect();
        let expected_attention: Vec<&str> =
            expected.attention_ids.iter().map(String::as_str).collect();
        assert_eq!(attention, expected_attention, "{fixture} attention order");

        // 四桶 + pending = total；摘要覆盖全部仓库。
        let bucket_sum = widget.compact.synced
            + widget.compact.syncable
            + widget.compact.needs_attention
            + widget.compact.no_remote
            + widget.counts.pending;
        assert_eq!(bucket_sum, widget.counts.total, "{fixture} bucket sum");

        // 已删仓库不得在任何投影中复活。
        for absent in &expected.absent_repo_ids {
            assert!(
                !snapshot.repos.iter().any(|repo| &repo.id == absent),
                "{fixture}: deleted repo {absent} resurrected in snapshot"
            );
            assert!(
                !widget.attention.iter().any(|repo| &repo.id == absent),
                "{fixture}: deleted repo {absent} resurrected in attention"
            );
        }

        if let Some(preserved) = expected.last_success_at_ms_preserved {
            assert_eq!(
                widget.last_success_at_ms,
                Some(preserved),
                "{fixture} last success time must be preserved"
            );
        }
        if let Some(code) = &expected.refresh_error_code {
            assert_eq!(
                widget.refresh_error_code.as_deref(),
                Some(code.as_str()),
                "{fixture} refresh error code"
            );
        }

        // Widget 投影不泄漏 path、完整远端 URL 或错误消息文本。
        let json = serde_json::to_string(&widget).unwrap();
        assert!(!json.contains("path"), "{fixture} widget leaked path");
        assert!(
            !json.contains("remoteUrl"),
            "{fixture} widget leaked remote url"
        );
        assert!(
            !json.contains("message"),
            "{fixture} widget leaked error message"
        );
    }
}

#[test]
fn staleness_boundaries_follow_expectations() {
    let expectations: Expectations =
        serde_json::from_slice(&load_fixture("expectations.json")).unwrap();

    let manual = &expectations.staleness["snapshot-manual"];
    let snapshot = decode_app_snapshot(&load_fixture("snapshot-manual.json")).unwrap();
    let widget = project_widget(&snapshot);
    assert_eq!(
        widget.staleness(manual["nowMs"].as_u64().unwrap()),
        Staleness::Manual
    );

    let boundary = &expectations.staleness["snapshot-stale-boundary"];
    let snapshot = decode_app_snapshot(&load_fixture("snapshot-stale-boundary.json")).unwrap();
    let widget = project_widget(&snapshot);
    assert_eq!(widget.last_success_at_ms, Some(1790000000000));
    // 等于两倍间隔：尚不标记过期。
    assert_eq!(
        widget.staleness(boundary["freshAtMs"].as_u64().unwrap()),
        Staleness::Fresh
    );
    // 超过边界一分钟：过期。
    assert_eq!(
        widget.staleness(boundary["staleAtMs"].as_u64().unwrap()),
        Staleness::Stale
    );
    // 时间回拨（成功时间在未来）：异常显示，等待刷新。
    assert_eq!(
        widget.staleness(boundary["futureAtMs"].as_u64().unwrap()),
        Staleness::FutureTime
    );
}

#[test]
fn unknown_schema_versions_are_rejected_for_widget_and_app() {
    let expectations: Expectations =
        serde_json::from_slice(&load_fixture("expectations.json")).unwrap();
    assert!(!expectations.rejects.is_empty());
    for fixture in &expectations.rejects {
        let bytes = load_fixture(fixture);
        assert_eq!(
            decode_widget_snapshot(&bytes),
            Err(DecodeError::UnknownSchema),
            "{fixture} must be rejected as unknown schema"
        );
    }
}

#[test]
fn malformed_payloads_are_rejected_not_defaulted() {
    assert_eq!(decode_app_snapshot(b"{}"), Err(DecodeError::Malformed));
    assert_eq!(
        decode_app_snapshot(b"{\"schemaVersion\":1}"),
        Err(DecodeError::Malformed)
    );
    assert_eq!(
        decode_widget_snapshot(b"{\"schemaVersion\":1,\"unknownField\":true}"),
        Err(DecodeError::Malformed)
    );
    // 未知枚举值不能回落为默认关系。
    let mut snapshot: Value = serde_json::from_slice(&load_fixture("snapshot-empty.json")).unwrap();
    snapshot["repos"] = serde_json::json!([{
        "id": "x",
        "name": "x",
        "path": "/x",
        "readState": "ok",
        "relation": "conflicted",
        "branch": "main",
        "hasCommit": true,
        "hasOrigin": true,
        "ahead": 0,
        "behind": 0,
        "actions": { "canOpenDirectory": true, "canOpenRemote": false, "canFetch": false, "canPull": false, "canPush": false }
    }]);
    assert_eq!(
        decode_app_snapshot(serde_json::to_vec(&snapshot).unwrap().as_slice()),
        Err(DecodeError::Malformed)
    );
}
