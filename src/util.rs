use std::{fmt::Debug, time::Duration};

use tokio::time::sleep;
use tracing::error;

/// Retry an async operation up to `retries` times with a fixed `delay` between attempts.
///
/// - `operation`: a closure returning a `Future` that yields `Result<T, E>`.
/// - `retries`: how many times to retry on failure.
/// - `delay`: how long to wait between retries.
///
/// Returns `Ok(T)` on the first successful attempt, or the last `Err(E)` if all retries fail.
pub async fn retry_async<Op, Fut, T, E>(
	mut operation: Op,
	mut retries: usize,
	delay: Duration,
) -> Result<T, E>
where
	E: Debug,
	Op: FnMut() -> Fut,
	Fut: Future<Output = Result<T, E>>,
{
	loop {
		match operation().await {
			Ok(v) => return Ok(v),
			Err(err) if retries > 0 => {
				retries -= 1;
				error!("Operation failed: {err:?}. Retries left: {retries}",);
				sleep(delay).await;
			}
			Err(err) => return Err(err),
		}
	}
}

pub fn try_from_any<'a, T: TryFrom<&'a prost_types::Any> + prost::Name>(
	any: &'a prost_types::Any,
) -> Result<T, tonic::Status> {
	T::try_from(any).map_err(|_| {
		tonic::Status::invalid_argument(format!(
			"payload is of wrong type, {} expected",
			T::type_url()
		))
	})
}

pub fn interceptor<T>(mutator: impl Fn(&mut T)) -> impl FnMut(T) -> Result<T, tonic::Status> {
	move |mut value: T| {
		mutator(&mut value);
		Ok(value)
	}
}
