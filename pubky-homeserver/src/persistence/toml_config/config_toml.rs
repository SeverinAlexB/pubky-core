//! Configuration for the server

use anyhow::Result;
use pkarr::Keypair;
use serde::{Deserialize, Deserializer, Serialize, Serializer};
use std::{
    fmt::Debug,
    net::{IpAddr, Ipv4Addr},
    path::PathBuf,
};
use hex;

use crate::persistence::toml_config::validate_domain::validate_domain;



#[derive(Debug, Serialize, Deserialize, Clone, PartialEq)]
struct DatabaseToml {
    #[serde(default = "default_storage_path")]
    storage: PathBuf,
}

fn default_storage_path() -> PathBuf {
    PathBuf::from("./storage/")
}

#[derive(Debug, Default, Serialize, Deserialize, Clone, PartialEq, Eq)]
struct ReverseProxyToml {
    pub public_port: Option<u16>,
}

#[derive(Debug, Serialize, Deserialize, Clone, PartialEq)]
struct LegacyBrowsersTompl {
    #[serde(deserialize_with = "validate_domain")]
    pub domain: Option<String>,
}

#[derive(Debug, Serialize, Deserialize, Clone, PartialEq)]
struct IoToml {
    #[serde(default = "default_http_port")]
    pub http_port: u16,
    #[serde(default = "default_https_port")]
    pub https_port: u16,
    #[serde(default = "default_public_ip")]
    pub public_ip: IpAddr,
    pub reverse_proxy: ReverseProxyToml,
    pub legacy_browsers: LegacyBrowsersTompl,
}

fn default_http_port() -> u16 {
    6286
}

fn default_https_port() -> u16 {
    6287
}

fn default_public_ip() -> IpAddr {
    IpAddr::V4(Ipv4Addr::new(127, 0, 0, 1))
}

/// The main server configuration
#[derive(Debug, Serialize, Deserialize, Clone, PartialEq)]
struct ConfigToml {
    /// Homeserver's secret key in hex format
    #[serde(serialize_with = "serialize_keypair", deserialize_with = "deserialize_keypair", default = "default_keypair")]
    secret_key: Keypair,
    database: DatabaseToml,
    io: IoToml,
}

impl ConfigToml {
    /// Reads the configuration from a TOML file at the specified path.
    /// 
    /// # Arguments
    /// * `path` - The path to the TOML configuration file
    /// 
    /// # Returns
    /// * `Result<ConfigToml>` - The parsed configuration or an error if reading/parsing fails
    pub fn from_file(path: impl AsRef<std::path::Path>) -> Result<Self> {
        let contents = std::fs::read_to_string(path)?;
        let config: ConfigToml = toml::from_str(&contents)?;
        Ok(config)
    }
}

fn serialize_keypair<S>(keypair: &Keypair, serializer: S) -> Result<S::Ok, S::Error>
where
    S: Serializer,
{
    let secret_key = keypair.secret_key();
    let hex_string = hex::encode(secret_key);
    serializer.serialize_str(&hex_string)
}

fn deserialize_keypair<'de, D>(deserializer: D) -> Result<Keypair, D::Error>
where
    D: Deserializer<'de>,
{
    let hex_string: String = String::deserialize(deserializer)?;
    let bytes = hex::decode(&hex_string).map_err(serde::de::Error::custom)?;
    if bytes.len() != 32 {
        return Err(serde::de::Error::custom(format!(
            "secret_key should be 32 bytes in hex (64 characters), got: {}",
            bytes.len()
        )));
    }
    let mut arr = [0u8; 32];
    arr.copy_from_slice(&bytes);
    Ok(Keypair::from_secret_key(&arr))
}

fn default_keypair() -> Keypair {
    Keypair::from_secret_key(&[0; 32])
}

#[cfg(test)]
mod tests {
    use std::net::Ipv4Addr;

    use super::*;

    const SAMPLE_CONFIG: &str = r#"
# Secret key (in hex) to generate the Homeserver's Keypair
secret_key = "0000000000000000000000000000000000000000000000000000000000000000"

[database]
# Storage directory Defaults to <System's Data Directory>
#
# Storage path can be relative or absolute.
storage = "./storage/"

[io]
# The port number to run an HTTP (clear text) server on.
http_port = 6286
# The port number to run an HTTPs (Pkarr TLS) server on.
https_port = 6287

# The public IP of this server.
# 
# This address will be mentioned in the Pkarr records of this
# Homeserver that is published on its public key (derived from `secret_key`)
public_ip = "127.0.0.1"

# If you are running this server behind a reverse proxy,
# you need to provide some extra configurations.
[io.reverse_proxy]
# The public port should be mapped to the `io::https_port`
#   and you should setup tcp forwarding (don't terminate TLS on that port).
public_port = 6287

# If you want your server to be accessible from legacy browsers,
#   you need to provide some extra configurations.
[io.legacy_browsers]
# An ICANN domain name is necessary to support legacy browsers
#
# Make sure to setup a domain name and point it the IP
#   address of this machine where you are running this server.
#
# This domain should point to the `<public_ip>:<http_port>`.
# 
# Currently we don't support ICANN TLS, so you should be running
#   a reverse proxy and managing certificates there for this endpoint.
domain = "example.com"
    "#;

    #[test]
    fn parse_config() {
        let config: ConfigToml = toml::from_str(SAMPLE_CONFIG).expect("Failed to parse config");
        
        // Verify database config
        assert_eq!(config.database.storage, PathBuf::from("./storage/"));
        
        // Verify IO config
        let io = config.io;
        assert_eq!(io.http_port, 6286);
        assert_eq!(io.https_port, 6287);
        assert_eq!(io.public_ip, IpAddr::V4(Ipv4Addr::new(127, 0, 0, 1)));
        
        // Verify reverse proxy config
        assert_eq!(io.reverse_proxy.public_port, Some(6287));
        
        // Verify legacy browsers config
        assert_eq!(io.legacy_browsers.domain, Some("example.com".to_string()));
        
        // Verify secret key is all zeros (default value)
        assert_eq!(config.secret_key, default_keypair());
    }
}

