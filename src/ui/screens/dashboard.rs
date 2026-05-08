use crossterm::event::{KeyCode, KeyEvent};
use ratatui::layout::{Constraint, Direction, Layout, Rect};
use ratatui::text::{Line, Span};
use ratatui::widgets::{Block, Borders, Paragraph};

use crate::app::AppState;
use crate::msg::{Command, FetchKind};
use crate::ui::Frame;
use crate::ui::screens::Screen;
use crate::ui::theme::Theme;
use crate::ui::widgets::key_hint;

pub fn draw(frame: &mut Frame, area: Rect, theme: &Theme, app: &AppState) {
    let chunks = Layout::default()
        .direction(Direction::Vertical)
        .constraints([Constraint::Length(7), Constraint::Min(1)])
        .split(area);

    let tickets = match &app.data.tickets {
        crate::ui::screens::LoadState::Loaded(v) => v.len().to_string(),
        crate::ui::screens::LoadState::Loading => "…".to_string(),
        crate::ui::screens::LoadState::NotLoaded => "?".to_string(),
        crate::ui::screens::LoadState::Failed(_) => "!".to_string(),
    };
    let prs = match &app.data.prs {
        crate::ui::screens::LoadState::Loaded(v) => v.len().to_string(),
        crate::ui::screens::LoadState::Loading => "…".to_string(),
        crate::ui::screens::LoadState::NotLoaded => "?".to_string(),
        crate::ui::screens::LoadState::Failed(_) => "!".to_string(),
    };
    let worktrees = match &app.data.worktrees {
        crate::ui::screens::LoadState::Loaded(v) => v.len().to_string(),
        crate::ui::screens::LoadState::Loading => "…".to_string(),
        crate::ui::screens::LoadState::NotLoaded => "?".to_string(),
        crate::ui::screens::LoadState::Failed(_) => "!".to_string(),
    };

    let lines = vec![
        Line::from(Span::styled("flow", theme.title())),
        Line::from(Span::styled(
            "your worktree-and-tmux remote control",
            theme.muted_style(),
        )),
        Line::from(""),
        Line::from(format!("  open tickets : {tickets}")),
        Line::from(format!("  open PRs     : {prs}")),
        Line::from(format!("  worktrees    : {worktrees}")),
    ];
    frame.render_widget(
        Paragraph::new(lines).block(Block::default().borders(Borders::ALL).title(" overview ")),
        chunks[0],
    );

    let hints = key_hint::line(
        theme,
        &[
            ("t", "tickets"),
            ("w", "worktrees"),
            ("p", "PRs"),
            ("r", "refresh all"),
            ("?", "help"),
            ("q", "quit"),
        ],
    );
    frame.render_widget(Paragraph::new(vec![hints]), chunks[1]);
}

pub fn handle(key: KeyEvent, app: &mut AppState) -> Option<Command> {
    match key.code {
        KeyCode::Char('t') => {
            app.screen = Screen::Tickets(super::tickets::State::default());
            Some(Command::Refresh {
                kind: FetchKind::Tickets,
                force: false,
            })
        }
        KeyCode::Char('w') => {
            app.screen = Screen::Worktrees(super::worktrees::State::default());
            Some(Command::Refresh {
                kind: FetchKind::Repos,
                force: false,
            })
        }
        KeyCode::Char('p') => {
            app.screen = Screen::Prs(super::prs::State::default());
            Some(Command::Refresh {
                kind: FetchKind::Repos,
                force: false,
            })
        }
        KeyCode::Char('r') => Some(Command::Refresh {
            kind: FetchKind::Tickets,
            force: true,
        }),
        KeyCode::Char('R') => Some(Command::RefreshAll),
        KeyCode::Char('q') => {
            app.should_quit = true;
            None
        }
        _ => None,
    }
}
