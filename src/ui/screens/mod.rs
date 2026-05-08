pub mod confirm_create;
pub mod dashboard;
pub mod help;
pub mod prs;
pub mod setup;
pub mod ticket_detail;
pub mod tickets;
pub mod worktrees;

use crate::domain::{Pr, Repo, Ticket, Worktree};

/// Per-fetch lifecycle state.
#[derive(Debug, Clone)]
pub enum LoadState<T> {
    NotLoaded,
    Loading,
    Loaded(T),
    Failed(String),
}

impl<T> LoadState<T> {
    #[allow(dead_code)]
    pub fn is_loading(&self) -> bool {
        matches!(self, Self::Loading)
    }
    pub fn loaded(&self) -> Option<&T> {
        match self {
            Self::Loaded(v) => Some(v),
            _ => None,
        }
    }
}

impl<T> Default for LoadState<T> {
    fn default() -> Self {
        Self::NotLoaded
    }
}

/// Tagged union of every screen's UI state.
#[derive(Debug)]
pub enum Screen {
    Setup(Box<setup::State>),
    Dashboard,
    Tickets(tickets::State),
    TicketDetail(Box<ticket_detail::State>),
    ConfirmCreate(Box<confirm_create::State>),
    Worktrees(worktrees::State),
    Prs(prs::State),
    /// Internal sentinel used while a handler runs. Never drawn or persisted.
    Pending,
}

impl Screen {
    pub fn name(&self) -> &'static str {
        match self {
            Screen::Setup(_) => "SETUP",
            Screen::Dashboard => "DASHBOARD",
            Screen::Tickets(_) => "TICKETS",
            Screen::TicketDetail(_) => "TICKET",
            Screen::ConfirmCreate(_) => "CONFIRM",
            Screen::Worktrees(_) => "WORKTREES",
            Screen::Prs(_) => "PRS",
            Screen::Pending => "",
        }
    }
}

/// Centralised cached fetch results, hung off `AppState`.
#[derive(Default)]
pub struct DataStore {
    pub tickets: LoadState<Vec<Ticket>>,
    pub repos: LoadState<Vec<Repo>>,
    pub worktrees: LoadState<Vec<Worktree>>,
    pub prs: LoadState<Vec<Pr>>,
}
