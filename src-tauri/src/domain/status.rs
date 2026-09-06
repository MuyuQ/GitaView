use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum RemoteRelation {
    Error,
    Diverged,
    RemoteAhead,
    LocalAhead,
    Synced,
    NoRemote,
}

impl RemoteRelation {
    /// 与 serde 序列化的 snake_case 值保持一致（spec §6 的线上取值）
    pub fn as_str(self) -> &'static str {
        match self {
            RemoteRelation::Error => "error",
            RemoteRelation::Diverged => "diverged",
            RemoteRelation::RemoteAhead => "remote_ahead",
            RemoteRelation::LocalAhead => "local_ahead",
            RemoteRelation::Synced => "synced",
            RemoteRelation::NoRemote => "no_remote",
        }
    }

    pub fn sort_rank(self) -> u8 {
        match self {
            RemoteRelation::Error => 0,
            RemoteRelation::Diverged => 1,
            RemoteRelation::RemoteAhead => 2,
            RemoteRelation::LocalAhead => 3,
            RemoteRelation::Synced => 4,
            RemoteRelation::NoRemote => 5,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn sort_rank_places_error_first_and_no_remote_last() {
        let statuses = [
            RemoteRelation::NoRemote,
            RemoteRelation::Synced,
            RemoteRelation::LocalAhead,
            RemoteRelation::Diverged,
            RemoteRelation::RemoteAhead,
            RemoteRelation::Error,
        ];
        let mut sorted = statuses;
        sorted.sort_by_key(|s| s.sort_rank());
        assert_eq!(
            sorted,
            [
                RemoteRelation::Error,
                RemoteRelation::Diverged,
                RemoteRelation::RemoteAhead,
                RemoteRelation::LocalAhead,
                RemoteRelation::Synced,
                RemoteRelation::NoRemote,
            ],
        );
    }
}
