use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use url::Url;

use super::ids::TicketKey;

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum TicketStatus {
    Todo,
    InProgress,
    InReview,
    Done,
    Other(String),
}

impl TicketStatus {
    pub fn from_jira(category: &str, name: &str) -> Self {
        match category {
            "new" | "to-do" => Self::Todo,
            "indeterminate" => match name.to_ascii_lowercase().as_str() {
                "in review" | "code review" => Self::InReview,
                _ => Self::InProgress,
            },
            "done" => Self::Done,
            _ => Self::Other(name.to_string()),
        }
    }

    pub fn label(&self) -> &str {
        match self {
            Self::Todo => "TODO",
            Self::InProgress => "WIP",
            Self::InReview => "REVIEW",
            Self::Done => "DONE",
            Self::Other(s) => s.as_str(),
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Ticket {
    pub key: TicketKey,
    pub summary: String,
    pub status: TicketStatus,
    pub assignee: Option<String>,
    pub url: Url,
    pub updated: DateTime<Utc>,
}
