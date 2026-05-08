//! Seed data for `--mock` mode: lets you exercise the TUI end-to-end without
//! hitting Jira or GitHub.

use std::sync::{Arc, Mutex};

use chrono::Utc;
use url::Url;

use crate::domain::{
    BranchName, PrNumber, PrState, RepoFullName, Ticket, TicketKey, TicketStatus,
};
use crate::services::github::{MockGithub, RepoSummary};
use crate::services::jira::MockJira;

pub fn mocks() -> (MockJira, MockGithub) {
    let tickets = vec![
        Ticket {
            key: TicketKey::new("PFC-1234").expect("static fixture"),
            summary: "Fix the login screen flicker".into(),
            status: TicketStatus::InProgress,
            assignee: Some("you".into()),
            url: Url::parse("https://example.atlassian.net/browse/PFC-1234").expect("static fixture"),
            updated: Utc::now(),
        },
        Ticket {
            key: TicketKey::new("PFC-1240").expect("static fixture"),
            summary: "Add OAuth callback handling".into(),
            status: TicketStatus::Todo,
            assignee: Some("you".into()),
            url: Url::parse("https://example.atlassian.net/browse/PFC-1240").expect("static fixture"),
            updated: Utc::now(),
        },
        Ticket {
            key: TicketKey::new("PFC-1252").expect("static fixture"),
            summary: "Investigate pager duty alert noise".into(),
            status: TicketStatus::InReview,
            assignee: Some("you".into()),
            url: Url::parse("https://example.atlassian.net/browse/PFC-1252").expect("static fixture"),
            updated: Utc::now(),
        },
    ];
    let repos = vec![
        RepoSummary {
            full_name: RepoFullName::new("pfc/pfc-ledger").expect("static fixture"),
            default_branch: BranchName::new("main").expect("static fixture"),
            pushed_at: Utc::now(),
        },
        RepoSummary {
            full_name: RepoFullName::new("pfc/pfc-edge").expect("static fixture"),
            default_branch: BranchName::new("main").expect("static fixture"),
            pushed_at: Utc::now(),
        },
    ];
    let prs = vec![crate::domain::Pr {
        repo: RepoFullName::new("pfc/pfc-ledger").expect("static fixture"),
        number: PrNumber(45),
        title: "feat: add invoice endpoint".into(),
        head_ref: BranchName::new("PFC-1100-invoice-endpoint").expect("static fixture"),
        author: "alice".into(),
        draft: false,
        state: PrState::Open,
        url: Url::parse("https://github.com/pfc/pfc-ledger/pull/45").expect("static fixture"),
    }];
    (
        MockJira::with_tickets(tickets),
        MockGithub {
            repos: Arc::new(Mutex::new(repos)),
            prs: Arc::new(Mutex::new(prs)),
        },
    )
}
