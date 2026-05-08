use std::path::PathBuf;

use crossterm::event::{KeyCode, KeyEvent};
use ratatui::layout::{Constraint, Direction, Layout, Rect};
use ratatui::style::Style;
use ratatui::text::{Line, Span};
use ratatui::widgets::{Block, Borders, Paragraph};

use crate::app::AppState;
use crate::config::Config;
use crate::domain::{BranchName, Repo, SessionName, Ticket};
use crate::msg::{Command, CreateWorkUnit};
use crate::services::slug;
use crate::ui::Frame;
use crate::ui::screens::Screen;
use crate::ui::theme::Theme;
use crate::ui::widgets::key_hint;

#[derive(Debug)]
pub struct State {
    pub ticket: Ticket,
    pub repo: Option<Repo>,
    pub plan: Option<Plan>,
    pub error: Option<String>,
}

#[derive(Debug, Clone)]
pub struct Plan {
    pub repo_path: PathBuf,
    pub worktree_path: PathBuf,
    pub branch: BranchName,
    pub start_point: Option<BranchName>,
    pub session: SessionName,
}

impl State {
    pub fn build(ticket: Ticket, repo: Repo, config: &Config) -> Result<Self, String> {
        let summary_slug = slug::slugify(&ticket.summary);
        let branch_str = if summary_slug.is_empty() {
            ticket.key.to_string()
        } else {
            format!("{}-{}", ticket.key, summary_slug)
        };
        let branch = BranchName::new(branch_str.clone()).map_err(|e| e.to_string())?;

        let parent = repo
            .local_path
            .parent()
            .ok_or_else(|| "repo has no parent directory".to_string())?
            .to_path_buf();
        let leaf = format!("{}-{}", repo.full_name.repo(), ticket.key);
        let worktree_path = parent.join(leaf);

        let session_str = render_session_name(&config.tmux.session_template, &ticket, &repo);
        let session = SessionName::new(session_str).map_err(|e| e.to_string())?;

        Ok(Self {
            ticket,
            repo: Some(repo.clone()),
            plan: Some(Plan {
                repo_path: repo.local_path,
                worktree_path,
                branch,
                start_point: Some(repo.default_branch),
                session,
            }),
            error: None,
        })
    }

    pub fn failed(ticket: Ticket, err: String) -> Self {
        Self {
            ticket,
            repo: None,
            plan: None,
            error: Some(err),
        }
    }
}

fn render_session_name(template: &str, ticket: &Ticket, repo: &Repo) -> String {
    let summary_slug = slug::slugify(&ticket.summary);
    let mut out = template.to_string();
    out = out.replace("{ticket}", ticket.key.as_str());
    out = out.replace("{repo}", repo.full_name.repo());
    out = out.replace("{slug}", &summary_slug);
    if out.contains('{') {
        // Template variable left unresolved; fall back to a safe combination.
        return format!("{}-{}", ticket.key, summary_slug.trim_end_matches('-'));
    }
    sanitize_session(&out)
}

fn sanitize_session(s: &str) -> String {
    s.chars()
        .map(|c| if c == '.' || c == ':' { '-' } else { c })
        .filter(|c| !c.is_whitespace())
        .collect()
}

pub fn draw(frame: &mut Frame, area: Rect, theme: &Theme, state: &State) {
    let chunks = Layout::default()
        .direction(Direction::Vertical)
        .constraints([Constraint::Min(1), Constraint::Length(1)])
        .split(area);

    let mut lines = vec![
        Line::from(vec![
            Span::styled("ticket : ", theme.muted_style()),
            Span::styled(state.ticket.key.as_str().to_string(), theme.title()),
            Span::raw("  "),
            Span::raw(state.ticket.summary.clone()),
        ]),
    ];
    if let Some(plan) = &state.plan {
        lines.extend([
            Line::from(vec![
                Span::styled("repo   : ", theme.muted_style()),
                Span::raw(
                    state
                        .repo
                        .as_ref()
                        .map(|r| r.full_name.to_string())
                        .unwrap_or_default(),
                ),
            ]),
            Line::from(vec![
                Span::styled("path   : ", theme.muted_style()),
                Span::raw(plan.worktree_path.to_string_lossy().into_owned()),
            ]),
            Line::from(vec![
                Span::styled("branch : ", theme.muted_style()),
                Span::raw(plan.branch.to_string()),
            ]),
            Line::from(vec![
                Span::styled("from   : ", theme.muted_style()),
                Span::raw(
                    plan.start_point
                        .as_ref()
                        .map(|b| b.to_string())
                        .unwrap_or_else(|| "(current HEAD)".into()),
                ),
            ]),
            Line::from(vec![
                Span::styled("session: ", theme.muted_style()),
                Span::raw(plan.session.to_string()),
            ]),
        ]);
    }
    if let Some(err) = &state.error {
        lines.push(Line::from(""));
        lines.push(Line::from(Span::styled(
            format!("error: {err}"),
            Style::default().fg(theme.bad),
        )));
    }

    frame.render_widget(
        Paragraph::new(lines).block(
            Block::default()
                .borders(Borders::ALL)
                .title(" confirm new work unit "),
        ),
        chunks[0],
    );

    let hints = if state.plan.is_some() {
        key_hint::line(theme, &[("Enter", "create + attach"), ("Esc", "cancel")])
    } else {
        key_hint::line(theme, &[("Esc", "cancel")])
    };
    frame.render_widget(Paragraph::new(vec![hints]), chunks[1]);
}

pub fn handle(key: KeyEvent, state: &mut State, app: &mut AppState) -> Option<Command> {
    match key.code {
        KeyCode::Enter => {
            let plan = state.plan.clone()?;
            let repo = state.repo.clone()?;
            app.screen = Screen::Worktrees(super::worktrees::State::default());
            Some(Command::CreateWorkUnit(CreateWorkUnit {
                ticket: Some(state.ticket.key.clone()),
                repo: repo.full_name,
                repo_path: plan.repo_path,
                worktree_path: plan.worktree_path,
                branch: plan.branch,
                start_point: plan.start_point,
                session: plan.session,
            }))
        }
        KeyCode::Esc => {
            app.screen = Screen::TicketDetail(Box::new(super::ticket_detail::State::new(
                state.ticket.clone(),
            )));
            None
        }
        _ => None,
    }
}
