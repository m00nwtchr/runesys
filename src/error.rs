use thiserror::Error;

#[derive(Error, Debug)]
pub enum Error {
	#[error("sqlx error")]
	Sqlx(#[from] sqlx::error::Error),
	#[error("sqlx migrate error")]
	Migrate(#[from] sqlx::migrate::MigrateError),
	#[error("io error")]
	Io(#[from] std::io::Error),
	#[cfg(feature = "grpc")]
	#[error("transport error")]
	Transport(#[from] tonic::transport::Error),
	#[error("unknown error")]
	Unknown,
}
