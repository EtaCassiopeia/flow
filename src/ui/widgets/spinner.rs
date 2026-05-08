const FRAMES: &[&str] = &["⠋", "⠙", "⠹", "⠸", "⠼", "⠴", "⠦", "⠧", "⠇", "⠏"];

pub fn frame(tick: u64) -> &'static str {
    FRAMES[(tick as usize) % FRAMES.len()]
}
