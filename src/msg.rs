#![allow(dead_code)]

use std::path::PathBuf;

use crossterm::event::Event as CtEvent;

use crate::domain::{BranchName, PrNumber, RepoFullName, SessionName, TicketKey};

/// Stable id for an in-flight fetch — used for cancellation routing.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct FetchId(pub u64);

#[derive(Debug, Clone)]
pub enum FetchKind {
    Tickets,
    Repos,
    Prs(RepoFullName),
}

#[derive(Debug)]
pub enum FetchResult {
    Tickets(crate::error::Result<Vec<crate::domain::Ticket>>),
    Repos(crate::error::Result<Vec<crate::domain::Repo>>),
    Prs(
        RepoFullName,
        crate::error::Result<Vec<crate::domain::Pr>>,
    ),
    Worktrees(crate::error::Result<Vec<crate::domain::Worktree>>),
}

#[derive(Debug)]
pub enum AppMsg {
    Input(CtEvent),
    Tick,
    FetchDone(FetchId, FetchResult),
    Command(Command),
    Quit,
}

/// Side-effect requests emitted by screens. Handled by `app::handle_command`.
#[derive(Debug, Clone)]
pub enum Command {
    /// Refresh data. `force=true` cancels any in-flight fetch of the same kind
    /// and bypasses the cache TTL.
    Refresh { kind: FetchKind, force: bool },
    /// Force-refresh every kind ('R' shortcut).
    RefreshAll,
    CreateWorkUnit(CreateWorkUnit),
    AttachSession(SessionName),
    DeleteWorktree {
        repo: RepoFullName,
        path: PathBuf,
        branch: BranchName,
        delete_branch: BranchDeleteMode,
        kill_session: Option<SessionName>,
    },
    CheckoutPr {
        repo: RepoFullName,
        number: PrNumber,
    },
    OpenUrl(String),
}

#[derive(Debug, Clone)]
pub struct CreateWorkUnit {
    pub ticket: Option<TicketKey>,
    pub repo: RepoFullName,
    pub repo_path: PathBuf,
    pub worktree_path: PathBuf,
    pub branch: BranchName,
    pub start_point: Option<BranchName>,
    pub session: SessionName,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum BranchDeleteMode {
    Keep,
    DeleteIfMerged,
    Force,
}
