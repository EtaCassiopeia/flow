use std::time::Instant;

use ratatui::layout::{Alignment, Rect};
use ratatui::style::Style;
use ratatui::widgets::{Block, Borders, Clear, Paragraph};

use crate::ui::Frame;
use crate::ui::theme::Theme;

#[derive(Debug, Clone)]
pub struct Toast {
    pub message: String,
    pub level: ToastLevel,
    pub created_at: Instant,
}

#[allow(dead_code)]
#[derive(Debug, Clone, Copy)]
pub enum ToastLevel {
    Info,
    Warn,
    Error,
}

impl Toast {
    pub fn info(msg: impl Into<String>) -> Self {
        Self {
            message: msg.into(),
            level: ToastLevel::Info,
            created_at: Instant::now(),
        }
    }
    pub fn error(msg: impl Into<String>) -> Self {
        Self {
            message: msg.into(),
            level: ToastLevel::Error,
            created_at: Instant::now(),
        }
    }

    pub fn expired(&self) -> bool {
        self.created_at.elapsed().as_secs() > 5
    }
}

pub fn render(frame: &mut Frame, area: Rect, theme: &Theme, toast: &Toast) {
    if area.height < 3 || area.width < 10 {
        return;
    }
    let w = area.width.saturating_sub(2).min((toast.message.len() + 4) as u16);
    let h = 3u16;
    let popup = Rect {
        x: area.x + area.width.saturating_sub(w + 2),
        y: area.y + area.height.saturating_sub(h + 1),
        width: w,
        height: h,
    };
    frame.render_widget(Clear, popup);
    let style = match toast.level {
        ToastLevel::Info => Style::default().fg(theme.accent),
        ToastLevel::Warn => Style::default().fg(theme.warn),
        ToastLevel::Error => Style::default().fg(theme.bad),
    };
    let block = Block::default().borders(Borders::ALL).border_style(style);
    let para = Paragraph::new(toast.message.clone())
        .block(block)
        .alignment(Alignment::Left);
    frame.render_widget(para, popup);
}
