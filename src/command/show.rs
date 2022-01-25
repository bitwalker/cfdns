use clap::Args;

use super::Command;
use crate::cloudflare::{DnsContent, DnsRecordType, Id, ProxyMode, Ttl};
use crate::config::Config;
use crate::system::AddressFamily;

#[derive(Args)]
pub struct Show;

#[derive(Debug)]
enum WatcherStatus {
    Disabled,
    Synced,
    OutOfSync,
    Failed,
}

#[derive(Debug)]
enum CloudflareStatus {
    OK,
    Missing,
    TypeMismatch(DnsRecordType),
    OutOfSync,
    Error(String),
}

struct SyncStatus {
    name: String,
    zone: Id,
    ty: DnsRecordType,
    local: DnsContent,
    upstream: Option<DnsContent>,
    status: CloudflareStatus,
    proxied: ProxyMode,
    ttl: Ttl,
}

impl Command for Show {
    fn invoke(&self, config: &mut Config) -> anyhow::Result<()> {
        if config.watchers.is_empty() {
            println!("No watchers configured!");
            return Ok(());
        }

        // Print the current v4 address for each configured interface, alongside other useful info
        for (index, watcher) in config.watchers.iter().enumerate() {
            let name = watcher.interface.name.as_str();
            let interval = watcher.interface.interval;
            let info = &watcher.interface.info;

            // For formatting, start each section with a newline after the first has been printed
            if index > 0 {
                println!();
            }

            println!("[{}]", name);
            if let Some(v4) = info.address(AddressFamily::IPv4) {
                println!("ipv4     = \"{}\"", v4);
            }
            if let Some(v6) = info.address(AddressFamily::IPv6) {
                println!("ipv6     = \"{}\"", v6);
            }
            println!("interval = {}", &interval);

            let mut status = WatcherStatus::Synced;
            let mut records = Vec::new();
            for zone in watcher.watching.iter() {
                for record in zone.records.iter() {
                    let mut sync = SyncStatus {
                        name: record.name.clone(),
                        zone: zone.id.clone(),
                        ty: record.ty,
                        local: record.content.clone(),
                        upstream: None,
                        status: CloudflareStatus::Missing,
                        proxied: ProxyMode::default(),
                        ttl: Ttl::default(),
                    };
                    match watcher.client.get_by_name(&zone.id, &record.name) {
                        Ok(None) => {}
                        Ok(Some(upstream)) => {
                            sync.proxied = upstream.proxied;
                            sync.ttl = upstream.ttl;
                            if sync.ty != upstream.ty {
                                sync.status = CloudflareStatus::TypeMismatch(upstream.ty);
                                sync.upstream = Some(upstream.content);
                            } else if sync.local == upstream.content {
                                sync.status = CloudflareStatus::OK;
                                sync.upstream = Some(upstream.content);
                            } else {
                                sync.status = CloudflareStatus::OutOfSync;
                                sync.upstream = Some(upstream.content);
                            }
                        }
                        Err(e) => {
                            sync.status = CloudflareStatus::Error(format!("{}", &e));
                        }
                    }
                    match &sync.status {
                        CloudflareStatus::Error(_) => {
                            status = WatcherStatus::Failed;
                        }
                        CloudflareStatus::OK => {}
                        _ => {
                            status = WatcherStatus::OutOfSync;
                        }
                    }
                    records.push(sync);
                }
            }
            // If there are no records to sync, the watcher is disabled automatically
            if records.is_empty() {
                status = WatcherStatus::Disabled;
            }
            println!("status   = \"{:?}\"", &status);

            for zone in watcher.watching.iter() {
                if records.is_empty() {
                    continue;
                }
                for record in records.iter().filter(|r| r.zone == zone.id) {
                    println!();
                    println!("[[{}.zones.\"{}\"]]", name, &zone.name);
                    let upstream = record
                        .upstream
                        .as_ref()
                        .map(|content| content.to_string())
                        .unwrap_or_else(|| "N/A".to_string());
                    println!("name      = \"{}\"", &record.name);
                    println!("type      = \"{}\"", &record.ty);
                    println!("local     = \"{}\"", &record.local);
                    println!("upstream  = \"{}\"", &upstream);
                    println!("proxied   = {}", &record.proxied.as_bool());
                    println!("ttl       = {}", &record.ttl);
                    println!("status    = \"{:?}\"", &record.status);
                }
            }
        }

        Ok(())
    }
}
