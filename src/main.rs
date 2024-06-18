use ratatui::backend::CrosstermBackend;
use reqwest::Client;

use self::backend::fetch::MangadexClient;
use self::backend::tui::{init, init_error_hooks, restore, run_app};

mod backend;
/// These would be like the frontend
mod view;

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    MangadexClient::new(Client::new())
        .search_mangas("death note")
        .await
        .inspect(|ok| println!("{:#?}", ok))
        .inspect_err(|e| println!("{e}"));
    Ok(())
}
