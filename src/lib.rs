#![warn(clippy::pedantic)]

use std::convert::Infallible;

use axum::http::Request;
use tonic::{body::Body, server::NamedService, service::Routes};
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

use crate::service::ServiceBuilder;

const NAMESPACE: Uuid = uuid!("466b8727-8f7f-4596-b59d-92b2252b2c4b");

pub trait Service {
	const INFO: ServiceInfo;
	#[cfg(debug_assertions)]
	const FILE_DESCRIPTOR_SET: &'static [u8];

	type Server;
	fn new_server(self) -> Self::Server;

	fn builder(self) -> ServiceBuilder<Self>
	where
		Self::Server: tonic::codegen::Service<Request<Body>, Error = Infallible>
			+ NamedService
			+ Clone
			+ Send
			+ Sync
			+ 'static,
		<Self::Server as tonic::codegen::Service<Request<Body>>>::Response:
			axum::response::IntoResponse,
		<Self::Server as tonic::codegen::Service<Request<Body>>>::Future: Send + 'static,
		Self: Sized,
	{
		ServiceBuilder::new(self)
	}
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
