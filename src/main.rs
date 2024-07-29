#![forbid(unsafe_code)]
use clap::Parser;
use once_cell::sync::Lazy;
use ratatui::backend::CrosstermBackend;
use ratatui_image::picker::{Picker, ProtocolType};
use reqwest::Client;
use strum::IntoEnumIterator;

use self::backend::error_log::init_error_hooks;
use self::backend::fetch::{MangadexClient, MANGADEX_CLIENT_INSTANCE};
use self::backend::filter::Languages;
use self::backend::tui::{init, restore, run_app};
use self::backend::{build_data_dir, APP_DATA_DIR};
use self::cli::CliArgs;
use self::global::PREFERRED_LANGUAGE;

//Todo! check if mangadex is in maintenance
mod utils;

mod backend;
/// These would be like the frontend
mod view;

mod common;

mod cli;

mod global;

#[cfg(unix)]
pub static PICKER: Lazy<Option<Picker>> = Lazy::new(|| {
    Picker::from_termios()
        .ok()
        .map(|mut picker| {
            picker.guess_protocol();
            picker
        })
        .filter(|picker| picker.protocol_type != ProtocolType::Halfblocks)
});

#[cfg(target_os = "windows")]
pub static PICKER: Lazy<Option<Picker>> = Lazy::new(|| {
    // Todo! figure out how to get the size of the terminal on windows
    let mut picker = Picker::new((7, 14));

    let protocol = picker.guess_protocol();

    if protocol == ProtocolType::Halfblocks {
        return None;
    }
    Some(picker)
});

#[tokio::main(flavor = "multi_thread", worker_threads = 7)]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    let cli_args = CliArgs::parse();

    if cli_args.dir {
        let app_dir = APP_DATA_DIR.as_ref().unwrap();
        println!("{}", app_dir.to_str().unwrap());
        return Ok(());
    }

    match cli_args.command {
        Some(command) => match command {
            cli::Commands::Lang { print, set } => {
                if print {
                    println!("The available languages are:");
                    Languages::iter()
                        .filter(|lang| *lang != Languages::Unkown)
                        .for_each(|lang| {
                            println!(
                                "{} {} | iso code : {}",
                                lang.as_emoji(),
                                lang.as_human_readable().to_lowercase(),
                                lang.as_iso_code()
                            )
                        });
                    return Ok(());
                }

                match set {
                    Some(lang) => {
                        let try_lang = Languages::try_from_iso_code(lang.as_str());

                        if try_lang.is_none() {
                            println!("The code : `{}` is not a valid Iso code, run `manga-tui lang --print` to list available languages and their Iso codes", lang);

                            return Ok(());
                        }

                        PREFERRED_LANGUAGE.set(try_lang.unwrap()).unwrap()
                    }
                    None => PREFERRED_LANGUAGE.set(Languages::default()).unwrap(),
                }
            }
        },
        None => PREFERRED_LANGUAGE.set(Languages::default()).unwrap(),
    }

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
