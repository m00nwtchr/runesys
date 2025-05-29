use runesys::Service;
use tonic::{Request, Response, Status, server::NamedService};
use tonic_health::{
	pb::{HealthCheckRequest, HealthCheckResponse, health_server::HealthServer},
	server::WatchStream,
};
use tracing::info;

#[derive(Service)]
#[server(HealthServer)]
pub struct HelloWorld {}

impl NamedService for HelloWorld {
	const NAME: &'static str = "Health";
}

#[tonic::async_trait]
impl tonic_health::pb::health_server::Health for HelloWorld {
	async fn check(
		&self,
		request: Request<HealthCheckRequest>,
	) -> Result<Response<HealthCheckResponse>, Status> {
		todo!()
	}

	type WatchStream = WatchStream;

	async fn watch(
		&self,
		request: Request<HealthCheckRequest>,
	) -> Result<Response<Self::WatchStream>, Status> {
		todo!()
	}
}

#[tokio::main]
async fn main() {
	HelloWorld {}
		.builder()
		// .with_http(axum::Router::new().route("/", axum::routing::get(|| async { "Hello, World!" })))
		.with_task(async {
			loop {}

			Ok(())
		})
		.run()
		.await
		.unwrap();
}
