#![warn(clippy::pedantic)]

use uuid::{Uuid, uuid};

#[cfg(feature = "cache")]
pub mod cache;
pub mod config;
pub mod error;
pub mod service;
#[cfg(feature = "telemetry")]
pub mod telemetry;
pub mod util;

#[cfg(feature = "derive")]
pub use runesys_derive::Service;

const NAMESPACE: Uuid = uuid!("466b8727-8f7f-4596-b59d-92b2252b2c4b");

pub trait Service {
	const INFO: ServiceInfo;

	#[cfg(debug_assertions)]
	const FILE_DESCRIPTOR_SET: &'static [u8];
}

pub struct ServiceInfo {
	pub name: &'static str,
	pub pkg: &'static str,
	pub version: &'static str,
}

impl ServiceInfo {
	pub fn uuid(&self) -> Uuid {
		Uuid::new_v5(&NAMESPACE, self.pkg.as_bytes())
	}
}

// #[cfg(debug_assertions)]
// pub fn add_reflection_service(
// 	s: &mut RoutesBuilder,
// 	name: impl Into<String>,
// ) -> anyhow::Result<()> {
// 	let reflection = tonic_reflection::server::Builder::configure()
// 		.register_encoded_file_descriptor_set(FILE_DESCRIPTOR_SET)
// 		.with_service_name(name)
// 		.build_v1()?;
//
// 	s.add_service(reflection);
// 	Ok(())
// }
//
// #[cfg(not(debug_assertions))]
// pub fn add_reflection_service(
// 	s: &mut RoutesBuilder,
// 	_name: impl Into<String>,
// ) -> anyhow::Result<()> {
// 	Ok(())
// }

pub mod tracing {
	#[cfg(feature = "telemetry")]
	use opentelemetry::trace::TracerProvider;
	use tracing::level_filters::LevelFilter;
	use tracing_subscriber::{layer::SubscriberExt, util::SubscriberInitExt};

	use crate::ServiceInfo;

	#[allow(private_interfaces)]
	pub fn init(info: &ServiceInfo) {
		if tracing::dispatcher::has_been_set() {
			return;
		}

		let subscriber = tracing_subscriber::registry()
			.with(
				tracing_subscriber::EnvFilter::builder()
					.with_default_directive(LevelFilter::INFO.into())
					.from_env_lossy(),
			)
			.with(tracing_subscriber::fmt::layer());

		#[cfg(feature = "telemetry")]
		let subscriber = subscriber
			.with(tracing_opentelemetry::OpenTelemetryLayer::new(
				crate::telemetry::init_tracer_provider(&info).tracer(info.pkg),
			))
			.with(tracing_opentelemetry::MetricsLayer::new(
				crate::telemetry::init_meter_provider(&info),
			));

		subscriber.init();
	}
}
