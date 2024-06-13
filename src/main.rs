use ratatui::backend::CrosstermBackend;

use self::backend::tui::{init, init_error_hooks, restore};
use self::view::app::run_app;

mod backend;
/// These would be like the frontend
mod view;

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    init_error_hooks()?;
    init()?;
    run_app(CrosstermBackend::new(std::io::stdout())).await?;
    restore()?;
    Ok(())
}
