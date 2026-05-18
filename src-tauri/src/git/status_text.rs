use crate::domain::status::RemoteRelation;
use crate::git::commands::GitBranchState;

pub fn change_label(state: &GitBranchState) -> String {
    match state.relation {
        RemoteRelation::Error => "!".to_string(),
        RemoteRelation::Synced => "✓".to_string(),
        RemoteRelation::LocalAhead => format!("↑ {}", state.ahead),
        RemoteRelation::RemoteAhead => format!("↓ {}", state.behind),
        RemoteRelation::Diverged => format!("⇕ {}", state.ahead + state.behind),
        RemoteRelation::NoRemote => "∅".to_string(),
    }
}

pub fn relation_hint(relation: RemoteRelation) -> &'static str {
    match relation {
        RemoteRelation::Error => "读取失败",
        RemoteRelation::Synced => "无需操作",
        RemoteRelation::LocalAhead => "可 Push",
        RemoteRelation::RemoteAhead => "可 Pull",
        RemoteRelation::Diverged => "需要人工处理",
        RemoteRelation::NoRemote => "未配置远端",
    }
}

pub fn state_hint(state: &GitBranchState) -> String {
    if state.relation == RemoteRelation::NoRemote {
        if state.has_remote {
            "未设置可比较的 upstream".to_string()
        } else {
            "未配置远端".to_string()
        }
    } else {
        relation_hint(state.relation).to_string()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn sample_state(relation: RemoteRelation) -> GitBranchState {
        GitBranchState {
            branch: "main".to_string(),
            relation,
            ahead: 0,
            behind: 0,
            has_remote: true,
            remote_url: None,
        }
    }

    #[test]
    fn formats_change_labels_for_all_relations() {
        let mut state = sample_state(RemoteRelation::LocalAhead);
        state.ahead = 2;
        assert_eq!(change_label(&state), "↑ 2");

        state.relation = RemoteRelation::RemoteAhead;
        state.ahead = 0;
        state.behind = 3;
        assert_eq!(change_label(&state), "↓ 3");

        state.relation = RemoteRelation::Diverged;
        state.ahead = 2;
        state.behind = 3;
        assert_eq!(change_label(&state), "⇕ 5");
    }

    #[test]
    fn state_hint_distinguishes_missing_remote_from_missing_upstream() {
        let mut state = sample_state(RemoteRelation::NoRemote);
        state.has_remote = false;
        assert_eq!(state_hint(&state), "未配置远端");

        state.has_remote = true;
        assert_eq!(state_hint(&state), "未设置可比较的 upstream");
    }
}
