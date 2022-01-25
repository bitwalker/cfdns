pub mod file;

use std::collections::{HashMap, HashSet};
use std::fmt::{self, Display};
use std::net::IpAddr;
use std::path::Path;

use anyhow::anyhow;
use serde::{Deserialize, Serialize};

use crate::cloudflare::{DnsRecord, Zone};
use crate::system::{AddressFamily, IfConfig, InterfaceInfo};
use crate::watcher::Watcher;

use self::file::ConfigFile;

#[derive(clap::ArgEnum, Copy, Clone, Debug, PartialEq, Eq)]
pub enum LogLevel {
    Off,
    Error,
    Warn,
    Info,
    Debug,
    Trace,
}
impl Default for LogLevel {
    fn default() -> Self {
        Self::Warn
    }
}
#[allow(clippy::from_over_into)]
impl Into<log::LevelFilter> for LogLevel {
    fn into(self) -> log::LevelFilter {
        use log::LevelFilter;
        match self {
            Self::Off => LevelFilter::Off,
            Self::Error => LevelFilter::Error,
            Self::Warn => LevelFilter::Warn,
            Self::Info => LevelFilter::Info,
            Self::Debug => LevelFilter::Debug,
            Self::Trace => LevelFilter::Trace,
        }
    }
}

#[derive(Copy, Clone, Debug, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize)]
#[repr(transparent)]
pub struct Interval(u16);
impl Interval {
    pub fn duration(&self) -> std::time::Duration {
        std::time::Duration::from_secs(self.0 as u64)
    }
}
impl Default for Interval {
    fn default() -> Self {
        Self(300)
    }
}
impl Display for Interval {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        write!(f, "{}", self.0)
    }
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct Interface {
    pub name: String,
    pub interval: Interval,
    #[serde(skip)]
    pub info: InterfaceInfo,
}
impl Interface {
    #[inline]
    pub fn address(&self, ty: AddressFamily) -> Option<IpAddr> {
        self.info.address(ty)
    }
}

pub struct Config {
    pub ifconfig: IfConfig,
    pub file: ConfigFile,
    pub watchers: Vec<Watcher>,
}
impl Default for Config {
    fn default() -> Self {
        Self {
            ifconfig: IfConfig::new(),
            file: ConfigFile::default(),
            watchers: vec![],
        }
    }
}
impl Config {
    pub fn from_path(path: &Path) -> anyhow::Result<Self> {
        self::file::read_from_path(path).and_then(Config::try_from)
    }

    pub fn from_system() -> anyhow::Result<Self> {
        self::file::read_from_system().and_then(Config::try_from)
    }
}
impl TryFrom<file::ConfigFile> for Config {
    type Error = anyhow::Error;

    fn try_from(config: file::ConfigFile) -> Result<Self, Self::Error> {
        let ifconfig = IfConfig::new();
        // For each configured interface, create a watcher that will watch on
        // the configured interval. Each watcher will have one or more zones
        // that use the same Cloudflare API token. Those zones will contain
        // all of the configured DNS records which are bound to an address of
        // the interface being monitored

        // Get all of the unique zones, and load their resource id and metadata
        let zones = {
            // Build a unique list of zone names referenced by records
            let mut zone_names = config
                .records
                .iter()
                .map(|r| r.zone.clone())
                .collect::<HashSet<_>>();
            // To account for zones with no records, we add any zones defined in the configuration
            for zone in config.zones.iter() {
                zone_names.insert(zone.name.clone());
            }

            let mut zones = HashMap::new();
            for zone_name in zone_names.drain() {
                let token = config
                    .zone(&zone_name)
                    .map(|z| z.token.as_str())
                    .ok_or_else(|| anyhow!("Reference to undefined zone '{}'", zone_name))?;
                let zc = config.zone(&zone_name).unwrap();
                // If a zone id was provided, we can skip requesting the zone from Cloudflare
                let zone = if let Some(id) = &zc.id {
                    Zone::new(id.clone(), zone_name.clone())
                } else {
                    Zone::get(&zone_name, token)?
                };
                zones.insert(zone_name, (token, zone));
            }
            zones
        };

        let mut watchers = Vec::<Watcher>::new();
        for mut interface in config.interfaces.iter().cloned() {
            // Get interface info
            let name = interface.name.as_str();
            interface.info = ifconfig
                .get(name)
                .ok_or_else(|| anyhow!("Unable to load interface '{}'", name))?
                .clone();
            // Get all of the records bound to this interface
            let records = config
                .records
                .iter()
                .filter(|r| r.interface == name)
                .collect::<Vec<_>>();
            // We need to uniquify watchers by API token, so while we're looping through zones to add
            // to the watcher, use the token associated with the zone to find the corresponding watcher.
            let mut watchers_by_token = HashMap::<String, Watcher>::new();
            // Build a set of unique zone names
            let zone_names = records
                .iter()
                .map(|r| r.zone.as_str())
                .collect::<HashSet<_>>();
            // For each zone, check if a watcher has been created.
            // If no watcher exists yet, create one, initializing it with the zone with its associated records.
            // Otherwise, append the zone and its records to the existing watcher.
            for zone_name in zone_names {
                // Fetch the zone details and token
                let (token, mut zone) = zones.get(zone_name).unwrap().clone();
                // Construct the expected DNS records for this zone
                for record in records.iter().filter(|r| r.zone == zone_name) {
                    let address_family = record.ty.try_into().unwrap();
                    zone.records.push(DnsRecord {
                        id: None,
                        zone_id: zone.id.clone(),
                        name: record.name.clone(),
                        ty: record.ty,
                        content: interface.address(address_family).unwrap().into(),
                        proxied: record.proxied,
                        ttl: record.ttl,
                    })
                }
                if let Some(watcher) = watchers_by_token.get_mut(token) {
                    watcher.watching.push(zone);
                } else {
                    let mut watcher = Watcher::new(interface.clone(), token.to_string())?;
                    watcher.watching.push(zone);
                    watchers_by_token.insert(token.to_string(), watcher);
                }
            }

            // If there were no records/zones defined for this interface, add a placeholder watcher
            // Such a watcher will not have anything to do, but can be used to show information about
            // the interface configuration, and in the future could support hot-reloading configuration
            if watchers_by_token.is_empty() {
                watchers.push(Watcher::new(interface.clone(), String::new())?);
            } else {
                // Append watchers for this interface to the final set
                for watcher in watchers_by_token.into_values() {
                    watchers.push(watcher);
                }
            }
        }

        Ok(Self {
            ifconfig,
            file: config,
            watchers,
        })
    }
}
