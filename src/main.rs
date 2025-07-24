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
use backend::manga_provider::mangadex::filter_widget::MangadexFilterWidget;
use backend::manga_provider::mangadex::filters::filter_provider::MangadexFilterProvider;
use backend::manga_provider::mangadex::{API_URL_BASE, COVER_IMG_URL_BASE, MangadexClient};
use backend::manga_provider::weebcentral::filter_state::{WeebcentralFilterState, WeebcentralFiltersProvider};
use backend::manga_provider::weebcentral::filter_widget::WeebcentralFilterWidget;
use backend::manga_provider::weebcentral::{WEEBCENTRAL_BASE_URL, WeebcentralProvider};
use backend::release_notifier::{GITHUB_URL, ReleaseNotifier};
use backend::secrets::keyring::KeyringStorage;
use backend::tracker::anilist::{Anilist, BASE_ANILIST_API_URL};
use clap::Parser;
use cli::{Credentials, check_anilist_credentials_are_stored};
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
use crate::backend::manga_provider::mangadex::get_cached_filters;

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

    let cli_args = CliArgs::parse();

    let manga_provider_cli = cli_args.manga_provider;

    cli_args.proccess_args().await?;

    let config = MangaTuiConfig::get();

    if config.check_new_updates {
        let notifier = ReleaseNotifier::new(GITHUB_URL.parse().unwrap());

        if let Err(e) = notifier.check_new_releases(&logger).await {
            logger.error(e);
        }
    }

    let anilist_storage = KeyringStorage::new();

    let init_anilist = |credentials: Credentials| {
        Anilist::new(BASE_ANILIST_API_URL.parse().unwrap())
            .with_token(credentials.access_token)
            .with_client_id(credentials.client_id)
    };

    let anilist_client = match config.check_anilist_credentials() {
        Some(credentials) => Some(init_anilist(credentials)),
        None => check_anilist_credentials_are_stored(anilist_storage)
            .inspect_err(|e| {
                logger.warn(format!(
                    "There is an issue when trying to check anilist credentials, more details about the error : {e}"
                ));
            })
            .ok()
            .flatten()
            .map(init_anilist),
    }
    .filter(|_| config.track_reading_history);

    if anilist_client.is_some() {
        logger.inform("Anilist is setup, tracking reading history");
        tokio::time::sleep(Duration::from_secs(1)).await;
    }

    let mut connection = Database::get_connection()?;
    let database = Database::new(&connection);

    database.setup()?;
    update_database_with_migrations(&mut connection, &logger)?;

    drop(connection);

    color_eyre::install()?;
    stdout().execute(EnableMouseCapture)?;

    let cache_provider: Arc<dyn Cacher> = InMemoryCache::init(8);

    let provider = if let Some(pro) = manga_provider_cli { pro } else { config.default_manga_provider };

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

            run_app(
                ratatui::init(),
                mangadex_client,
                anilist_client,
                MangadexFilterProvider::from(get_cached_filters()),
                MangadexFilterWidget::new(),
            )
            .await?;
        },
        MangaProviders::Weebcentral => {
            logger.inform("Using Weeb central as manga provider");
            tokio::time::sleep(Duration::from_secs(1)).await;
            run_app(
                ratatui::init(),
                WeebcentralProvider::new(WEEBCENTRAL_BASE_URL.parse().unwrap(), cache_provider),
                anilist_client,
                WeebcentralFiltersProvider::new(WeebcentralFilterState::default()),
                WeebcentralFilterWidget::new(),
            )
            .await?;
        },
    }
    ratatui::restore();
    stdout().execute(DisableMouseCapture)?;

    Ok(())
}
