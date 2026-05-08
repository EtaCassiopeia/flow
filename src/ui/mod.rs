pub mod screens;
pub mod theme;
pub mod widgets;

use std::io;

use crossterm::event::{DisableMouseCapture, EnableMouseCapture};
use crossterm::execute;
use crossterm::terminal::{
    EnterAlternateScreen, LeaveAlternateScreen, disable_raw_mode, enable_raw_mode,
};
use ratatui::Terminal;
use ratatui::backend::CrosstermBackend;

pub type Backend = CrosstermBackend<io::Stdout>;
pub type Frame<'a> = ratatui::Frame<'a>;

/// Owns the alt-screen + raw-mode lifecycle. Drop or `restore()` will always
/// return the terminal to a usable state — this is what the panic hook uses.
pub struct Tui {
    pub terminal: Terminal<Backend>,
    suspended: bool,
}

impl Tui {
    pub fn enter() -> io::Result<Self> {
        enable_raw_mode()?;
        let mut stdout = io::stdout();
        execute!(stdout, EnterAlternateScreen, EnableMouseCapture)?;
        let backend = CrosstermBackend::new(stdout);
        let terminal = Terminal::new(backend)?;
        Ok(Self {
            terminal,
            suspended: false,
        })
    }

    /// Hand the terminal over to a foreground command (typically `tmux attach`).
    /// Restores raw mode + alt-screen on return.
    pub fn suspend_for<F>(&mut self, f: F) -> io::Result<()>
    where
        F: FnOnce() -> io::Result<()>,
    {
        self.leave_screen()?;
        self.suspended = true;
        let result = f();
        self.enter_screen()?;
        self.suspended = false;
        self.terminal.clear()?;
        result
    }

    fn leave_screen(&mut self) -> io::Result<()> {
        let mut stdout = io::stdout();
        execute!(stdout, LeaveAlternateScreen, DisableMouseCapture)?;
        disable_raw_mode()?;
        self.terminal.show_cursor().ok();
        Ok(())
    }

    fn enter_screen(&mut self) -> io::Result<()> {
        enable_raw_mode()?;
        let mut stdout = io::stdout();
        execute!(stdout, EnterAlternateScreen, EnableMouseCapture)?;
        Ok(())
    }

    pub fn restore() -> io::Result<()> {
        let mut stdout = io::stdout();
        execute!(stdout, LeaveAlternateScreen, DisableMouseCapture).ok();
        disable_raw_mode().ok();
        Ok(())
    }
}

impl Drop for Tui {
    fn drop(&mut self) {
        if !self.suspended {
            let _ = Tui::restore();
        }
    }
}

/// Install a panic hook that always restores the terminal before delegating
/// to the previous (default) hook.
pub fn install_panic_hook() {
    let prev = std::panic::take_hook();
    std::panic::set_hook(Box::new(move |info| {
        let _ = Tui::restore();
        prev(info);
    }));
}
