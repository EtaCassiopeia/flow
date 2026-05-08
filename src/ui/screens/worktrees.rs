use crossterm::event::{KeyCode, KeyEvent};
use ratatui::layout::{Constraint, Direction, Layout, Rect};
use ratatui::style::Style;
use ratatui::text::{Line, Span};
use ratatui::widgets::{Block, Borders, List, ListItem, ListState, Paragraph};

use crate::app::AppState;
use crate::domain::Worktree;
use crate::msg::{BranchDeleteMode, Command};
use crate::ui::Frame;
use crate::ui::screens::{LoadState, Screen};
use crate::ui::theme::Theme;
use crate::ui::widgets::key_hint;

#[derive(Debug, Default)]
pub struct State {
    pub selected: usize,
    pub confirming_delete: Option<usize>,
    pub delete_mode: BranchDeleteMode,
}

impl Default for BranchDeleteMode {
    fn default() -> Self {
        BranchDeleteMode::DeleteIfMerged
    }
}

pub fn draw(frame: &mut Frame, area: Rect, theme: &Theme, state: &State, app: &AppState) {
    let chunks = Layout::default()
        .direction(Direction::Vertical)
        .constraints([Constraint::Min(1), Constraint::Length(1)])
        .split(area);

    let items: Vec<(Worktree, ListItem)> = match &app.data.worktrees {
        LoadState::Loaded(v) => v.iter().map(|w| (w.clone(), worktree_item(theme, w))).collect(),
        LoadState::Loading => {
            frame.render_widget(
                Paragraph::new("scanning worktrees…").style(theme.muted_style()),
                chunks[0],
            );
            return;
        }
        LoadState::NotLoaded => {
            frame.render_widget(
                Paragraph::new("press r to scan").style(theme.muted_style()),
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
    let title = if let Some(idx) = state.confirming_delete {
        let mode = match state.delete_mode {
            BranchDeleteMode::Keep => "keep branch",
            BranchDeleteMode::DeleteIfMerged => "delete if merged",
            BranchDeleteMode::Force => "FORCE delete branch",
        };
        format!(
            " worktrees ({}) — confirm delete #{}: {} (Enter) / m to cycle / Esc cancel ",
            items.len(),
            idx + 1,
            mode
        )
    } else {
        format!(" worktrees ({}) ", items.len())
    };
    let list = List::new(list_items)
        .block(Block::default().borders(Borders::ALL).title(title))
        .highlight_style(theme.selected())
        .highlight_symbol("▍ ");
    frame.render_stateful_widget(list, chunks[0], &mut ls);

    let hints = key_hint::line(
        theme,
        &[
            ("j/k", "move"),
            ("Enter", "attach"),
            ("d", "delete"),
            ("r", "refresh"),
            ("Esc", "back"),
        ],
    );
    frame.render_widget(Paragraph::new(vec![hints]), chunks[1]);
}

fn worktree_item(theme: &Theme, w: &Worktree) -> ListItem<'static> {
    let dirty_marker = if w.status.dirty { "●" } else { " " };
    let dirty_style = if w.status.dirty {
        Style::default().fg(theme.warn)
    } else {
        Style::default().fg(theme.good)
    };
    let session_str = w
        .session
        .as_ref()
        .map(|s| format!("[{}]", s))
        .unwrap_or_else(|| "[ no session ]".to_string());
    let session_style = if w.session.is_some() {
        Style::default().fg(theme.accent)
    } else {
        theme.muted_style()
    };
    let track = if w.status.has_upstream {
        format!("↑{} ↓{}", w.status.ahead, w.status.behind)
    } else {
        "no upstream".into()
    };
    let line = Line::from(vec![
        Span::styled(dirty_marker.to_string(), dirty_style),
        Span::raw(" "),
        Span::styled(format!("{:<28}", w.repo.to_string()), Style::default().fg(theme.accent)),
        Span::raw(" "),
        Span::raw(format!("{:<32}", w.branch.to_string())),
        Span::raw(" "),
        Span::styled(format!("{:<14}", track), theme.muted_style()),
        Span::raw(" "),
        Span::styled(session_str, session_style),
    ]);
    ListItem::new(line)
}

pub fn handle(key: KeyEvent, state: &mut State, app: &mut AppState) -> Option<Command> {
    let worktrees: Vec<Worktree> = app.data.worktrees.loaded().cloned().unwrap_or_default();

    if let Some(idx) = state.confirming_delete {
        return handle_confirm(key, idx, state, &worktrees);
    }

    match key.code {
        KeyCode::Char('j') | KeyCode::Down => {
            if !worktrees.is_empty() {
                state.selected = (state.selected + 1).min(worktrees.len() - 1);
            }
            None
        }
        KeyCode::Char('k') | KeyCode::Up => {
            state.selected = state.selected.saturating_sub(1);
            None
        }
        KeyCode::Enter => {
            let w = worktrees.get(state.selected)?;
            let session = w.session.clone()?;
            Some(Command::AttachSession(session))
        }
        KeyCode::Char('d') => {
            if state.selected < worktrees.len() {
                state.confirming_delete = Some(state.selected);
                state.delete_mode = BranchDeleteMode::DeleteIfMerged;
            }
            None
        }
        KeyCode::Char('r') => Some(Command::Refresh {
            kind: crate::msg::FetchKind::Repos,
            force: true,
        }),
        KeyCode::Esc => {
            app.screen = Screen::Dashboard;
            None
        }
        _ => None,
    }
}

fn handle_confirm(
    key: KeyEvent,
    idx: usize,
    state: &mut State,
    worktrees: &[Worktree],
) -> Option<Command> {
    match key.code {
        KeyCode::Char('m') => {
            state.delete_mode = match state.delete_mode {
                BranchDeleteMode::Keep => BranchDeleteMode::DeleteIfMerged,
                BranchDeleteMode::DeleteIfMerged => BranchDeleteMode::Force,
                BranchDeleteMode::Force => BranchDeleteMode::Keep,
            };
            None
        }
        KeyCode::Enter => {
            let w = worktrees.get(idx).cloned()?;
            state.confirming_delete = None;
            Some(Command::DeleteWorktree {
                repo: w.repo,
                path: w.path,
                branch: w.branch,
                delete_branch: state.delete_mode,
                kill_session: w.session,
            })
        }
        KeyCode::Esc => {
            state.confirming_delete = None;
            None
        }
        _ => None,
    }
}
