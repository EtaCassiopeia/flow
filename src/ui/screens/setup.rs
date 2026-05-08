use crossterm::event::{KeyCode, KeyEvent};
use ratatui::layout::{Constraint, Direction, Layout, Rect};
use ratatui::style::Style;
use ratatui::text::{Line, Span};
use ratatui::widgets::{Block, Borders, Paragraph};

use crate::app::AppState;
use crate::config::Config;
use crate::msg::Command;
use crate::ui::Frame;
use crate::ui::screens::Screen;
use crate::ui::theme::Theme;
use crate::ui::widgets::key_hint;

#[derive(Debug, Default)]
pub struct State {
    pub field: Field,
    pub code_root: String,
    pub jira_host: String,
    pub jira_email: String,
    pub jira_token: String,
    pub gh_token: String,
    pub error: Option<String>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum Field {
    #[default]
    CodeRoot,
    JiraHost,
    JiraEmail,
    JiraToken,
    GhToken,
    Submit,
}

impl Field {
    fn next(self) -> Self {
        use Field::*;
        match self {
            CodeRoot => JiraHost,
            JiraHost => JiraEmail,
            JiraEmail => JiraToken,
            JiraToken => GhToken,
            GhToken => Submit,
            Submit => Submit,
        }
    }
    fn prev(self) -> Self {
        use Field::*;
        match self {
            CodeRoot => CodeRoot,
            JiraHost => CodeRoot,
            JiraEmail => JiraHost,
            JiraToken => JiraEmail,
            GhToken => JiraToken,
            Submit => GhToken,
        }
    }
}

pub fn initial() -> State {
    State {
        code_root: "~/code".into(),
        ..State::default()
    }
}

pub fn draw(frame: &mut Frame, area: Rect, theme: &Theme, state: &State) {
    let chunks = Layout::default()
        .direction(Direction::Vertical)
        .constraints([
            Constraint::Length(2),
            Constraint::Length(3),
            Constraint::Length(3),
            Constraint::Length(3),
            Constraint::Length(3),
            Constraint::Length(3),
            Constraint::Length(3),
            Constraint::Min(1),
            Constraint::Length(1),
        ])
        .split(area);

    frame.render_widget(
        Paragraph::new(vec![
            Line::from(Span::styled("first-run setup", theme.title())),
            Line::from(Span::styled(
                "tokens stored in macOS Keychain; nothing else is written until you submit.",
                theme.muted_style(),
            )),
        ]),
        chunks[0],
    );

    field(frame, chunks[1], theme, "code root", &state.code_root, state.field == Field::CodeRoot, false);
    field(frame, chunks[2], theme, "jira host", &state.jira_host, state.field == Field::JiraHost, false);
    field(frame, chunks[3], theme, "jira email", &state.jira_email, state.field == Field::JiraEmail, false);
    field(frame, chunks[4], theme, "jira token", &state.jira_token, state.field == Field::JiraToken, true);
    field(frame, chunks[5], theme, "github token", &state.gh_token, state.field == Field::GhToken, true);

    let submit_label = if state.field == Field::Submit {
        Span::styled(" [ submit ] ", theme.selected())
    } else {
        Span::styled(" [ submit ] ", theme.muted_style())
    };
    frame.render_widget(Paragraph::new(Line::from(vec![submit_label])), chunks[6]);

    if let Some(err) = &state.error {
        frame.render_widget(
            Paragraph::new(Line::from(Span::styled(
                format!("error: {err}"),
                Style::default().fg(theme.bad),
            ))),
            chunks[7],
        );
    }

    let hints = key_hint::line(
        theme,
        &[("Tab/Sh-Tab", "next/prev"), ("Enter", "submit"), ("Esc", "quit")],
    );
    frame.render_widget(Paragraph::new(vec![hints]), chunks[8]);
}

fn field(
    frame: &mut Frame,
    area: Rect,
    theme: &Theme,
    label: &str,
    value: &str,
    focused: bool,
    secret: bool,
) {
    let style = if focused { theme.selected() } else { Style::default() };
    let display: String = if secret {
        "•".repeat(value.chars().count())
    } else {
        value.to_string()
    };
    let block = Block::default()
        .borders(Borders::ALL)
        .border_style(if focused {
            Style::default().fg(theme.accent)
        } else {
            theme.muted_style()
        })
        .title(format!(" {label} "));
    frame.render_widget(Paragraph::new(display).style(style).block(block), area);
}

pub fn handle(key: KeyEvent, state: &mut State, app: &mut AppState) -> Option<Command> {
    use Field::*;
    match key.code {
        KeyCode::Tab | KeyCode::Down => {
            state.field = state.field.next();
        }
        KeyCode::BackTab | KeyCode::Up => {
            state.field = state.field.prev();
        }
        KeyCode::Esc => {
            app.should_quit = true;
        }
        KeyCode::Enter => {
            if state.field == Submit {
                match submit(state) {
                    Ok(cfg) => {
                        app.config = cfg;
                        app.screen = Screen::Dashboard;
                    }
                    Err(e) => state.error = Some(e),
                }
            } else {
                state.field = state.field.next();
            }
        }
        KeyCode::Backspace => {
            target_mut(state).pop();
        }
        KeyCode::Char(c) => {
            target_mut(state).push(c);
        }
        _ => {}
    }
    None
}

fn target_mut(state: &mut State) -> &mut String {
    match state.field {
        Field::CodeRoot => &mut state.code_root,
        Field::JiraHost => &mut state.jira_host,
        Field::JiraEmail => &mut state.jira_email,
        Field::JiraToken => &mut state.jira_token,
        Field::GhToken => &mut state.gh_token,
        Field::Submit => {
            // Should not happen; return a scratch buffer.
            &mut state.code_root
        }
    }
}

fn submit(state: &State) -> Result<Config, String> {
    if state.code_root.is_empty() {
        return Err("code_root is required".into());
    }
    if state.jira_host.is_empty() || state.jira_email.is_empty() {
        return Err("jira host and email are required".into());
    }
    if !state.jira_token.is_empty() {
        crate::services::keychain::set(
            crate::services::keychain::Slot::JiraToken {
                email: &state.jira_email,
            },
            &state.jira_token,
        )
        .map_err(|e| format!("keychain (jira): {e}"))?;
    }
    if !state.gh_token.is_empty() {
        crate::services::keychain::set(
            crate::services::keychain::Slot::GithubToken,
            &state.gh_token,
        )
        .map_err(|e| format!("keychain (github): {e}"))?;
    }

    let mut cfg = Config::sample();
    cfg.code_root = state.code_root.clone();
    cfg.jira.host = state.jira_host.clone();
    cfg.jira.email = state.jira_email.clone();
    let path = crate::config::default_path();
    crate::config::save(&path, &cfg).map_err(|e| format!("save config: {e}"))?;
    Ok(cfg)
}
