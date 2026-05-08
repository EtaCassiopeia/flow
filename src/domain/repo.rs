use std::path::PathBuf;

use serde::{Deserialize, Serialize};

use super::ids::{BranchName, RepoFullName};

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum RepoSource {
    Watched,
    Discovered,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Repo {
    pub full_name: RepoFullName,
    pub local_path: PathBuf,
    pub default_branch: BranchName,
    pub source: RepoSource,
}
