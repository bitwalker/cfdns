mod show;
mod sync;

use clap::Subcommand;

use crate::config::Config;

use self::show::Show;
use self::sync::Sync;

/// This trait allows for generically invoking commands
pub trait Command {
    /// Invoking a command executes the logic associated with it.
    ///
    /// A command is not guaranteed to terminate, but if/when it does,
    /// it must return an `anyhow::Result`.
    fn invoke(&self, config: &mut Config) -> anyhow::Result<()>;
}

#[derive(Subcommand)]
pub(crate) enum Commands {
    /// Summarizes the configuration and current synchronization status
    Show(Show),
    /// Synchronize DNS records based on the current configuration
    Sync(Sync),
}

impl Command for Commands {
    #[inline]
    fn invoke(&self, config: &mut Config) -> anyhow::Result<()> {
        match self {
            Self::Show(c) => c.invoke(config),
            Self::Sync(c) => c.invoke(config),
        }
    }
}
