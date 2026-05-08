use serde::{Deserialize, Serialize};
use url::Url;

use super::ids::{BranchName, PrNumber, RepoFullName};

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum PrState {
    Open,
    Closed,
    Merged,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Pr {
    pub repo: RepoFullName,
    pub number: PrNumber,
    pub title: String,
    pub head_ref: BranchName,
    pub author: String,
    pub draft: bool,
    pub state: PrState,
    pub url: Url,
}
