use crate::domain::repo::RepoStatusDto;
use crate::domain::status::RemoteRelation;

pub const TRAY_REFRESH_ID: &str = "refresh-status";
pub const TRAY_SHOW_ID: &str = "show";
pub const TRAY_QUIT_ID: &str = "quit";

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum TrayMenuRowKind {
    Display,
    Separator,
    Action { id: &'static str },
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct TrayMenuRow {
    pub kind: TrayMenuRowKind,
    pub text: String,
}

impl TrayMenuRow {
    fn display(text: impl Into<String>) -> Self {
        Self {
            kind: TrayMenuRowKind::Display,
            text: text.into(),
        }
    }

    fn separator() -> Self {
        Self {
            kind: TrayMenuRowKind::Separator,
            text: String::new(),
        }
    }

    fn action(id: &'static str, text: impl Into<String>) -> Self {
        Self {
            kind: TrayMenuRowKind::Action { id },
            text: text.into(),
        }
    }
}

pub fn loading_tray_rows() -> Vec<TrayMenuRow> {
    let mut rows = vec![TrayMenuRow::display("正在读取仓库状态...")];
    append_action_rows(&mut rows);
    rows
}

pub fn tray_status_rows(statuses: &[RepoStatusDto]) -> Vec<TrayMenuRow> {
    let mut rows = vec![
        TrayMenuRow::display(format!("GitaView · 共 {} 个仓库", statuses.len())),
        TrayMenuRow::display(format!(
            "已完全同步 {}",
            statuses
                .iter()
                .filter(|status| status.relation == RemoteRelation::Synced)
                .count()
        )),
    ];

    let mut unsynced = statuses
        .iter()
        .filter(|status| status.relation != RemoteRelation::Synced)
        .collect::<Vec<_>>();
    unsynced.sort_by_key(|status| status.relation.sort_rank());

    if !unsynced.is_empty() {
        rows.push(TrayMenuRow::separator());
        rows.extend(
            unsynced
                .into_iter()
                .map(|status| TrayMenuRow::display(status_row_label(status))),
        );
    }

    append_action_rows(&mut rows);
    rows
}

pub fn error_tray_rows(message: &str) -> Vec<TrayMenuRow> {
    let mut rows = vec![
        TrayMenuRow::display("GitaView · 状态读取失败"),
        TrayMenuRow::display(message),
    ];
    append_action_rows(&mut rows);
    rows
}

fn append_action_rows(rows: &mut Vec<TrayMenuRow>) {
    rows.push(TrayMenuRow::separator());
    rows.push(TrayMenuRow::action(TRAY_REFRESH_ID, "刷新状态"));
    rows.push(TrayMenuRow::action(TRAY_SHOW_ID, "显示 GitaView"));
    rows.push(TrayMenuRow::action(TRAY_QUIT_ID, "退出"));
}

fn status_row_label(status: &RepoStatusDto) -> String {
    match status.relation {
        RemoteRelation::Error => {
            format!("读取失败 · {} · {}", status.name, status.hint)
        }
        RemoteRelation::Diverged => format_status_with_change("分叉", status),
        RemoteRelation::RemoteAhead => format_status_with_change("远程领先", status),
        RemoteRelation::LocalAhead => format_status_with_change("本地领先", status),
        RemoteRelation::NoRemote => {
            format!("无远端 · {} · {}", status.name, status.branch)
        }
        RemoteRelation::Synced => {
            format!("已同步 · {} · {}", status.name, status.branch)
        }
    }
}

fn format_status_with_change(prefix: &str, status: &RepoStatusDto) -> String {
    format!(
        "{} · {} · {} · {}",
        prefix, status.name, status.branch, status.change_label
    )
}

#[cfg(test)]
mod tests {
    use super::*;

    fn sample_status(id: &str, relation: RemoteRelation) -> RepoStatusDto {
        RepoStatusDto {
            id: id.to_string(),
            name: id.to_string(),
            path: format!("C:/{id}"),
            group: "全部分组".to_string(),
            branch: "main".to_string(),
            relation,
            change_label: match relation {
                RemoteRelation::Diverged => "↑ 2 ↓ 1".to_string(),
                RemoteRelation::RemoteAhead => "↓ 3".to_string(),
                RemoteRelation::LocalAhead => "↑ 4".to_string(),
                RemoteRelation::Error => "!".to_string(),
                _ => "-".to_string(),
            },
            hint: format!("{id} hint"),
            has_remote: relation != RemoteRelation::NoRemote,
            remote_url: None,
        }
    }

    fn display_texts(rows: &[TrayMenuRow]) -> Vec<&str> {
        rows.iter()
            .filter_map(|row| match row.kind {
                TrayMenuRowKind::Display => Some(row.text.as_str()),
                _ => None,
            })
            .collect()
    }

    #[test]
    fn status_rows_show_summary_and_hide_synced_repositories() {
        let rows = tray_status_rows(&[
            sample_status("synced-repo", RemoteRelation::Synced),
            sample_status("local-repo", RemoteRelation::LocalAhead),
        ]);
        let texts = display_texts(&rows);

        assert!(texts.contains(&"GitaView · 共 2 个仓库"));
        assert!(texts.contains(&"已完全同步 1"));
        assert!(texts
            .iter()
            .any(|text| text.contains("本地领先 · local-repo · main · ↑ 4")));
        assert!(!texts.iter().any(|text| text.contains("synced-repo")));
    }

    #[test]
    fn status_rows_include_every_unsynced_relation_and_keep_no_remote_last() {
        let rows = tray_status_rows(&[
            sample_status("no-remote", RemoteRelation::NoRemote),
            sample_status("local", RemoteRelation::LocalAhead),
            sample_status("remote", RemoteRelation::RemoteAhead),
            sample_status("diverged", RemoteRelation::Diverged),
            sample_status("broken", RemoteRelation::Error),
        ]);
        let texts = display_texts(&rows);

        assert!(texts
            .iter()
            .any(|text| text.contains("读取失败 · broken · broken hint")));
        assert!(texts
            .iter()
            .any(|text| text.contains("分叉 · diverged · main · ↑ 2 ↓ 1")));
        assert!(texts
            .iter()
            .any(|text| text.contains("远程领先 · remote · main · ↓ 3")));
        assert!(texts
            .iter()
            .any(|text| text.contains("本地领先 · local · main · ↑ 4")));
        assert!(texts
            .iter()
            .any(|text| text.contains("无远端 · no-remote · main")));
        assert_eq!(texts.last(), Some(&"无远端 · no-remote · main"));
    }

    #[test]
    fn loading_rows_use_display_item_before_actions() {
        let rows = loading_tray_rows();

        assert_eq!(
            rows.first(),
            Some(&TrayMenuRow::display("正在读取仓库状态..."))
        );
        assert!(rows.iter().any(|row| row.kind
            == (TrayMenuRowKind::Action {
                id: TRAY_REFRESH_ID
            })));
    }

    #[test]
    fn empty_status_rows_still_include_summary_and_actions() {
        let rows = tray_status_rows(&[]);
        let texts = display_texts(&rows);

        assert!(texts.contains(&"GitaView · 共 0 个仓库"));
        assert!(texts.contains(&"已完全同步 0"));
        assert!(rows.iter().any(|row| row.kind
            == (TrayMenuRowKind::Action {
                id: TRAY_REFRESH_ID
            })));
        assert!(rows
            .iter()
            .any(|row| row.kind == (TrayMenuRowKind::Action { id: TRAY_SHOW_ID })));
        assert!(rows
            .iter()
            .any(|row| row.kind == (TrayMenuRowKind::Action { id: TRAY_QUIT_ID })));
    }
}
