use thiserror::Error;

pub(crate) type Result<T> = std::result::Result<T, crate::error::Error>;

#[derive(Error, Debug)]
pub enum Error {
	#[error("config error: {0}")]
	Config(String),

	#[error("transport error")]
	Transport(#[from] tonic::transport::Error),
	#[error("reflection error")]
	Reflection(#[from] tonic_reflection::server::Error),

	#[error("io error")]
	Io(#[from] std::io::Error),

	#[cfg(feature = "db")]
	#[error("sqlx error")]
	Sqlx(#[from] sqlx::error::Error),
	#[cfg(feature = "db")]
	#[error("sqlx migrate error")]
	Migrate(#[from] sqlx::migrate::MigrateError),

	#[error(transparent)]
	Other(#[from] Box<dyn std::error::Error + Send + Sync>),
}
