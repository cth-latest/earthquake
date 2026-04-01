use crate::Result;
use crate::error::Error;
use async_trait::async_trait;
use rand::prelude::*;
use reqwest::Proxy as ReqwestProxy;
use serde::{Deserialize, Serialize};
use std::fs::File;
use std::io::{BufRead, BufReader};
use std::sync::Arc;
use std::time::{Duration, Instant};
use url::Url;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum ProxyType {
    Http,
    Https,
    Socks4,
    Socks5,
}

impl std::fmt::Display for ProxyType {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            ProxyType::Http => write!(f, "http"),
            ProxyType::Https => write!(f, "https"),
            ProxyType::Socks4 => write!(f, "socks4"),
            ProxyType::Socks5 => write!(f, "socks5"),
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Proxy {
    pub proxy_type: ProxyType,
    pub host: String,
    pub port: u16,
    pub username: Option<String>,
    pub password: Option<String>,
    #[serde(skip)]
    pub last_used: Option<Instant>,
    #[serde(skip)]
    pub failure_count: u32,
}

impl Proxy {
    pub fn new(proxy_type: ProxyType, host: impl Into<String>, port: u16) -> Self {
        Self {
            proxy_type,
            host: host.into(),
            port,
            username: None,
            password: None,
            last_used: None,
            failure_count: 0,
        }
    }

    pub fn with_auth(mut self, username: impl Into<String>, password: impl Into<String>) -> Self {
        self.username = Some(username.into());
        self.password = Some(password.into());
        self
    }

    pub fn from_url(url: &str) -> Result<Self> {
        let url = Url::parse(url)?;

        let host = url
            .host_str()
            .ok_or_else(|| Error::InvalidProxy("Missing host".to_string()))?
            .to_string();

        let port = url
            .port()
            .ok_or_else(|| Error::InvalidProxy("Missing port".to_string()))?;

        let proxy_type = match url.scheme() {
            "http" => ProxyType::Http,
            "https" => ProxyType::Https,
            "socks4" => ProxyType::Socks4,
            "socks5" => ProxyType::Socks5,
            scheme => {
                return Err(Error::InvalidProxy(format!(
                    "Unsupported scheme: {}",
                    scheme
                )));
            }
        };

        let mut proxy = Self::new(proxy_type, host, port);

        let username = url.username();
        if !username.is_empty() {
            let password = url.password().unwrap_or("");
            proxy = proxy.with_auth(username, password);
        }

        Ok(proxy)
    }

    pub fn to_url(&self) -> String {
        let auth_part = if let (Some(username), Some(password)) = (&self.username, &self.password) {
            format!("{}:{}@", username, password)
        } else {
            String::new()
        };

        format!(
            "{}://{}{}:{}",
            self.proxy_type, auth_part, self.host, self.port
        )
    }

    pub fn to_reqwest_proxy(&self) -> Result<ReqwestProxy> {
        let proxy_url = self.to_url();
        ReqwestProxy::all(&proxy_url).map_err(|e| Error::InvalidProxy(e.to_string()))
    }

    pub fn mark_used(&mut self) {
        self.last_used = Some(Instant::now());
    }

    pub fn mark_failure(&mut self) {
        self.failure_count += 1;
    }

    pub fn reset_failure(&mut self) {
        self.failure_count = 0;
    }

    pub fn is_available(&self, cooldown: Duration) -> bool {
        match self.last_used {
            Some(last_used) => last_used.elapsed() >= cooldown,
            None => true,
        }
    }
}

impl std::fmt::Display for Proxy {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}", self.to_url())
    }
}

#[async_trait]
pub trait ProxyProvider: Send + Sync {
    async fn next(&self) -> Option<Proxy>;
    async fn len(&self) -> usize;
    async fn reset(&self);
}

pub struct FileProxyProvider {
    proxies: Arc<parking_lot::RwLock<Vec<Proxy>>>,
    cooldown: Duration,
    max_failures: u32,
    random: bool,
}

impl FileProxyProvider {
    pub fn new() -> Self {
        Self {
            proxies: Arc::new(parking_lot::RwLock::new(Vec::new())),
            cooldown: Duration::from_secs(0),
            max_failures: 3,
            random: false,
        }
    }

    pub fn with_cooldown(mut self, cooldown: Duration) -> Self {
        self.cooldown = cooldown;
        self
    }

    pub fn with_max_failures(mut self, max_failures: u32) -> Self {
        self.max_failures = max_failures;
        self
    }

    pub fn random(mut self, random: bool) -> Self {
        self.random = random;
        self
    }

    pub fn load_from_file(&self, path: impl AsRef<std::path::Path>) -> Result<()> {
        let file = File::open(path)?;
        let reader = BufReader::new(file);
        let mut proxies = Vec::new();

        for line in reader.lines() {
            let line = line;

            if line.is_err() {
                continue;
            }

            let line = line.unwrap();
            let line = line.trim();

            if line.is_empty() {
                continue;
            }

            match Proxy::from_url(line) {
                Ok(proxy) => proxies.push(proxy),
                Err(_) => continue,
            }
        }

        *self.proxies.write() = proxies;
        Ok(())
    }

    pub async fn load_from_url(&self, url: &str) -> Result<()> {
        let response = reqwest::get(url).await?.text().await?;

        let mut proxies = Vec::new();

        for line in response.lines() {
            let line = line.trim();

            if line.is_empty() {
                continue;
            }

            match Proxy::from_url(line) {
                Ok(proxy) => proxies.push(proxy),
                Err(_) => continue,
            }
        }

        *self.proxies.write() = proxies;
        Ok(())
    }

    pub fn add_proxy(&self, proxy: Proxy) {
        self.proxies.write().push(proxy);
    }
}

#[async_trait]
impl ProxyProvider for FileProxyProvider {
    async fn next(&self) -> Option<Proxy> {
        let mut proxies = self.proxies.write();

        if proxies.is_empty() {
            return None;
        }

        let index = if self.random {
            rand::thread_rng().gen_range(0..proxies.len())
        } else {
            // Find the first available proxy
            let mut available_idx = None;

            for (idx, proxy) in proxies.iter().enumerate() {
                if proxy.failure_count < self.max_failures && proxy.is_available(self.cooldown) {
                    available_idx = Some(idx);
                    break;
                }
            }

            match available_idx {
                Some(idx) => idx,
                None => {
                    // Reset all proxies if none are available
                    for proxy in proxies.iter_mut() {
                        proxy.reset_failure();
                        proxy.last_used = None;
                    }

                    0
                }
            }
        };

        let mut proxy = proxies[index].clone();
        proxy.mark_used();

        Some(proxy)
    }

    async fn len(&self) -> usize {
        self.proxies.read().len()
    }

    async fn reset(&self) {
        let mut proxies = self.proxies.write();

        for proxy in proxies.iter_mut() {
            proxy.reset_failure();
            proxy.last_used = None;
        }
    }
}
