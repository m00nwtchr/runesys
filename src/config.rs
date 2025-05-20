use std::{
	net::{IpAddr, Ipv6Addr},
	sync::{LazyLock, OnceLock},
};

use figment::{
	Figment, Metadata, Profile, Provider,
	value::{Dict, Map},
};
use serde::{Deserialize, Serialize, de::DeserializeOwned};
use url::Url;

use crate::{Service, ServiceInfo};

#[derive(Debug, Deserialize, Serialize)]
pub struct Config {
	pub grpc_port: u16,

	#[cfg(feature = "http")]
	pub http_port: u16,

	pub address: IpAddr,

	#[cfg(feature = "redis")]
	pub redis_url: Url,

	#[cfg(feature = "db")]
	pub postgres_url: Option<Url>,
}

impl Default for Config {
	fn default() -> Self {
		Config {
			grpc_port: 50051,
			http_port: 3434,
			address: IpAddr::V6(Ipv6Addr::UNSPECIFIED),
			redis_url: Url::parse("redis://valkey/").expect("Hardcoded Redis URL"),
			postgres_url: None,
		}
	}
}

impl Config {
	// Allow the configuration to be extracted from any `Provider`.
	fn from<T: Provider>(provider: T) -> Result<Config, figment::Error> {
		Figment::from(provider).extract()
	}

	// Provide a default provider, a `Figment`.
	fn figment() -> Figment {
		use figment::providers::Env;

		Figment::from(Config::default()).merge(Env::raw())
	}
}

impl Provider for Config {
	fn metadata(&self) -> Metadata {
		Metadata::named("runesys Config")
	}

	fn data(&self) -> Result<Map<Profile, Dict>, figment::Error> {
		figment::providers::Serialized::defaults(Config::default()).data()
	}
}

pub static FIGMENT: LazyLock<Figment> = LazyLock::new(|| Config::figment());

#[macro_export]
macro_rules! define_config {
	($ty:ty) => {
		pub fn config() -> &'static $ty {
			pub static CONFIG: std::sync::OnceLock<$ty> = std::sync::OnceLock::new();
			CONFIG.get_or_init(|| ::runesys::config::FIGMENT.extract().unwrap())
		}
	};
}

pub fn config() -> &'static Config {
	pub static CONFIG: OnceLock<Config> = OnceLock::new();
	CONFIG.get_or_init(|| FIGMENT.extract().unwrap())
}
