use clap::{Parser, Subcommand};

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
pub struct CliArgs {
    #[command(subcommand)]
    pub command: Option<Commands>,
}
