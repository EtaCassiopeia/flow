use crossterm::event::{KeyCode, KeyEvent};
use ratatui::layout::{Constraint, Direction, Layout, Rect};
use ratatui::style::Style;
use ratatui::text::{Line, Span};
use ratatui::widgets::{Block, Borders, List, ListItem, ListState, Paragraph};

use crate::app::AppState;
use crate::domain::Pr;
use crate::msg::Command;
use crate::ui::Frame;
use crate::ui::screens::{LoadState, Screen};
use crate::ui::theme::Theme;
use crate::ui::widgets::key_hint;

#[derive(Debug, Default)]
pub struct State {
    pub selected: usize,
}

pub fn draw(frame: &mut Frame, area: Rect, theme: &Theme, state: &State, app: &AppState) {
    let chunks = Layout::default()
        .direction(Direction::Vertical)
        .constraints([Constraint::Min(1), Constraint::Length(1)])
        .split(area);

    let items: Vec<(Pr, ListItem)> = match &app.data.prs {
        LoadState::Loaded(v) => v.iter().map(|p| (p.clone(), pr_item(theme, p))).collect(),
        LoadState::Loading => {
            frame.render_widget(
                Paragraph::new("loading PRs…").style(theme.muted_style()),
                chunks[0],
            );
            return;
        }
        LoadState::NotLoaded => {
            frame.render_widget(
                Paragraph::new("press r to load").style(theme.muted_style()),
                chunks[0],
            );
            return;
        }
        LoadState::Failed(e) => {
            frame.render_widget(
                Paragraph::new(format!("error: {e}")).style(Style::default().fg(theme.bad)),
                chunks[0],
            );
            return;
        }
    };

    let list_items: Vec<ListItem> = items.iter().map(|(_, i)| i.clone()).collect();
    let mut ls = ListState::default();
    ls.select(if items.is_empty() {
        None
    } else {
        Some(state.selected.min(items.len() - 1))
    });
    let list = List::new(list_items)
        .block(
            Block::default()
                .borders(Borders::ALL)
                .title(format!(" open PRs ({}) ", items.len())),
        )
        .highlight_style(theme.selected())
        .highlight_symbol("▍ ");
    frame.render_stateful_widget(list, chunks[0], &mut ls);

    let hints = key_hint::line(
        theme,
        &[
            ("j/k", "move"),
            ("Enter", "checkout as worktree"),
            ("o", "open in browser"),
            ("r", "refresh"),
            ("Esc", "back"),
        ],
    );
    frame.render_widget(Paragraph::new(vec![hints]), chunks[1]);
}

fn pr_item(theme: &Theme, p: &Pr) -> ListItem<'static> {
    let draft = if p.draft { "[draft] " } else { "" };
    let line = Line::from(vec![
        Span::styled(format!("#{:<5}", p.number.0), Style::default().fg(theme.accent)),
        Span::styled(format!("{:<28}", p.repo.to_string()), theme.muted_style()),
        Span::raw(" "),
        Span::styled(draft.to_string(), Style::default().fg(theme.warn)),
        Span::raw(p.title.clone()),
        Span::raw("  "),
        Span::styled(format!("@{}", p.author), theme.muted_style()),
    ]);
    ListItem::new(line)
}

pub fn handle(key: KeyEvent, state: &mut State, app: &mut AppState) -> Option<Command> {
    let prs: Vec<Pr> = app.data.prs.loaded().cloned().unwrap_or_default();
    match key.code {
        KeyCode::Char('j') | KeyCode::Down => {
            if !prs.is_empty() {
                state.selected = (state.selected + 1).min(prs.len() - 1);
            }
            None
        }
        KeyCode::Char('k') | KeyCode::Up => {
            state.selected = state.selected.saturating_sub(1);
            None
        }
        KeyCode::Enter => {
            let p = prs.get(state.selected)?;
            Some(Command::CheckoutPr {
                repo: p.repo.clone(),
                number: p.number,
            })
        }
        KeyCode::Char('o') => {
            let p = prs.get(state.selected)?;
            Some(Command::OpenUrl(p.url.to_string()))
        }
        KeyCode::Char('r') => {
            // refresh requires repo; we'll emit a Refresh that the app turns into per-repo fetches.
            Some(Command::Refresh {
                kind: crate::msg::FetchKind::Repos,
                force: true,
            })
        }
        KeyCode::Esc => {
            app.screen = Screen::Dashboard;
            None
        }
        _ => None,
    }
}
