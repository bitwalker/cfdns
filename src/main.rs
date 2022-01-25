pub(crate) mod cloudflare;
pub(crate) mod command;
pub(crate) mod config;
pub(crate) mod system;
pub(crate) mod watcher;

use std::path::PathBuf;

use clap::{AppSettings, Parser};

use self::command::{Command, Commands};
use self::config::Config;

#[derive(Parser)]
#[clap(version, about, long_about = None)]
#[clap(name = "cfdns")]
#[clap(bin_name = "cfdns")]
#[clap(setting(AppSettings::SubcommandRequiredElseHelp))]
#[clap(global_setting(AppSettings::DeriveDisplayOrder))]
struct App {
    /// Specify the config file to load
    #[clap(short, long, env, global = true)]
    config: Option<PathBuf>,

    /// Configure logging
    #[clap(short, long, arg_enum, default_value_t, global = true)]
    log: config::LogLevel,

    #[clap(subcommand)]
    command: Commands,
}

fn main() -> anyhow::Result<()> {
    let app = App::parse();

    let mut builder = env_logger::Builder::new();
    builder.filter_level(app.log.into()).parse_env("LOG").init();

    let mut config = match app.config {
        Some(path) => Config::from_path(path.as_path())?,
        None => Config::from_system()?,
    };

    app.command.invoke(&mut config)
}
