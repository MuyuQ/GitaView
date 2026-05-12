use serde::{Deserialize, Serialize};
use std::path::PathBuf;

use super::status::RemoteRelation;

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct RepoRecord {
    pub id: String,
    pub name: String,
    pub path: PathBuf,
    pub group: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct RepoStatusDto {
    pub id: String,
    pub name: String,
    pub path: String,
    pub group: String,
    pub branch: String,
    pub relation: RemoteRelation,
    pub change_label: String,
    pub hint: String,
    pub has_remote: bool,
    pub remote_url: Option<String>,
}
