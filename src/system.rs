use std::collections::HashMap;
use std::net::{IpAddr, Ipv4Addr, Ipv6Addr};
use std::path::Path;
use std::process::{Command, Stdio};
use std::str::FromStr;

use anyhow::bail;
use ifcfg::InterfaceAddress;

pub use ifcfg::AddressFamily;

/// Represents the current platform type we're running on
#[allow(clippy::upper_case_acronyms)]
pub enum Platform {
    UDM,
    UDMP,
    UDMSE,
    UDR,
    Other,
}

impl FromStr for Platform {
    type Err = ();

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        match s {
            "UDM" => Ok(Self::UDM),
            "UDM-Pro" => Ok(Self::UDMP),
            "UDM-SE" => Ok(Self::UDMSE),
            "UDR" => Ok(Self::UDR),
            _ => Ok(Self::Other),
        }
    }
}

impl Platform {
    pub fn detect() -> anyhow::Result<Self> {
        // If ubnt-device-info doesn't exist, we're not on a UbiOS platform
        let path = Path::new("/usr/bin/ubnt-device-info");
        if !path.exists() {
            return Ok(Self::Other);
        }

        let mut cmd = Command::new(&path);
        cmd.arg("model_short");

        let output = match cmd.stderr(Stdio::inherit()).output() {
            Ok(status) => status,
            Err(e) => bail!("Failed to execute ubnt-device-info: {}", e),
        };

        if !output.status.success() {
            bail!("ubnt-device-info failed with status {}", output.status);
        }

        let s = String::from_utf8(output.stdout).unwrap();

        Ok(s.parse().unwrap())
    }
}

/// Tracks all interfaces that have a bound IP address (v4 or v6)
pub struct IfConfig {
    interfaces: HashMap<String, InterfaceInfo>,
}
impl IfConfig {
    /// Reads the system network interfaces for the set of known IP addresses
    pub fn new() -> Self {
        let mut ifcfg = ifcfg::IfCfg::get().expect("Failed to load network interfaces");

        let mut interfaces = HashMap::with_capacity(ifcfg.len());
        for interface in ifcfg.drain(0..) {
            let info = InterfaceInfo::from(&interface);
            if info.has_ip() {
                interfaces.insert(interface.name, info);
            }
        }

        Self { interfaces }
    }

    /// Get info about the interface with the given name
    pub fn get(&self, name: &str) -> Option<&InterfaceInfo> {
        self.interfaces.get(name)
    }
}

/// Represents known address information about a specific network interface
#[derive(Default, Debug, Clone)]
pub struct InterfaceInfo {
    v4: Option<Ipv4Addr>,
    v6: Option<Ipv6Addr>,
}
impl InterfaceInfo {
    pub fn has_ip(&self) -> bool {
        self.v4.is_some() || self.v6.is_some()
    }

    pub fn address(&self, ty: AddressFamily) -> Option<IpAddr> {
        match ty {
            AddressFamily::IPv4 => self.v4.map(IpAddr::V4),
            AddressFamily::IPv6 => self.v6.map(IpAddr::V6),
            _ => None,
        }
    }
}
impl From<&ifcfg::IfCfg> for InterfaceInfo {
    fn from(interface: &ifcfg::IfCfg) -> Self {
        Self {
            v4: extract_v4_address(interface.addresses.as_slice()),
            v6: extract_v6_address(interface.addresses.as_slice()),
        }
    }
}

fn extract_v4_address(addresses: &[InterfaceAddress]) -> Option<Ipv4Addr> {
    for addr in addresses {
        match addr.address_family {
            AddressFamily::IPv4 => match addr.address.unwrap().ip() {
                IpAddr::V4(addr) => return Some(addr),
                other => panic!("Address declared as v4, but got v6 value: {:?}", &other),
            },
            _ => continue,
        }
    }

    None
}

fn extract_v6_address(addresses: &[InterfaceAddress]) -> Option<Ipv6Addr> {
    for addr in addresses {
        match addr.address_family {
            AddressFamily::IPv6 => match addr.address.unwrap().ip() {
                IpAddr::V6(addr) => return Some(addr),
                other => panic!("Address declared as v6, but got v4 value: {:?}", &other),
            },
            _ => continue,
        }
    }

    None
}
