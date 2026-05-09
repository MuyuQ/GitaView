use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum RemoteRelation {
    Synced,
    LocalAhead,
    RemoteAhead,
    Diverged,
    NoRemote,
}

impl RemoteRelation {
    pub fn sort_rank(self) -> u8 {
        match self {
            RemoteRelation::Diverged => 0,
            RemoteRelation::RemoteAhead => 1,
            RemoteRelation::LocalAhead => 2,
            RemoteRelation::Synced => 3,
            RemoteRelation::NoRemote => 4,
        }
    }

    pub fn collapsed_bucket(self) -> CollapsedBucket {
        match self {
            RemoteRelation::Synced => CollapsedBucket::Synced,
            RemoteRelation::LocalAhead | RemoteRelation::RemoteAhead => CollapsedBucket::Syncable,
            RemoteRelation::Diverged => CollapsedBucket::NeedsAttention,
            RemoteRelation::NoRemote => CollapsedBucket::NoRemote,
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum CollapsedBucket {
    Synced,
    Syncable,
    NeedsAttention,
    NoRemote,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn sort_rank_places_no_remote_last() {
        let statuses = [
            RemoteRelation::NoRemote,
            RemoteRelation::Synced,
            RemoteRelation::LocalAhead,
            RemoteRelation::Diverged,
            RemoteRelation::RemoteAhead,
        ];
        let mut sorted = statuses;
        sorted.sort_by_key(|s| s.sort_rank());
        assert_eq!(
            sorted,
            [
                RemoteRelation::Diverged,
                RemoteRelation::RemoteAhead,
                RemoteRelation::LocalAhead,
                RemoteRelation::Synced,
                RemoteRelation::NoRemote,
            ],
        );
    }

    #[test]
    fn collapsed_bucket_keeps_no_remote_separate() {
        assert_eq!(RemoteRelation::NoRemote.collapsed_bucket(), CollapsedBucket::NoRemote);
        assert_eq!(RemoteRelation::LocalAhead.collapsed_bucket(), CollapsedBucket::Syncable);
        assert_eq!(RemoteRelation::RemoteAhead.collapsed_bucket(), CollapsedBucket::Syncable);
    }
}
