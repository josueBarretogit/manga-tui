#![allow(dead_code)]
#![allow(deprecated)]

use clap::Parser;
use ratatui::backend::CrosstermBackend;
use reqwest::StatusCode;

use self::backend::database::Database;
use self::backend::error_log::init_error_hooks;
use self::backend::fetch::{MangadexClient, API_URL_BASE, COVER_IMG_URL_BASE, MANGADEX_CLIENT_INSTANCE};
use self::backend::migration::migrate_version;
use self::backend::tui::{init, restore, run_app};
use self::backend::{build_data_dir, APP_DATA_DIR};
use self::cli::CliArgs;
use self::config::MangaTuiConfig;

mod backend;
mod cli;
mod common;
mod config;
mod global;
mod logger;
mod utils;
mod view;

#[tokio::main(flavor = "multi_thread", worker_threads = 7)]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    let cli_args = CliArgs::parse();

    cli_args.proccess_args()?;

    match build_data_dir() {
        Ok(_) => {},
        Err(e) => {
            eprint!(
            "Data directory could not be created, this is where your manga history and manga downloads is stored
             \n this could be for many reasons such as the application not having enough permissions
            \n Try setting the environment variable `MANGA_TUI_DATA_DIR` to some path pointing to a directory, example: /home/user/somedirectory 
            \n Error details : {e}"
            );
            return Ok(());
        },
    }

    let mangadex_client = MangadexClient::new(API_URL_BASE.parse().unwrap(), COVER_IMG_URL_BASE.parse().unwrap())
        .with_image_quality(MangaTuiConfig::get().image_quality);

    println!("Checking mangadex status...");

    let mangadex_status = mangadex_client.check_status().await;

    match mangadex_status {
        Ok(response) => {
            if response.status() != StatusCode::OK {
                println!("Mangadex appears to be in maintenance, please come back later");
                return Ok(());
            }
        },
        Err(e) => {
            println!("Some error ocurred, more details : {e}");
            return Ok(());
        },
    }

    MANGADEX_CLIENT_INSTANCE.set(mangadex_client).unwrap();

    let mut connection = Database::get_connection()?;
    let database = Database::new(&connection);

    database.setup()?;
    migrate_version(&mut connection)?;

    drop(connection);

    init_error_hooks()?;
    init()?;
    run_app(CrosstermBackend::new(std::io::stdout()), MangadexClient::global().clone()).await?;
    restore()?;
    Ok(())
}
