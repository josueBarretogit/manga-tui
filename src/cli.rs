use clap::{crate_version, Parser, Subcommand};
use strum::IntoEnumIterator;

use crate::backend::filter::Languages;

#[derive(Subcommand)]
pub enum Commands {
    Lang {
        #[arg(short, long)]
        print: bool,
        #[arg(short, long)]
        set: Option<String>,
    },
}

#[derive(Parser)]
#[command(version = crate_version!())]
pub struct CliArgs {
    #[command(subcommand)]
    pub command: Option<Commands>,
    #[arg(short, long)]
    pub data_dir: bool,
}

impl CliArgs {
    pub fn print_available_languages() {
        println!("The available languages are:");
        Languages::iter().filter(|lang| *lang != Languages::Unkown).for_each(|lang| {
            println!("{} {} | iso code : {}", lang.as_emoji(), lang.as_human_readable().to_lowercase(), lang.as_iso_code())
        });
    }
}
