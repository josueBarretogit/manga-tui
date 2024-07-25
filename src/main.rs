#![forbid(unsafe_code)]
use clap::Parser;
use once_cell::sync::Lazy;
use ratatui::backend::CrosstermBackend;
use ratatui_image::picker::Picker;
use reqwest::Client;
use strum::IntoEnumIterator;

use self::backend::build_data_dir;
use self::backend::error_log::init_error_hooks;
use self::backend::fetch::{MangadexClient, MANGADEX_CLIENT_INSTANCE};
use self::backend::filter::Languages;
use self::backend::tui::{init, restore, run_app};
use self::cli::CliArgs;
use self::global::PREFERRED_LANGUAGE;

mod utils;

mod backend;
/// These would be like the frontend
mod view;

mod common;

mod cli;

mod global;

pub static PICKER: Lazy<Option<Picker>> = Lazy::new(|| {
    Picker::from_termios().ok().map(|mut picker| {
        picker.guess_protocol();
        picker
    })
});

#[tokio::main(flavor = "multi_thread", worker_threads = 7)]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    let cli_args = CliArgs::parse();

    if let Some(command) = cli_args.command {
        match command {
            cli::Commands::Lang { print, set } => {
                if print {
                    println!("The available languages are:");
                    Languages::iter()
                        .filter(|lang| *lang != Languages::Unkown)
                        .for_each(|lang| {
                            println!(
                                "{} {} argument form : {}",
                                lang.as_emoji(),
                                lang.as_human_readable(),
                                lang.as_param()
                            )
                        });
                    return Ok(());
                }

                match set {
                    Some(lang) => {
                        PREFERRED_LANGUAGE
                            .set(Languages::from(lang.as_str()))
                            .unwrap();
                    }
                    None => PREFERRED_LANGUAGE.set(Languages::default()).unwrap(),
                }
            }
        }
    }

    println!("{}", Languages::get_preferred_lang().as_human_readable());

    match build_data_dir() {
        Ok(_) => {}
        Err(e) => {
            panic!("Data dir could not be created, details : {e}")
        }
    }

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
