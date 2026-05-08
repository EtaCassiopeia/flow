use ratatui::layout::Rect;
use ratatui::widgets::{Block, Borders, Clear, Paragraph};

use crate::ui::Frame;
use crate::ui::theme::Theme;

pub fn render(frame: &mut Frame, area: Rect, theme: &Theme) {
    let w = (area.width.saturating_sub(8)).min(60);
    let h = (area.height.saturating_sub(4)).min(20);
    let popup = Rect {
        x: area.x + (area.width.saturating_sub(w)) / 2,
        y: area.y + (area.height.saturating_sub(h)) / 2,
        width: w,
        height: h,
    };
    frame.render_widget(Clear, popup);
    let body = "\
Global

  ?        toggle this help
  q        quit
  Esc      back / cancel
  R        force-refresh everything (bypass cache)

Dashboard

  t        tickets
  w        worktrees
  p        PRs

Lists

  j / k    move
  /        filter (where supported)
  r        force-refresh current view
  Enter    activate (open / attach / create)
  o        open URL in browser

Worktrees screen

  d        delete (with branch-lifecycle prompt: m to cycle modes)
  Enter    attach tmux session

Caching

  Stale data shows immediately on screen entry; a background refresh
  swaps in fresh data when ready. `r` and `R` always force a refresh.
";
    frame.render_widget(
        Paragraph::new(body).block(
            Block::default()
                .borders(Borders::ALL)
                .border_style(theme.muted_style())
                .title(" help "),
        ),
        popup,
    );
}
