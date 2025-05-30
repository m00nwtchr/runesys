use std::{convert::Infallible, net::SocketAddr, pin::Pin};

use futures::future::select_all;
#[cfg(feature = "db")]
use sqlx::{PgPool, migrate::MigrateError};
use tokio::{net::TcpListener, task::JoinHandle};
use tonic::{
	body::Body,
	codegen::{Service, http::Request},
	server::NamedService,
	service::Routes,
	transport::Server,
};
use tonic_health::server::health_reporter;
use tower::util::option_layer;
use tower_http::{add_extension::AddExtensionLayer, trace::TraceLayer};
use tracing::info;
use url::Url;

use crate::error::{Error, Result};

type BoxFut<T> = Pin<Box<dyn Future<Output = T> + 'static>>;
type ApplyFn<T> = Box<dyn FnOnce(T) -> T>;
type SetupTask<T> = BoxFut<Result<Option<ApplyFn<T>>>>;

/// A generic microservice builder for gRPC + optional HTTP
pub struct ServiceBuilder<R>
where
	R: crate::Service,
{
	grpc: Routes,
	#[cfg(feature = "http")]
	http: Option<axum::Router>,
	#[cfg(feature = "db")]
	pg_pool: Option<PgPool>,

	setup_tasks: Vec<SetupTask<Self>>,
	handles: Vec<JoinHandle<Result<()>>>,
}

pub struct ServiceState {
	pub health_reporter: tonic_health::server::HealthReporter,
}

#[cfg(debug_assertions)]
pub fn add_reflection_service<S>(r: Routes) -> Result<Routes>
where
	S: crate::Service,
	S::Server: NamedService,
{
	if S::FILE_DESCRIPTOR_SET.is_empty() {
		return Ok(r);
	}
	let reflection = tonic_reflection::server::Builder::configure()
		.register_encoded_file_descriptor_set(S::FILE_DESCRIPTOR_SET)
		.with_service_name(S::Server::NAME)
		.build_v1()?;

	Ok(r.add_service(reflection))
}

#[cfg(not(debug_assertions))]
pub fn add_reflection_service<S>(r: Routes) -> Result<Routes> {
	Ok(r)
}

impl<R> Default for ServiceBuilder<R>
where
	R: crate::Service,
{
	fn default() -> Self {
		Self {
			grpc: Routes::default(),
			#[cfg(feature = "http")]
			http: None,
			#[cfg(feature = "db")]
			pg_pool: None,
			setup_tasks: Vec::new(),
			handles: Vec::new(),
		}
	}
}

impl<R> ServiceBuilder<R>
where
	R: crate::Service + 'static,
	R::Server: NamedService,
{
	/// Initialize tracing, load config, setup health + gRPC address
	pub fn new(svc: R) -> Self
	where
		R::Server: Service<Request<Body>, Error = Infallible> + Clone + Send + Sync + 'static,
		<R::Server as Service<Request<Body>>>::Response: axum::response::IntoResponse,
		<R::Server as Service<Request<Body>>>::Future: Send + 'static,
	{
		crate::tracing::init(&R::INFO);
		crate::config::config();

		let mut s = Self::default().with_service(svc.new_server());
		s.grpc = add_reflection_service::<R>(s.grpc).unwrap();
		s
	}
}

impl<R> ServiceBuilder<R>
where
	R: crate::Service + 'static,
	R::Server: NamedService,
{
	/// Register a tonic gRPC service
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
		self.grpc = self.grpc.add_service(svc);
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
	pub fn with_pg<F, Fut>(mut self, init: F) -> Result<Self>
	where
		F: FnOnce(PgPool) -> Fut + 'static,
		Fut: Future<Output = std::result::Result<(), MigrateError>>,
	{
		let config = crate::config::config();
		let postgres_url = config
			.postgres_url
			.as_ref()
			.map(Url::as_str)
			.ok_or(Error::Config("Postgres URL not set".to_string()))?;

		Ok(self.with_setup_task(Box::pin(async move {
			let pg_pool = sqlx::postgres::PgPoolOptions::new()
				.max_connections(5)
				.connect(postgres_url)
				.await?;
			init(pg_pool.clone()).await?;

			let apply: ApplyFn<Self> = Box::new(move |mut sb| {
				sb.pg_pool = Some(pg_pool);
				sb
			});
			Ok(Some(apply))
		})))
	}

	pub fn with_setup_task(mut self, f: SetupTask<Self>) -> Self {
		self.setup_tasks.push(Box::pin(f));
		self
	}

	pub fn with_task<Fut>(mut self, f: Fut) -> Self
	where
		Fut: Future<Output = Result<()>> + Send + 'static,
	{
		self.handles.push(tokio::spawn(f));
		self
	}

	/// Build and run gRPC + optional HTTP + report
	pub async fn run(mut self) -> Result<()> {
		let config = crate::config::config();

		let patches = futures::future::join_all(self.setup_tasks.drain(..))
			.await
			.into_iter()
			.filter_map(|r| {
				if let Err(r) = &r {
					tracing::error!("{r}");
				}
				r.ok()
			})
			.filter_map(|opt| opt);
		self = patches.fold(self, |acc, patch| patch(acc));

		let health_reporter = {
			let (hr, hs) = health_reporter();
			hr.set_serving::<R::Server>().await;

			self.grpc = self.grpc.add_service(hs);
			hr
		};

		let sb = tower::ServiceBuilder::new()
			.layer(TraceLayer::new_for_grpc())
			.layer(AddExtensionLayer::new(health_reporter.clone()));
		#[cfg(feature = "db")]
		let sb = sb.layer(option_layer(
			self.pg_pool
				.as_ref()
				.map(|pg| AddExtensionLayer::new(pg.clone())),
		));

		// gRPC builder
		let grpc_builder = Server::builder().layer(sb).add_routes(self.grpc);
		let grpc_addr = SocketAddr::new(config.address, config.grpc_port);
		self.handles.push(tokio::spawn(async move {
			info!("{} gRPC at {grpc_addr}", R::INFO.name);
			Ok(grpc_builder.serve(grpc_addr).await?)
		}));

		// combine with HTTP if present
		#[cfg(feature = "http")]
		if let Some(router) = self.http {
			let sb = tower::ServiceBuilder::new()
				.layer(TraceLayer::new_for_http())
				.layer(AddExtensionLayer::new(health_reporter));

			#[cfg(feature = "db")]
			let sb = sb.layer(option_layer(
				self.pg_pool
					.as_ref()
					.map(|pg| AddExtensionLayer::new(pg.clone())),
			));
			let router = router.layer(sb);

			let http_addr = SocketAddr::new(config.address, config.http_port);
			self.handles.push(tokio::spawn(async move {
				info!("{} HTTP at {http_addr}", R::INFO.name);
				Ok(axum::serve(TcpListener::bind(http_addr).await?, router).await?)
			}));
		}

		if self.handles.is_empty() {
			panic!("No services to run");
		}

		let (res, _, remaining) = select_all(self.handles).await;
		for handle in remaining {
			handle.abort();
		}
		res.unwrap()?;

		Ok(())
	}
}
