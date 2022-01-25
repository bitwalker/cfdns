use std::env;
use std::path::{Path, PathBuf};

use anyhow::{bail, Context};
use serde::{Deserialize, Serialize};

use crate::cloudflare::{DnsRecordType, Id, ProxyMode, Ttl};

use super::Interface;

#[derive(Default, Serialize, Deserialize)]
#[serde(default, deny_unknown_fields)]
pub struct ConfigFile {
    pub interfaces: Vec<Interface>,
    pub records: Vec<RecordConfig>,
    pub zones: Vec<ZoneConfig>,
}
impl ConfigFile {
    pub fn zone(&self, name: &str) -> Option<&ZoneConfig> {
        for zone in self.zones.iter() {
            if zone.name == name {
                return Some(zone);
            }
        }

        None
    }
}

#[derive(Serialize, Deserialize)]
pub struct ZoneConfig {
    #[serde(default)]
    pub id: Option<Id>,
    pub name: String,
    pub token: String,
}

#[derive(Serialize, Deserialize)]
pub struct RecordConfig {
    pub name: String,
    pub zone: String,
    pub interface: String,
    #[serde(default, rename = "type")]
    pub ty: DnsRecordType,
    #[serde(default)]
    pub ttl: Ttl,
    #[serde(default)]
    pub proxied: ProxyMode,
}

pub fn read_from_path(path: &Path) -> anyhow::Result<ConfigFile> {
    let contents = std::fs::read_to_string(path)
        .with_context(|| format!("Failed to read config from {}", path.display()))?;
    let config = toml::from_str::<ConfigFile>(contents.as_str())
        .with_context(|| format!("Failed to parse config at {}", path.display()))?;

    validate(config)
}

pub fn read_from_system() -> anyhow::Result<ConfigFile> {
    use crate::system::Platform;

    let config_dir = match Platform::detect()? {
        Platform::UDM | Platform::UDMP => PathBuf::from("/mnt/data/cfdns/etc"),
        Platform::UDMSE | Platform::UDR => PathBuf::from("/data/cfdns/etc"),
        Platform::Other => match dirs::config_dir() {
            Some(dir) => dir.join("cfdns"),
            None => env::current_dir().unwrap(),
        },
    };

    let config_path = config_dir.join("config.toml");
    read_from_path(config_path.as_path())
}

fn validate(mut config: ConfigFile) -> anyhow::Result<ConfigFile> {
    for (i, interface) in config.interfaces.iter().enumerate() {
        if interface.name.is_empty() {
            bail!("Interface is missing name at index {}", i);
        }
    }

    for (i, record) in config.records.iter().enumerate() {
        if record.name.is_empty() {
            bail!("Record is missing name at index {}", i);
        }

        if record.interface.is_empty() {
            bail!(
                "Record '{}' requires a non-empty interface binding",
                &record.name
            );
        }

        if record.zone.is_empty() {
            bail!(
                "Record '{}' requires a non-empty zone binding",
                &record.name
            );
        }
    }

    for (i, zone) in config.zones.iter_mut().enumerate() {
        if zone.name.is_empty() {
            bail!("Zone is missing name at index {}", i);
        }

        if zone.token.is_empty() {
            bail!("Zone '{}' is missing a token", &zone.name);
        }
    }

    Ok(config)
}
