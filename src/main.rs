#![allow(unused)]
#![allow(dead_code)]
#![allow(deprecated)]
#![allow(clippy::single_match)]

use std::io::stdout;
use std::process::exit;
use std::sync::Arc;
use std::time::Duration;

use backend::cache::Cacher;
use backend::cache::in_memory::InMemoryCache;
use backend::manga_provider::MangaProviders;
use backend::manga_provider::mangadex::filter::MangadexFilterProvider;
use backend::manga_provider::mangadex::filter_widget::MangadexFilterWidget;
use backend::manga_provider::mangadex::{API_URL_BASE, COVER_IMG_URL_BASE, MangadexClient};
use backend::manga_provider::manganato::filter_state::{ManganatoFilterState, ManganatoFiltersProvider};
use backend::manga_provider::manganato::filter_widget::ManganatoFilterWidget;
use backend::manga_provider::manganato::{MANGANATO_BASE_URL, ManganatoProvider};
use backend::manga_provider::weebcentral::filter_state::WeebcentralFiltersProvider;
use backend::manga_provider::weebcentral::filter_widget::WeebcentralFilterWidget;
use backend::manga_provider::weebcentral::{WEEBCENTRAL_BASE_URL, WeebcentralProvider};
use backend::release_notifier::{GITHUB_URL, ReleaseNotifier};
use backend::secrets::anilist::AnilistStorage;
use backend::tracker::anilist::{Anilist, BASE_ANILIST_API_URL};
use clap::Parser;
use crossterm::ExecutableCommand;
use crossterm::event::{DisableMouseCapture, EnableMouseCapture};
use http::StatusCode;
use log::LevelFilter;
use logger::{ILogger, Logger};

use self::backend::build_data_dir;
use self::backend::database::Database;
use self::backend::migration::update_database_with_migrations;
use self::backend::tui::run_app;
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
    let logger = Logger;

    pretty_env_logger::formatted_builder()
        .format_module_path(false)
        .filter_level(LevelFilter::Info)
        .init();

    let cli_args = CliArgs::parse();

    let provider = cli_args.manga_provider;

    cli_args.proccess_args().await?;

    let notifier = ReleaseNotifier::new(GITHUB_URL.parse().unwrap());

    if let Err(e) = notifier.check_new_releases(&logger).await {
        logger.error(e);
    }

    match build_data_dir(&logger) {
        Ok(_) => {},
        Err(e) => {
            logger.error(
            format!(
            "Data directory could not be created, this is where your manga history and manga downloads is stored
             \n this could be for many reasons such as the application not having enough permissions
            \n Try setting the environment variable `MANGA_TUI_DATA_DIR` to some path pointing to a directory, example: /home/user/somedirectory 
            \n Error details : {e}"
        ).into()
            );
            exit(1)
        },
    }

    let anilist_storage = AnilistStorage::new();

    let anilist_client = match anilist_storage.check_credentials_stored() {
        Ok(Some(credentials)) => {
            logger.inform("Anilist is setup, tracking reading history");
            tokio::time::sleep(Duration::from_secs(1)).await;
            Some(
                Anilist::new(BASE_ANILIST_API_URL.parse().unwrap())
                    .with_token(credentials.access_token)
                    .with_client_id(credentials.client_id),
            )
        },
        Err(e) => {
            logger.warn(format!("There is an issue when trying to check for anilist, more details about the error : {e}"));
            None
        },
        _ => None,
    };

    let config = MangaTuiConfig::get();

    let mut connection = Database::get_connection()?;
    let database = Database::new(&connection);

    database.setup()?;
    update_database_with_migrations(&mut connection, &logger)?;

    drop(connection);

    color_eyre::install()?;
    stdout().execute(EnableMouseCapture)?;

    let cache_provider: Arc<dyn Cacher> = InMemoryCache::init(8);

    match provider {
        MangaProviders::Mangadex => {
            let mangadex_client =
                MangadexClient::new(API_URL_BASE.parse().unwrap(), COVER_IMG_URL_BASE.parse().unwrap(), cache_provider)
                    .with_image_quality(config.image_quality);

            logger.inform("Checking mangadex status...");

            let mangadex_status = mangadex_client.check_status().await;

            match mangadex_status {
                Ok(response) => {
                    if response.status() != StatusCode::OK {
                        logger.warn("Mangadex appears to be in maintenance, please come back later");
                        exit(0)
                    }
                },
                Err(e) => {
                    logger.error(format!("Some error ocurred, more details : {e}").into());
                    exit(1)
                },
            }

            run_app(ratatui::init(), mangadex_client, anilist_client, MangadexFilterProvider::new(), MangadexFilterWidget::new())
                .await?;
        },
        MangaProviders::Manganato => {},
        MangaProviders::Weebcentral => {
            logger.inform("Using weebcentral as manga provider");
            tokio::time::sleep(Duration::from_secs(1)).await;
            run_app(
                ratatui::init(),
                WeebcentralProvider::new(WEEBCENTRAL_BASE_URL.parse().unwrap(), cache_provider),
                anilist_client,
                WeebcentralFiltersProvider::new(backend::manga_provider::weebcentral::filter_state::WeebcentralFilterState {}),
                WeebcentralFilterWidget {},
            )
            .await?;
        },
    }
    ratatui::restore();
    stdout().execute(DisableMouseCapture)?;

    Ok(())
}
