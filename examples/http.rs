use runesys::{Service, service::ServiceBuilder};

#[derive(Service)]
#[service("Hello-World")]
pub struct HelloWorld;

#[tokio::main]
async fn main() {
	ServiceBuilder::<HelloWorld>::new()
		.unwrap()
		.with_http(axum::Router::new().route("/", axum::routing::get(|| async { "Hello, World!" })))
		.run()
		.await
		.unwrap();
}
