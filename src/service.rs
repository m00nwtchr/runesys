use std::{convert::Infallible, marker::PhantomData, net::SocketAddr};

use futures::future::select_all;
#[cfg(feature = "db")]
use sqlx::{PgPool, migrate::MigrateError};
use tokio::{net::TcpListener, task::JoinHandle};
#[cfg(feature = "grpc")]
use tonic::{
	body::Body,
	codegen::{Service, http::Request},
	server::NamedService,
	service::Routes,
	transport::Server,
};
#[cfg(feature = "grpc")]
use tonic_health::server::health_reporter;
#[cfg(feature = "common")]
use tower::util::option_layer;
#[cfg(feature = "common")]
use tower_http::{add_extension::AddExtensionLayer, trace::TraceLayer};
use tracing::info;

use crate::ServiceInfo;

pub type Result<T> = std::result::Result<T, crate::error::Error>;

/// A generic microservice builder for gRPC + optional HTTP
pub struct ServiceBuilder<R>
where
	R: crate::Service,
{
	info: ServiceInfo,

	#[cfg(feature = "grpc")]
	grpc: Option<Routes>,
	#[cfg(feature = "http")]
	http: Option<axum::Router>,
	#[cfg(feature = "db")]
	pg_pool: Option<sqlx::PgPool>,

	_inner: PhantomData<R>,
}

pub struct ServiceState {
	#[cfg(feature = "grpc")]
	pub health_reporter: tonic_health::server::HealthReporter,
}

impl<R> ServiceBuilder<R>
where
	R: crate::Service,
{
	/// Initialize tracing, load config, setup health + gRPC address
	pub fn new(info: ServiceInfo) -> Result<Self> {
		crate::tracing::init(&info);
		crate::config::config(&info);

		Ok(Self {
			info,
			#[cfg(feature = "grpc")]
			grpc: None,

			#[cfg(feature = "http")]
			http: None,

			#[cfg(feature = "db")]
			pg_pool: None,

			_inner: PhantomData,
		})
	}

	/// Register a tonic gRPC service
	#[cfg(feature = "grpc")]
	pub fn with_service<S>(mut self, svc: S) -> Self
	where
		S: Service<Request<Body>, Error = Infallible>
			+ NamedService
			+ Clone
			+ Send
			+ Sync
			+ 'static,
		S::Response: axum::response::IntoResponse,
		S::Future: Send + 'static,
	{
		self.grpc = Some(match self.grpc {
			Some(routes) => routes.add_service(svc),
			None => Routes::new(svc),
		});

		self
	}

	/// Add an HTTP endpoint alongside gRPC
	#[cfg(feature = "http")]
	pub fn with_http<T>(mut self, router: T) -> Self
	where
		T: Send + 'static,
		axum::Router: From<T>,
	{
		self.http = Some(axum::Router::from(router));
		self
	}

	/// Add postgres database connection
	#[cfg(feature = "db")]
	pub async fn with_pg<F, Fut>(mut self, init: F) -> Result<Self>
	where
		F: FnOnce(PgPool) -> Fut,
		Fut: Future<Output = std::result::Result<(), MigrateError>>,
	{
		let config = crate::config::config(&self.info);
		let pg_pool = sqlx::postgres::PgPoolOptions::new()
			.max_connections(5)
			.connect(config.postgres_url.as_str())
			.await?;
		init(pg_pool.clone()).await?;
		self.pg_pool = Some(pg_pool);
		Ok(self)
	}

	/// Build and run gRPC + optional HTTP + report
	pub async fn run(mut self) -> Result<()> {
		let config = crate::config::config(&self.info);

		let mut handles: Vec<JoinHandle<Result<()>>> = Vec::new();

		#[cfg(feature = "grpc")]
		let health_reporter = if self.grpc.is_some() {
			let (hr, hs) = health_reporter();

			self.grpc = Some(self.grpc.unwrap().add_service(hs));
			Some(hr)
		} else {
			None
		};

		#[cfg(feature = "grpc")]
		if let Some(grpc) = self.grpc {
			// gRPC builder
			let grpc_builder = Server::builder()
				.layer({
					let mut sb = tower::ServiceBuilder::new()
						.layer(TraceLayer::new_for_grpc())
						.layer(AddExtensionLayer::new(
							health_reporter.as_ref().unwrap().clone(),
						));

					#[cfg(feature = "db")]
					let sb = sb.layer(option_layer(
						self.pg_pool
							.as_ref()
							.map(|pg| AddExtensionLayer::new(pg.clone())),
					));

					sb
				})
				.add_routes(grpc);

			let grpc_addr = SocketAddr::new(config.address, config.grpc_port);
			handles.push(tokio::spawn(async move {
				info!("{} gRPC at {}", self.info.name, grpc_addr);
				Ok(grpc_builder.serve(grpc_addr).await?)
			}));
		}

		// combine with HTTP if present
		#[cfg(feature = "http")]
		if let Some(router) = self.http {
			let router = router.layer({
				let sb = tower::ServiceBuilder::new().layer(TraceLayer::new_for_http());

				#[cfg(feature = "grpc")]
				let sb = sb.layer(option_layer(
					health_reporter
						.as_ref()
						.map(|hr| AddExtensionLayer::new(hr.clone())),
				));

				#[cfg(feature = "db")]
				let sb = sb.layer(option_layer(
					self.pg_pool
						.as_ref()
						.map(|pg| AddExtensionLayer::new(pg.clone())),
				));

				sb
			});

			let http_addr = SocketAddr::new(config.address, config.http_port);
			handles.push(tokio::spawn(async move {
				info!("{} HTTP at {}", self.info.name, http_addr);
				Ok(axum::serve(TcpListener::bind(http_addr).await?, router).await?)
			}));
		}

		let (res, _, remaining) = select_all(handles).await;
		for handle in remaining {
			handle.abort();
		}
		res.unwrap()?;

		Ok(())
	}
}
