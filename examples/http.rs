use runesys::{Service, service::ServiceBuilder};

#[derive(Service)]
pub struct HelloWorld {}

#[tokio::main]
async fn main() {
	ServiceBuilder::<HelloWorld>::new()
		.with_http(axum::Router::new().route("/", axum::routing::get(|| async { "Hello, World!" })))
		.run()
		.await
		.unwrap();
}
