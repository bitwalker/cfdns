use std::thread;

use clap::Args;
use log::{info, warn};

use crate::config::Config;
use crate::watcher::Watcher;

use super::Command;

#[derive(Args)]
pub struct Sync {
    /// When true, the sync runs in daemon-mode, i.e. indefinitely
    #[clap(short, long)]
    daemon: bool,
    /// Only sync records bound to a specific interface
    #[clap(short, long)]
    interface: Option<String>,
    /// Only sync records with the given name
    #[clap(short, long)]
    record: Option<String>,
}

impl Command for Sync {
    fn invoke(&self, config: &mut Config) -> anyhow::Result<()> {
        if config.watchers.is_empty() {
            warn!("No watchers configured, nothing to do!");
            return Ok(());
        }

        // If not running as a daemon, simply poll each matching watcher once, then terminate
        if !self.daemon {
            info!("Performing a one-time sync");
            for watcher in config.watchers.iter_mut() {
                if should_watch(watcher, self.interface.as_ref(), self.record.as_ref()) {
                    watcher.poll()?;
                } else {
                    info!(
                        "Skipping watcher for {}, no records to sync",
                        &watcher.interface.name
                    );
                }
            }
            return Ok(());
        }

        // Otherwise, we are going to spawn a thread for each watcher
        // Each watcher will poll once, then sleep for its configured interval.
        info!("Starting daemon");

        let mut threads = Vec::new();
        for mut watcher in config.watchers.drain(0..) {
            if !should_watch(&mut watcher, self.interface.as_ref(), self.record.as_ref()) {
                info!(
                    "Skipping watcher for {}, no records to sync",
                    &watcher.interface.name
                );
                continue;
            }
            info!("Starting thread for {} watcher", &watcher.interface.name);
            let handle = thread::spawn(move || {
                let interval = watcher.interface.interval;

                loop {
                    let _ = watcher.poll();
                    thread::sleep(interval.duration());
                }
            });
            threads.push(handle);
        }

        for handle in threads.drain(0..) {
            if let Err(e) = handle.join() {
                std::panic::resume_unwind(e);
            }
        }

        Ok(())
    }
}

fn should_watch(
    watcher: &mut Watcher,
    interface: Option<&String>,
    record: Option<&String>,
) -> bool {
    if let Some(iface) = interface {
        if watcher.interface.name != iface.as_str() {
            return false;
        }
    }

    if let Some(rec) = record {
        let rec = rec.as_str();
        let mut zones = Vec::new();
        for mut zone in watcher.watching.drain(0..) {
            zone.records = zone
                .records
                .drain(0..)
                .filter(|r| r.name == rec)
                .collect::<Vec<_>>();
            if !zone.records.is_empty() {
                zones.push(zone);
            }
        }
        watcher.watching = zones;

        return !watcher.watching.is_empty();
    }

    true
}
