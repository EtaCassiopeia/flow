use ratatui::style::{Color, Modifier, Style};

#[allow(dead_code)]
#[derive(Debug, Clone, Copy)]
pub struct Theme {
    pub bg: Color,
    pub fg: Color,
    pub muted: Color,
    pub accent: Color,
    pub good: Color,
    pub warn: Color,
    pub bad: Color,
    pub selection_bg: Color,
}

impl Theme {
    pub fn dark() -> Self {
        Self {
            bg: Color::Reset,
            fg: Color::Reset,
            muted: Color::DarkGray,
            accent: Color::Cyan,
            good: Color::Green,
            warn: Color::Yellow,
            bad: Color::Red,
            selection_bg: Color::Indexed(238),
        }
    }

    pub fn title(&self) -> Style {
        Style::default()
            .fg(self.accent)
            .add_modifier(Modifier::BOLD)
    }

    pub fn selected(&self) -> Style {
        Style::default()
            .bg(self.selection_bg)
            .add_modifier(Modifier::BOLD)
    }

    pub fn muted_style(&self) -> Style {
        Style::default().fg(self.muted)
    }
}
