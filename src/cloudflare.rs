use std::fmt::{self, Display};
use std::net::{IpAddr, Ipv4Addr, Ipv6Addr};
use std::str::FromStr;

use anyhow::{anyhow, bail};
use serde::de::Visitor;
use serde::{Deserialize, Deserializer, Serialize, Serializer};

use crate::system::AddressFamily;

/// This type represents a non-empty identifier string bound to a Cloudflare resource
#[derive(Clone, Debug, PartialEq, Eq, Serialize)]
#[repr(transparent)]
pub struct Id(String);
impl Display for Id {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        write!(f, "{}", self.0)
    }
}
impl<'de> Deserialize<'de> for Id {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: Deserializer<'de>,
    {
        use serde::de::Error;

        let s = String::deserialize(deserializer)?;
        if s.is_empty() {
            Err(Error::custom("invalid id string, cannot be empty"))
        } else {
            Ok(Self(s))
        }
    }
}

/// This struct represents the few key details about DNS Zones in Cloudflare we care about
#[derive(Clone, Debug, Deserialize)]
pub struct Zone {
    pub id: Id,
    pub name: String,
    #[serde(skip)]
    pub records: Vec<DnsRecord>,
}
impl Zone {
    pub fn new(id: Id, name: String) -> Self {
        Self {
            id,
            name,
            records: vec![],
        }
    }

    pub fn get(name: &str, token: &str) -> anyhow::Result<Self> {
        let client = Cloudflare::new(token.to_string())?;
        if let Some(zone) = client.zone_by_name(name)? {
            Ok(zone)
        } else {
            bail!("No such zone '{}'", name);
        }
    }
}
/// This enum represents the type of DNS records we support updating
#[derive(Copy, Clone, Debug, PartialEq, Eq, Hash, Deserialize, Serialize)]
#[allow(clippy::upper_case_acronyms)]
pub enum DnsRecordType {
    A,
    AAAA,
    CNAME,
    Other,
}
impl Display for DnsRecordType {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        write!(f, "{:?}", self)
    }
}
impl Default for DnsRecordType {
    fn default() -> Self {
        Self::A
    }
}
impl From<AddressFamily> for DnsRecordType {
    fn from(ty: AddressFamily) -> Self {
        match ty {
            AddressFamily::IPv4 => Self::A,
            AddressFamily::IPv6 => Self::AAAA,
            _ => Self::Other,
        }
    }
}
impl TryInto<AddressFamily> for DnsRecordType {
    type Error = ();
    fn try_into(self) -> Result<AddressFamily, Self::Error> {
        match self {
            Self::A => Ok(AddressFamily::IPv4),
            Self::AAAA => Ok(AddressFamily::IPv6),
            _ => Err(()),
        }
    }
}

/// This struct represents the TTL value for a DNS record
///
/// The implementation is a bit more complicated because we want to handle assigning
/// the correct default TTL in Cloudflare (a value of '1', or automatic) if we don't
/// have a specific TTL on hand.
///
/// This struct has the same representation as a u32 value
#[derive(Default, Copy, Clone, Debug, Deserialize, PartialEq, Eq, PartialOrd, Ord)]
#[repr(transparent)]
pub struct Ttl(Option<core::num::NonZeroU32>);
impl Display for Ttl {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        if let Some(ttl) = self.0 {
            write!(f, "{}", ttl.get())
        } else {
            write!(f, "1")
        }
    }
}
impl Serialize for Ttl {
    fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: Serializer,
    {
        if let Some(v) = self.0 {
            serializer.serialize_u32(v.get())
        } else {
            // Default to '1', or automatic, if not set
            serializer.serialize_u32(1)
        }
    }
}

/// This enum represents whether or not a DNS record is proxied by Cloudflare
///
/// We use a dedicated type here vs a boolean because it has default behavior
/// that we wish to encode when (de)serializing
#[derive(Copy, Clone, Debug, PartialEq, Eq)]
pub enum ProxyMode {
    Proxied,
    None,
}
impl ProxyMode {
    pub fn as_bool(&self) -> bool {
        match self {
            Self::Proxied => true,
            Self::None => false,
        }
    }
}
impl Default for ProxyMode {
    #[inline(always)]
    fn default() -> Self {
        Self::None
    }
}
impl Serialize for ProxyMode {
    fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: Serializer,
    {
        match self {
            Self::Proxied => serializer.serialize_bool(true),
            Self::None => serializer.serialize_bool(false),
        }
    }
}
impl<'de> Deserialize<'de> for ProxyMode {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: Deserializer<'de>,
    {
        use serde::de::Error;

        struct ProxyModeVisitor;
        impl<'de> Visitor<'de> for ProxyModeVisitor {
            type Value = ProxyMode;

            fn expecting(&self, formatter: &mut fmt::Formatter) -> fmt::Result {
                formatter.write_str("an optional proxy mode")
            }

            #[inline]
            fn visit_bool<E>(self, v: bool) -> Result<Self::Value, E>
            where
                E: Error,
            {
                if v {
                    Ok(ProxyMode::Proxied)
                } else {
                    Ok(ProxyMode::None)
                }
            }

            #[inline]
            fn visit_unit<E>(self) -> Result<Self::Value, E>
            where
                E: Error,
            {
                Ok(ProxyMode::None)
            }

            #[inline]
            fn visit_none<E>(self) -> Result<Self::Value, E>
            where
                E: Error,
            {
                Ok(ProxyMode::None)
            }

            #[inline]
            fn visit_some<D>(self, deserializer: D) -> Result<Self::Value, D::Error>
            where
                D: Deserializer<'de>,
            {
                bool::deserialize(deserializer).map(|v| {
                    if v {
                        ProxyMode::Proxied
                    } else {
                        ProxyMode::None
                    }
                })
            }
        }
        deserializer.deserialize_option(ProxyModeVisitor)
    }
}

/// This struct represents the key details of a single DNS record in Cloudflare
///
/// This record is used for rendering data received from Cloudflare, as well as
/// encoding the parameters for create/update operations.
#[derive(Clone, Debug, Deserialize, Serialize)]
pub struct DnsRecord {
    #[serde(skip_serializing)]
    pub id: Option<Id>,
    #[serde(skip_serializing)]
    pub zone_id: Id,
    pub name: String,
    #[serde(rename = "type")]
    pub ty: DnsRecordType,
    pub content: DnsContent,
    #[serde(default)]
    pub proxied: ProxyMode,
    #[serde(default)]
    pub ttl: Ttl,
}
impl DnsRecord {
    /// Given an IPv4 or IPv6 address, attempts to update this DNS record.
    ///
    /// If the record is of a matching address type, and a change was applied,
    /// then `Ok(true)` is returned. If no change was made, then `Ok(false)` is
    /// returned.
    ///
    /// If the record is of a different address type, then `Err(())` is returned.
    pub fn try_update(&mut self, addr: IpAddr) -> anyhow::Result<bool> {
        match addr {
            IpAddr::V4(desired) if self.ty == DnsRecordType::A => match self.content {
                DnsContent::A(current) if current == desired => Ok(false),
                DnsContent::A(_) => {
                    self.content = DnsContent::A(desired);
                    Ok(true)
                }
                _ => bail!(
                    "unable to update record for {} with address of different type",
                    &self.name
                ),
            },
            IpAddr::V6(desired) if self.ty == DnsRecordType::AAAA => match self.content {
                DnsContent::AAAA(current) if current == desired => Ok(false),
                DnsContent::AAAA(_) => {
                    self.content = DnsContent::AAAA(desired);
                    Ok(true)
                }
                _ => bail!(
                    "unable to update record for {} with address of different type",
                    &self.name
                ),
            },
            _ => bail!(
                "unable to update record for {} with address of different type",
                &self.name
            ),
        }
    }
}

/// This enum represents the actual value of a DNS record, e.g. for A records, the IPv4 address.
#[derive(Clone, Debug, PartialEq, Eq)]
#[allow(clippy::upper_case_acronyms)]
pub enum DnsContent {
    A(Ipv4Addr),
    AAAA(Ipv6Addr),
    Other(String),
}
impl Serialize for DnsContent {
    fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: Serializer,
    {
        let s = format!("{}", self);
        serializer.serialize_str(&s)
    }
}
impl<'de> Deserialize<'de> for DnsContent {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: Deserializer<'de>,
    {
        use serde::de::Error;

        let s = String::deserialize(deserializer)?;
        s.parse::<Self>()
            .map_err(|_| Error::custom("Invalid DNS content"))
    }
}
impl From<Ipv4Addr> for DnsContent {
    #[inline]
    fn from(addr: Ipv4Addr) -> Self {
        Self::A(addr)
    }
}
impl From<Ipv6Addr> for DnsContent {
    #[inline]
    fn from(addr: Ipv6Addr) -> Self {
        Self::AAAA(addr)
    }
}
impl From<IpAddr> for DnsContent {
    #[inline]
    fn from(addr: IpAddr) -> Self {
        match addr {
            IpAddr::V4(v4) => Self::A(v4),
            IpAddr::V6(v6) => Self::AAAA(v6),
        }
    }
}
impl Display for DnsContent {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        match self {
            Self::A(addr) => write!(f, "{}", addr),
            Self::AAAA(addr) => write!(f, "{}", addr),
            Self::Other(value) => write!(f, "{}", value),
        }
    }
}
impl FromStr for DnsContent {
    type Err = ();

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        match s.parse::<IpAddr>().map_err(|_| ()) {
            Ok(IpAddr::V4(addr)) => Ok(Self::A(addr)),
            Ok(IpAddr::V6(addr)) => Ok(Self::AAAA(addr)),
            Err(_) => Ok(Self::Other(s.to_string())),
        }
    }
}

/// This struct represents the payload returned from the Cloudflare API
#[derive(Serialize, Deserialize)]
struct Response<T> {
    success: bool,
    result: Option<T>,
    errors: Vec<ResponseError>,
}
impl<T> Response<T> {
    /// Converts the Response object to a Result based on whether it was successful or not, unwrapping the payload
    fn ok(mut self) -> anyhow::Result<T> {
        if self.success {
            Ok(self.result.ok_or_else(|| {
                anyhow!(
                    "expected successful response to contain payload of type {}, but got null",
                    std::any::type_name::<T>()
                )
            })?)
        } else {
            Err(self.errors.pop().unwrap().into())
        }
    }
}

#[derive(thiserror::Error, Debug, Serialize, Deserialize)]
#[error("Request failed with code {code}: {message}")]
struct ResponseError {
    code: usize,
    message: String,
}

/// This struct represents an instantiation of a Cloudflare API client, bound to a specific token
pub struct Cloudflare {
    client: reqwest::blocking::Client,
}
impl Cloudflare {
    /// Create a new Cloudflare API client
    pub fn new(token: String) -> anyhow::Result<Self> {
        use reqwest::header::{self, HeaderMap, HeaderValue};
        use std::time::Duration;

        let mut headers = HeaderMap::new();
        let bearer = format!("Bearer {}", &token);
        let mut key = HeaderValue::from_str(&bearer)?;
        key.set_sensitive(true);
        headers.insert(header::AUTHORIZATION, key);

        let client = reqwest::blocking::Client::builder()
            .timeout(Duration::from_secs(30))
            .default_headers(headers)
            .build()?;

        Ok(Self { client })
    }

    /// Fetch the zone identifier for the zone with the given domain name
    pub fn zone_by_name(&self, name: &str) -> anyhow::Result<Option<Zone>> {
        let response: Response<Vec<Zone>> = self
            .client
            .get("https://api.cloudflare.com/client/v4/zones".to_string())
            .query(&[("name", name), ("status", "active")])
            .send()?
            .error_for_status()?
            .json()?;

        let mut zones = response.ok()?;

        Ok(zones.pop())
    }

    /// Get the current DNS record binding for the given name, in the given zone
    pub fn get_by_name(&self, zone_id: &Id, name: &str) -> anyhow::Result<Option<DnsRecord>> {
        let response: Response<Vec<DnsRecord>> = self
            .client
            .get(format!(
                "https://api.cloudflare.com/client/v4/zones/{}/dns_records",
                zone_id
            ))
            .query(&[("name", name)])
            .send()?
            .error_for_status()?
            .json()?;

        let mut records = response.ok()?;

        Ok(records.pop())
    }

    /// Get the current DNS record binding for the given name and type, in the given zone
    pub fn get(
        &self,
        zone_id: &Id,
        name: &str,
        ty: DnsRecordType,
    ) -> anyhow::Result<Option<DnsRecord>> {
        let ty = ty.to_string();
        let response: Response<Vec<DnsRecord>> = self
            .client
            .get(format!(
                "https://api.cloudflare.com/client/v4/zones/{}/dns_records",
                zone_id
            ))
            .query(&[("name", name), ("type", ty.as_str())])
            .send()?
            .error_for_status()?
            .json()?;

        let mut records = response.ok()?;

        Ok(records.pop())
    }

    /// Create the given DNS record
    pub fn create(&self, record: &mut DnsRecord) -> anyhow::Result<()> {
        if record.id.is_some() {
            bail!("Cannot create a DNS record with a resource id set");
        }
        let zone_id = &record.zone_id;
        let response: Response<DnsRecord> = self
            .client
            .post(format!(
                "https://api.cloudflare.com/client/v4/zones/{}/dns_records",
                zone_id
            ))
            .json(&record)
            .send()?
            .error_for_status()?
            .json()?;

        *record = response.ok()?;

        Ok(())
    }

    /// Update the given DNS record
    pub fn update(&self, record: &mut DnsRecord) -> anyhow::Result<()> {
        if let Some(id) = &record.id {
            let zone_id = &record.zone_id;
            let response: Response<DnsRecord> = self
                .client
                .put(format!(
                    "https://api.cloudflare.com/client/v4/zones/{}/dns_records/{}",
                    zone_id, id
                ))
                .json(&record)
                .send()?
                .error_for_status()?
                .json()?;

            *record = response.ok()?;

            Ok(())
        } else {
            bail!("Cannot update a DNS record that is missing its Cloudflare resource id");
        }
    }
}
