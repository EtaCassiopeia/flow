use std::path::PathBuf;

use serde::{Deserialize, Serialize};

use super::ids::{BranchName, RepoFullName, SessionName, TicketKey};

#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct WorktreeStatus {
    pub dirty: bool,
    pub ahead: u32,
    pub behind: u32,
    pub has_upstream: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Worktree {
    pub repo: RepoFullName,
    pub path: PathBuf,
    pub branch: BranchName,
    pub ticket: Option<TicketKey>,
    pub status: WorktreeStatus,
    pub session: Option<SessionName>,
}
