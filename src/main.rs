use ratatui::backend::CrosstermBackend;

use self::backend::tui::{init, init_error_hooks, restore, run_app};

mod backend;
/// These would be like the frontend
mod view;

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    // id manga for testing 75ee72ab-c6bf-4b87-badd-de839156934c 
    init_error_hooks()?;
    init()?;
    run_app(CrosstermBackend::new(std::io::stdout())).await?;
    restore()?;
    Ok(())
}
