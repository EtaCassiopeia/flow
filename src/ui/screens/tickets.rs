use crossterm::event::{KeyCode, KeyEvent};
use ratatui::layout::{Constraint, Direction, Layout, Rect};
use ratatui::style::Style;
use ratatui::text::{Line, Span};
use ratatui::widgets::{Block, Borders, List, ListItem, ListState, Paragraph};

use crate::app::AppState;
use crate::domain::Ticket;
use crate::msg::{Command, FetchKind};
use crate::ui::Frame;
use crate::ui::screens::{LoadState, Screen};
use crate::ui::theme::Theme;
use crate::ui::widgets::key_hint;

#[derive(Debug, Default)]
pub struct State {
    pub selected: usize,
    pub filter: String,
    pub filtering: bool,
}

pub fn draw(frame: &mut Frame, area: Rect, theme: &Theme, state: &State, app: &AppState) {
    let chunks = Layout::default()
        .direction(Direction::Vertical)
        .constraints([
            Constraint::Length(if state.filtering { 3 } else { 0 }),
            Constraint::Min(1),
            Constraint::Length(1),
        ])
        .split(area);

    if state.filtering {
        let p = Paragraph::new(format!("/{}", state.filter))
            .block(Block::default().borders(Borders::ALL).title(" filter "));
        frame.render_widget(p, chunks[0]);
    }

    let items: Vec<(Ticket, ListItem)> = match &app.data.tickets {
        LoadState::Loaded(v) => v
            .iter()
            .filter(|t| matches_filter(t, &state.filter))
            .map(|t| (t.clone(), ticket_to_item(theme, t)))
            .collect(),
        LoadState::Loading => {
            let p = Paragraph::new("loading tickets…").style(theme.muted_style());
            frame.render_widget(p, chunks[1]);
            return;
        }
        LoadState::NotLoaded => {
            let p = Paragraph::new("press r to refresh").style(theme.muted_style());
            frame.render_widget(p, chunks[1]);
            return;
        }
        LoadState::Failed(e) => {
            let p = Paragraph::new(format!("error: {e}")).style(Style::default().fg(theme.bad));
            frame.render_widget(p, chunks[1]);
            return;
        }
    };

    let list_items: Vec<ListItem> = items.iter().map(|(_, i)| i.clone()).collect();
    let mut list_state = ListState::default();
    list_state.select(if items.is_empty() { None } else { Some(state.selected.min(items.len() - 1)) });
    let list = List::new(list_items)
        .block(
            Block::default()
                .borders(Borders::ALL)
                .title(format!(" tickets ({}) ", items.len())),
        )
        .highlight_style(theme.selected())
        .highlight_symbol("▍ ");
    frame.render_stateful_widget(list, chunks[1], &mut list_state);

    let hints = key_hint::line(
        theme,
        &[
            ("j/k", "move"),
            ("Enter", "open"),
            ("/", "filter"),
            ("r", "refresh"),
            ("Esc", "back"),
        ],
    );
    frame.render_widget(Paragraph::new(vec![hints]), chunks[2]);
}

fn matches_filter(t: &Ticket, filter: &str) -> bool {
    if filter.is_empty() {
        return true;
    }
    let f = filter.to_ascii_lowercase();
    t.key.as_str().to_ascii_lowercase().contains(&f)
        || t.summary.to_ascii_lowercase().contains(&f)
}

fn ticket_to_item(theme: &Theme, t: &Ticket) -> ListItem<'static> {
    let status_style = match t.status {
        crate::domain::TicketStatus::Done => Style::default().fg(theme.good),
        crate::domain::TicketStatus::InProgress => Style::default().fg(theme.warn),
        crate::domain::TicketStatus::InReview => Style::default().fg(theme.accent),
        _ => theme.muted_style(),
    };
    let line = Line::from(vec![
        Span::styled(format!("{:<10}", t.key.as_str()), Style::default().fg(theme.accent)),
        Span::styled(
            format!("{:<8}", t.status.label().to_string()),
            status_style,
        ),
        Span::raw(" "),
        Span::raw(t.summary.clone()),
    ]);
    ListItem::new(line)
}

pub fn handle(key: KeyEvent, state: &mut State, app: &mut AppState) -> Option<Command> {
    if state.filtering {
        return handle_filter_mode(key, state);
    }
    match key.code {
        KeyCode::Char('j') | KeyCode::Down => {
            if let Some(v) = visible_tickets(app, state)
                && !v.is_empty()
            {
                state.selected = (state.selected + 1).min(v.len() - 1);
            }
            None
        }
        KeyCode::Char('k') | KeyCode::Up => {
            state.selected = state.selected.saturating_sub(1);
            None
        }
        KeyCode::Enter => {
            if let Some(v) = visible_tickets(app, state) {
                if let Some(t) = v.get(state.selected).cloned() {
                    app.screen =
                        Screen::TicketDetail(Box::new(super::ticket_detail::State::new(t)));
                    return Some(Command::Refresh {
                        kind: crate::msg::FetchKind::Repos,
                        force: false,
                    });
                }
            }
            None
        }
        KeyCode::Char('/') => {
            state.filtering = true;
            state.filter.clear();
            None
        }
        KeyCode::Char('r') => Some(Command::Refresh {
            kind: FetchKind::Tickets,
            force: true,
        }),
        KeyCode::Esc => {
            app.screen = Screen::Dashboard;
            None
        }
        _ => None,
    }
}

fn handle_filter_mode(key: KeyEvent, state: &mut State) -> Option<Command> {
    match key.code {
        KeyCode::Esc => {
            state.filtering = false;
            state.filter.clear();
        }
        KeyCode::Enter => {
            state.filtering = false;
        }
        KeyCode::Backspace => {
            state.filter.pop();
        }
        KeyCode::Char(c) => state.filter.push(c),
        _ => {}
    }
    None
}

fn visible_tickets(app: &AppState, state: &State) -> Option<Vec<Ticket>> {
    let LoadState::Loaded(all) = &app.data.tickets else {
        return None;
    };
    Some(
        all.iter()
            .filter(|t| matches_filter(t, &state.filter))
            .cloned()
            .collect(),
    )
}
