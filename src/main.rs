use ratatui::backend::CrosstermBackend;
use reqwest::Client;

use self::backend::fetch::{MangadexClient, MANGADEX_CLIENT_INSTANCE};
use self::backend::tui::{init, init_error_hooks, restore, run_app};

mod utils;

mod backend;
/// These would be like the frontend
mod view;

#[tokio::main(flavor = "multi_thread", worker_threads = 7)]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    let user_agent = format!(
        "manga-tui/0.beta-testing1.0 ({}/{}/{})",
        std::env::consts::FAMILY,
        std::env::consts::OS,
        std::env::consts::ARCH
    );

    let mangadex_client =
        MangadexClient::new(Client::builder().user_agent(user_agent).build().unwrap());


    MANGADEX_CLIENT_INSTANCE.set(mangadex_client).unwrap();



    init_error_hooks()?;
    init()?;
    run_app(CrosstermBackend::new(std::io::stdout())).await?;
    restore()?;
    Ok(())
}
