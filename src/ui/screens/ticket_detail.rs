use crossterm::event::{KeyCode, KeyEvent};
use ratatui::layout::{Constraint, Direction, Layout, Rect};
use ratatui::style::Style;
use ratatui::text::{Line, Span};
use ratatui::widgets::{Block, Borders, List, ListItem, ListState, Paragraph};

use crate::app::AppState;
use crate::domain::{Repo, Ticket};
use crate::msg::Command;
use crate::services::slug;
use crate::ui::Frame;
use crate::ui::screens::{LoadState, Screen};
use crate::ui::theme::Theme;
use crate::ui::widgets::key_hint;

#[derive(Debug)]
pub struct State {
    pub ticket: Ticket,
    pub selected_repo: usize,
}

impl State {
    pub fn new(ticket: Ticket) -> Self {
        Self {
            ticket,
            selected_repo: 0,
        }
    }
}

pub fn draw(frame: &mut Frame, area: Rect, theme: &Theme, state: &State, app: &AppState) {
    let chunks = Layout::default()
        .direction(Direction::Vertical)
        .constraints([Constraint::Length(7), Constraint::Min(1), Constraint::Length(1)])
        .split(area);

    let header = vec![
        Line::from(vec![
            Span::styled(state.ticket.key.as_str().to_string(), theme.title()),
            Span::raw("  "),
            Span::styled(state.ticket.status.label().to_string(), theme.muted_style()),
        ]),
        Line::from(state.ticket.summary.clone()),
        Line::from(Span::styled(
            format!(
                "assignee: {}    updated: {}",
                state.ticket.assignee.as_deref().unwrap_or("?"),
                state.ticket.updated.format("%Y-%m-%d %H:%M")
            ),
            theme.muted_style(),
        )),
        Line::from(Span::styled(state.ticket.url.to_string(), theme.muted_style())),
    ];
    frame.render_widget(
        Paragraph::new(header).block(Block::default().borders(Borders::ALL).title(" ticket ")),
        chunks[0],
    );

    let items: Vec<(Repo, ListItem)> = match &app.data.repos {
        LoadState::Loaded(v) => v
            .iter()
            .map(|r| {
                let line = Line::from(vec![
                    Span::styled(r.full_name.as_str().to_string(), Style::default().fg(theme.accent)),
                    Span::raw("  "),
                    Span::styled(r.local_path.to_string_lossy().into_owned(), theme.muted_style()),
                ]);
                (r.clone(), ListItem::new(line))
            })
            .collect(),
        LoadState::Loading => {
            frame.render_widget(
                Paragraph::new("loading repos…").style(theme.muted_style()),
                chunks[1],
            );
            return;
        }
        LoadState::NotLoaded => {
            frame.render_widget(
                Paragraph::new("no repos loaded").style(theme.muted_style()),
                chunks[1],
            );
            return;
        }
        LoadState::Failed(e) => {
            frame.render_widget(
                Paragraph::new(format!("error: {e}")).style(Style::default().fg(theme.bad)),
                chunks[1],
            );
            return;
        }
    };

    let list_items: Vec<ListItem> = items.iter().map(|(_, i)| i.clone()).collect();
    let mut ls = ListState::default();
    ls.select(if items.is_empty() {
        None
    } else {
        Some(state.selected_repo.min(items.len() - 1))
    });
    let list = List::new(list_items)
        .block(
            Block::default()
                .borders(Borders::ALL)
                .title(" choose a repo "),
        )
        .highlight_style(theme.selected())
        .highlight_symbol("▍ ");
    frame.render_stateful_widget(list, chunks[1], &mut ls);

    let hints = key_hint::line(
        theme,
        &[("j/k", "move"), ("c", "create"), ("o", "open URL"), ("Esc", "back")],
    );
    frame.render_widget(Paragraph::new(vec![hints]), chunks[2]);
}

pub fn handle(key: KeyEvent, state: &mut State, app: &mut AppState) -> Option<Command> {
    let repos: Vec<Repo> = app.data.repos.loaded().cloned().unwrap_or_default();
    match key.code {
        KeyCode::Char('j') | KeyCode::Down => {
            if !repos.is_empty() {
                state.selected_repo = (state.selected_repo + 1).min(repos.len() - 1);
            }
            None
        }
        KeyCode::Char('k') | KeyCode::Up => {
            state.selected_repo = state.selected_repo.saturating_sub(1);
            None
        }
        KeyCode::Char('c') => {
            let Some(repo) = repos.get(state.selected_repo).cloned() else {
                app.toast = Some(crate::ui::widgets::toast::Toast::error(
                    "no repos available — try `r` to refresh",
                ));
                return None;
            };
            let confirm =
                super::confirm_create::State::build(state.ticket.clone(), repo, &app.config)
                    .unwrap_or_else(|e| {
                        super::confirm_create::State::failed(state.ticket.clone(), e.to_string())
                    });
            app.screen = Screen::ConfirmCreate(Box::new(confirm));
            None
        }
        KeyCode::Char('o') => Some(Command::OpenUrl(state.ticket.url.to_string())),
        KeyCode::Esc => {
            app.screen = Screen::Tickets(super::tickets::State::default());
            None
        }
        _ => None,
    }
}

/// Default branch derivation: `<TICKET>-<slug-of-summary>`.
#[allow(dead_code)]
pub fn default_branch_name(ticket: &Ticket) -> String {
    let slug = slug::slugify(&ticket.summary);
    if slug.is_empty() {
        ticket.key.to_string()
    } else {
        format!("{}-{}", ticket.key, slug)
    }
}
