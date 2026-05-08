use serde::{Deserialize, Serialize};

use super::{Repo, Session, Ticket, Worktree};

#[allow(dead_code)]
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct WorkUnit {
    pub ticket: Option<Ticket>,
    pub repo: Repo,
    pub worktree: Worktree,
    pub session: Option<Session>,
}
