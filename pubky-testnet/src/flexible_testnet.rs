#![doc = include_str!("../README.md")]
//!

#![deny(missing_docs)]
#![deny(rustdoc::broken_intra_doc_links)]
#![cfg_attr(any(), deny(clippy::unwrap_used))]
use std::{str::FromStr, time::Duration};

use anyhow::Result;
use http_relay::HttpRelay;
use pubky::Keypair;
use pubky_homeserver::{ConfigToml, DataDirMock, DomainPort, HomeserverSuite};
use url::Url;

/// A local test network for Pubky Core development.
/// Can create a flexible amount of pkarr relays, http relays and homeservers.
///
/// Keeps track of the components and can create new ones.
/// Cleans up all resources when dropped.
pub struct FlexibleTestnet {
    pub(crate) dht: pkarr::mainline::Testnet,
    pub(crate) pkarr_relays: Vec<pkarr_relay::Relay>,
    pub(crate) http_relays: Vec<HttpRelay>,
    pub(crate) homeservers: Vec<HomeserverSuite>,
}

impl FlexibleTestnet {
    /// Run a new testnet with a local DHT.
    pub async fn new() -> Result<Self> {
        let dht = pkarr::mainline::Testnet::new_async(3).await?;
        let testnet = Self {
            dht,
            pkarr_relays: vec![],
            http_relays: vec![],
            homeservers: vec![],
        };

        Ok(testnet)
    }

    /// Run the full homeserver suite with core and admin server
    /// Automatically listens on the default ports.
    /// Automatically uses the configured bootstrap nodes and relays in this Testnet.
    pub async fn create_homeserver_suite(&mut self) -> Result<&HomeserverSuite> {
        let mock_dir =
            DataDirMock::new(ConfigToml::test(), Some(Keypair::from_secret_key(&[0; 32])))?;
        self.create_homeserver_suite_with_mock(mock_dir).await
    }

    /// Run the full homeserver suite with core and admin server
    /// Automatically listens on the configured ports.
    /// Automatically uses the configured bootstrap nodes and relays in this Testnet.
    pub async fn create_homeserver_suite_with_mock(
        &mut self,
        mut mock_dir: DataDirMock,
    ) -> Result<&HomeserverSuite> {
        mock_dir.config_toml.pkdns.dht_bootstrap_nodes = Some(self.dht_bootstrap_nodes());
        if !self.dht_relay_urls().is_empty() {
            mock_dir.config_toml.pkdns.dht_relay_nodes = Some(self.dht_relay_urls().to_vec());
        }
        let homeserver = HomeserverSuite::run_with_data_dir_mock(mock_dir).await?;
        self.homeservers.push(homeserver);
        Ok(self
            .homeservers
            .last()
            .expect("homeservers should be non-empty"))
    }

    /// Run an HTTP Relay
    pub async fn create_http_relay(&mut self) -> Result<&HttpRelay> {
        let relay = HttpRelay::builder()
            .http_port(0) // Random available port
            .run()
            .await?;
        self.http_relays.push(relay);
        Ok(self
            .http_relays
            .last()
            .expect("http relays should be non-empty"))
    }

    /// Run a new Pkarr relay.
    ///
    /// You can access the list of relays at [Self::relays].
    pub async fn create_pkarr_relay(&mut self) -> Result<Url> {
        let relay = pkarr_relay::Relay::run_test(&self.dht).await?;
        let url = relay.local_url();
        self.pkarr_relays.push(relay);

        Ok(url)
    }

    /// Run a new Pkarr relay.
    ///
    /// You can access the list of relays at [Self::relays].
    pub async fn create_pkarr_relay2(&mut self) -> Result<Url> {
        let mut builder =pkarr_relay::Relay::builder();
        builder
        .disable_rate_limiter()
        .cache_size(0)
        .http_port(0)
        .pkarr(|builder| {
            builder.no_default_network();
            builder.bootstrap(&self.dht.bootstrap);
            builder.request_timeout(Duration::from_millis(2000));
            builder
        });
        let relay = unsafe { builder.run().await? };
        let url = relay.local_url();
        self.pkarr_relays.push(relay);

        Ok(url)
    }

    // === Getters ===

    /// Returns a list of DHT bootstrapping nodes.
    pub fn dht_bootstrap_nodes(&self) -> Vec<DomainPort> {
        self.dht
            .bootstrap
            .iter()
            .map(|s| {
                DomainPort::from_str(s)
                    .expect("boostrap nodes from the pkarr dht are always valid domain:port pairs")
            })
            .collect()
    }

    /// Returns a list of pkarr relays.
    pub fn dht_relay_urls(&self) -> Vec<Url> {
        self.pkarr_relays.iter().map(|r| r.local_url()).collect()
    }

    /// Create a [ClientBuilder] and configure it to use this local test network.
    pub fn pubky_client_builder(&self) -> pubky::ClientBuilder {
        let relays = self.dht_relay_urls();

        let mut builder = pubky::Client::builder();
        builder.pkarr(|builder| {
            builder.no_default_network();
            builder.bootstrap(&self.dht.bootstrap);
            if relays.is_empty() {
                builder.no_relays();
            } else {
                builder.relays(&relays)
                .expect("testnet relays should be valid urls");
            }
            // 100ms timeout for requests. This makes methods like `resolve_most_recent` fast 
            // because it doesn't need to wait the default 2s which would slow down the tests.
            builder.request_timeout(Duration::from_millis(2000)); 
            builder
        });

        builder
    }

    /// Create a [pkarr::ClientBuilder] and configure it to use this local test network.
    pub fn pkarr_client_builder(&self) -> pkarr::ClientBuilder {
        let relays = self.dht_relay_urls();
        let mut builder = pkarr::Client::builder();
        builder.no_default_network();
        builder.bootstrap(&self.dht.bootstrap);
        if !relays.is_empty() {
            builder.relays(&relays).expect("Testnet relays should be valid urls");
        }

        builder
    }
}

#[cfg(test)]
mod test {
    use std::time::Duration;

    use pubky::Keypair;
    use crate::FlexibleTestnet;

    /// Make sure the components are kept alive even when dropped.
    #[tokio::test]
    async fn test_keep_relays_alive_even_when_dropped() {
        let mut testnet = FlexibleTestnet::new().await.unwrap();
        {
            let _relay = testnet.create_http_relay().await.unwrap();
        }
        assert_eq!(testnet.http_relays.len(), 1);
    }

    /// Test that a user can signup in the testnet.
    /// This is an e2e tests to check if everything is correct.
    #[tokio::test]
    async fn test_signup() {
        let mut testnet = FlexibleTestnet::new().await.unwrap();
        testnet.create_homeserver_suite().await.unwrap();
        let client = testnet.pubky_client_builder().build().unwrap();
        let hs = testnet.homeservers.first().unwrap();
        let keypair = Keypair::random();
        let pubky = keypair.public_key();

        let session = client.signup(&keypair, &hs.public_key(), None).await.unwrap();
        assert_eq!(session.pubky(), &pubky);
    }

    #[tokio::test]
    async fn test_independent_dhts() {
        let t1 = FlexibleTestnet::new().await.unwrap();
        let t2 = FlexibleTestnet::new().await.unwrap();
        
        assert_ne!(t1.dht.bootstrap, t2.dht.bootstrap);
    }

    /// If everything is linked correctly, the hs_pubky should be resolvable from the pkarr client.
    #[tokio::test]
    async fn test_homeserver_resolvable() {
        let mut testnet = FlexibleTestnet::new().await.unwrap();
        let hs_pubky = testnet.create_homeserver_suite().await.unwrap().public_key();

        // Make sure the pkarr packet of the hs is resolvable.
        let pkarr_client = testnet.pkarr_client_builder().build().unwrap();
        let _packet = pkarr_client.resolve(&hs_pubky).await.unwrap();
        
        // Make sure the pkarr can resolve the hs_pubky.
        let pubkey = format!("{}", hs_pubky);
        let _endpoint = pkarr_client.resolve_https_endpoint(pubkey.as_str()).await.unwrap();
    }

    #[tokio::test]
    async fn test_spawn_in_parallel() {
        let mut handles = Vec::new();

        for _ in 0..10 {
            let handle = tokio::spawn(async move {
                let mut testnet = match FlexibleTestnet::new().await {
                    Ok(testnet) => testnet,
                    Err(e) => {
                        panic!("Failed to create testnet: {}", e);
                    }
                };
                testnet.create_homeserver_suite().await.unwrap();
                let client = testnet.pubky_client_builder().build().unwrap();
                let hs = testnet.homeservers.first().unwrap();
                let keypair = Keypair::random();
                let pubky = keypair.public_key();
        
                let session = client.signup(&keypair, &hs.public_key(), None).await.unwrap();
                assert_eq!(session.pubky(), &pubky);
                tokio::time::sleep(Duration::from_secs(3)).await;
            });
            handles.push(handle);
        }

        for handle in handles {
            match handle.await {
                Ok(_) => {}
                Err(e) => {
                    panic!("{}", e);
                }
            }
        }
    }
}
