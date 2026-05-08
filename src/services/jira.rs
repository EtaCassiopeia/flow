use std::sync::Arc;
use std::sync::Mutex;

use async_trait::async_trait;
use chrono::{DateTime, Utc};
use serde::Deserialize;
use thiserror::Error;
use url::Url;

use crate::domain::{Ticket, TicketKey, TicketStatus};

#[allow(dead_code)]
#[derive(Error, Debug)]
pub enum JiraError {
    #[error("http: {0}")]
    Http(#[from] reqwest::Error),
    #[error("authentication failed (HTTP 401)")]
    Unauthorized,
    #[error("forbidden (HTTP 403)")]
    Forbidden,
    #[error("server error: HTTP {status}")]
    Server { status: u16 },
    #[error("invalid jira host {host:?}")]
    InvalidHost { host: String },
    #[error("invalid issue key: {0}")]
    InvalidKey(String),
    #[error("url: {0}")]
    Url(#[from] url::ParseError),
}

type Result<T> = std::result::Result<T, JiraError>;

#[async_trait]
pub trait JiraClient: Send + Sync {
    async fn search(&self, jql: &str, max: usize) -> Result<Vec<Ticket>>;
    #[allow(dead_code)]
    async fn get(&self, key: &TicketKey) -> Result<Ticket>;
}

pub struct ReqwestJira {
    base: Url,
    email: String,
    token: String,
    http: reqwest::Client,
}

impl ReqwestJira {
    pub fn new(host: &str, email: String, token: String) -> Result<Self> {
        let base = Url::parse(&format!("https://{host}/"))
            .map_err(|_| JiraError::InvalidHost { host: host.into() })?;
        Ok(Self {
            base,
            email,
            token,
            http: reqwest::Client::builder()
                .user_agent("flow/0.1")
                .build()
                .expect("reqwest client"),
        })
    }

    /// Atlassian Cloud uses HTTP Basic auth with `email:api_token` for API tokens.
    fn authed(&self, req: reqwest::RequestBuilder) -> reqwest::RequestBuilder {
        req.basic_auth(&self.email, Some(&self.token))
    }

    fn issue_url(&self, key: &str) -> Result<Url> {
        Ok(self.base.join(&format!("browse/{key}"))?)
    }
}

#[async_trait]
impl JiraClient for ReqwestJira {
    async fn search(&self, jql: &str, max: usize) -> Result<Vec<Ticket>> {
        let url = self.base.join("rest/api/3/search/jql")?;
        let body = serde_json::json!({
            "jql": jql,
            "maxResults": max,
            "fields": ["summary", "status", "assignee", "updated"],
        });
        let resp = self
            .authed(self.http.post(url).json(&body))
            .send()
            .await?;
        check_status(&resp)?;
        let parsed: SearchResponse = resp.json().await?;
        let mut out = Vec::with_capacity(parsed.issues.len());
        for i in parsed.issues {
            if let Some(t) = issue_to_ticket(i, |k| self.issue_url(k).ok()) {
                out.push(t);
            }
        }
        Ok(out)
    }

    async fn get(&self, key: &TicketKey) -> Result<Ticket> {
        let url = self
            .base
            .join(&format!("rest/api/3/issue/{}", key.as_str()))?;
        let resp = self.authed(self.http.get(url)).send().await?;
        check_status(&resp)?;
        let raw: IssueRaw = resp.json().await?;
        issue_to_ticket(raw, |k| self.issue_url(k).ok())
            .ok_or_else(|| JiraError::InvalidKey(key.to_string()))
    }
}

fn check_status(resp: &reqwest::Response) -> Result<()> {
    match resp.status().as_u16() {
        200..=299 => Ok(()),
        401 => Err(JiraError::Unauthorized),
        403 => Err(JiraError::Forbidden),
        s => Err(JiraError::Server { status: s }),
    }
}

#[derive(Deserialize)]
struct SearchResponse {
    #[serde(default)]
    issues: Vec<IssueRaw>,
}

#[derive(Deserialize)]
struct IssueRaw {
    key: String,
    fields: IssueFields,
}

#[derive(Deserialize)]
struct IssueFields {
    summary: Option<String>,
    status: Option<StatusRaw>,
    assignee: Option<AssigneeRaw>,
    updated: Option<String>,
}

#[derive(Deserialize)]
struct StatusRaw {
    name: String,
    #[serde(default, rename = "statusCategory")]
    category: Option<StatusCategoryRaw>,
}

#[derive(Deserialize)]
struct StatusCategoryRaw {
    key: String,
}

#[derive(Deserialize)]
struct AssigneeRaw {
    #[serde(rename = "displayName")]
    display_name: Option<String>,
}

fn issue_to_ticket(raw: IssueRaw, mk_url: impl Fn(&str) -> Option<Url>) -> Option<Ticket> {
    let key = TicketKey::new(&raw.key).ok()?;
    let summary = raw.fields.summary.unwrap_or_default();
    let status = raw
        .fields
        .status
        .map(|s| {
            let cat = s.category.map(|c| c.key).unwrap_or_default();
            TicketStatus::from_jira(&cat, &s.name)
        })
        .unwrap_or(TicketStatus::Other("?".into()));
    let assignee = raw.fields.assignee.and_then(|a| a.display_name);
    let updated = raw
        .fields
        .updated
        .as_deref()
        .and_then(|s| DateTime::parse_from_rfc3339(s).ok())
        .map(|d| d.with_timezone(&Utc))
        .unwrap_or_else(Utc::now);
    let url = mk_url(raw.key.as_str())
        .unwrap_or_else(|| Url::parse("https://example.invalid/").expect("placeholder url"));
    Some(Ticket {
        key,
        summary,
        status,
        assignee,
        url,
        updated,
    })
}

#[derive(Default, Clone)]
pub struct MockJira {
    inner: Arc<Mutex<Vec<Ticket>>>,
}

impl MockJira {
    pub fn with_tickets(tickets: Vec<Ticket>) -> Self {
        Self {
            inner: Arc::new(Mutex::new(tickets)),
        }
    }
}

#[async_trait]
impl JiraClient for MockJira {
    async fn search(&self, _jql: &str, max: usize) -> Result<Vec<Ticket>> {
        let g = self.inner.lock().expect("mock jira lock");
        Ok(g.iter().take(max).cloned().collect())
    }

    async fn get(&self, key: &TicketKey) -> Result<Ticket> {
        let g = self.inner.lock().expect("mock jira lock");
        g.iter()
            .find(|t| t.key == *key)
            .cloned()
            .ok_or_else(|| JiraError::InvalidKey(key.to_string()))
    }
}
