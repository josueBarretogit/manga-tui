use crossterm::execute;
use crossterm::terminal::{disable_raw_mode, enable_raw_mode,  EnterAlternateScreen, LeaveAlternateScreen};


pub enum Action {
    Quit,
    Tick,
    ZoomIn,
    ZoomOut,
}

/// Initialize the terminal
pub fn init() -> std::io::Result<()> {
    execute!(std::io::stdout(), EnterAlternateScreen)?;
    enable_raw_mode()?;
    Ok(())
}

pub fn restore() -> std::io::Result<()> {
    execute!(std::io::stdout(), LeaveAlternateScreen)?;
    disable_raw_mode()?;
    Ok(())
}
