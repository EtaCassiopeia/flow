use ratatui::layout::Rect;
use ratatui::style::Style;
use ratatui::text::{Line, Span};
use ratatui::widgets::Paragraph;

use crate::ui::Frame;
use crate::ui::theme::Theme;
use crate::ui::widgets::spinner;

pub struct StatusBarProps<'a> {
    pub mode: &'a str,
    pub in_flight: usize,
    pub tick: u64,
    pub right: Option<&'a str>,
}

pub fn render(frame: &mut Frame, area: Rect, theme: &Theme, props: StatusBarProps<'_>) {
    let mut spans: Vec<Span<'static>> = vec![
        Span::styled(
            format!(" {} ", props.mode),
            Style::default().bg(theme.accent).fg(theme.bg),
        ),
        Span::raw(" "),
    ];
    if props.in_flight > 0 {
        spans.push(Span::styled(
            format!("{} {} ", spinner::frame(props.tick), props.in_flight),
            Style::default().fg(theme.warn),
        ));
    }
    if let Some(r) = props.right {
        spans.push(Span::styled(r.to_string(), theme.muted_style()));
    }
    frame.render_widget(Paragraph::new(Line::from(spans)), area);
}
