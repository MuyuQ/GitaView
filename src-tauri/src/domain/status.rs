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

    pub fn collapsed_bucket(self) -> CollapsedBucket {
        match self {
            RemoteRelation::Error | RemoteRelation::Diverged => CollapsedBucket::NeedsAttention,
            RemoteRelation::LocalAhead | RemoteRelation::RemoteAhead => CollapsedBucket::Syncable,
            RemoteRelation::Synced => CollapsedBucket::Synced,
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

    #[test]
    fn collapsed_bucket_keeps_no_remote_separate() {
        assert_eq!(
            RemoteRelation::NoRemote.collapsed_bucket(),
            CollapsedBucket::NoRemote
        );
        assert_eq!(
            RemoteRelation::LocalAhead.collapsed_bucket(),
            CollapsedBucket::Syncable
        );
        assert_eq!(
            RemoteRelation::RemoteAhead.collapsed_bucket(),
            CollapsedBucket::Syncable
        );
    }

    #[test]
    fn collapsed_bucket_maps_error_to_needs_attention() {
        assert_eq!(
            RemoteRelation::Error.collapsed_bucket(),
            CollapsedBucket::NeedsAttention
        );
        assert_eq!(
            RemoteRelation::Diverged.collapsed_bucket(),
            CollapsedBucket::NeedsAttention
        );
    }
}
