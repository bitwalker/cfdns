use anyhow::anyhow;
use log::{info, warn};

use crate::cloudflare::*;
use crate::config::Interface;
use crate::system::IfConfig;

pub struct Watcher {
    pub client: Cloudflare,
    pub token: String,
    pub interface: Interface,
    pub watching: Vec<Zone>,
}
impl Watcher {
    pub fn new(interface: Interface, token: String) -> anyhow::Result<Self> {
        Ok(Self {
            client: Cloudflare::new(token.clone())?,
            token,
            interface,
            watching: Vec::new(),
        })
    }

    pub fn poll(&mut self) -> anyhow::Result<()> {
        info!("Checking for updates to {}", &self.interface.name);

        // Fetch latest interface info
        let ifconfig = IfConfig::new();
        let info = ifconfig
            .get(&self.interface.name)
            .ok_or_else(|| anyhow!("Unable to load interface '{}'", &self.interface.name))?;

        // Update watcher-local info
        self.interface.info = info.clone();

        // Traverse each watched zone, syncing any records which are changed as a result of the poll
        for zone in self.watching.iter_mut() {
            for record in zone.records.iter_mut() {
                if let Some(addr) = self.interface.info.address(record.ty.try_into().unwrap()) {
                    // If we don't yet know the record identifier, ask Cloudflare for it
                    if record.id.is_none() {
                        info!("Looking up record metadata for {}", &record.name);
                        // Update our local view of the record with data from Cloudflare
                        if let Some(found) = self.client.get(&zone.id, &record.name, record.ty)? {
                            info!(
                                "Found {} record in Cloudflare for {}: {}",
                                &found.ty, &found.name, &found.content
                            );
                            *record = found;
                        } else {
                            info!("No record of {} in Cloudflare", &record.name);
                        }
                    }
                    // Apply the current interface address, and if the content changes, update the record in Cloudflare
                    if record.id.is_some() {
                        if record.try_update(addr)? {
                            info!("Updating {} with new address {}", &record.name, &addr);
                            self.client.update(record)?;
                        } else {
                            info!("{} is up to date!", &record.name);
                        }
                    } else {
                        info!("Creating {} with address {}", &record.name, &addr);
                        // Make sure the record has current content
                        record.content = addr.into();
                        self.client.create(record)?;
                    }
                } else {
                    warn!(
                        "Unable to find interface address for {} of appropriate type for {} record",
                        &record.name, &record.ty
                    );
                }
            }
        }

        info!("Sync for {} is complete!", &self.interface.name);

        Ok(())
    }
}
