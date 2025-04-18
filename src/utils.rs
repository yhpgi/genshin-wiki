use crate::error::{AppError, AppResult};
use tokio::sync::{Semaphore, SemaphorePermit};
use tokio::task;

pub async fn run_blocking<F, T>(func: F) -> AppResult<T>
where
    F: FnOnce() -> AppResult<T> + Send + 'static,
    T: Send + 'static,
{
    match task::spawn_blocking(func).await {
        Ok(Ok(res)) => Ok(res),
        Ok(Err(e)) => Err(e),
        Err(e) => Err(AppError::from(e)),
    }
}

pub async fn run_cpu_intensive<F, T>(func: F) -> AppResult<T>
where
    F: FnOnce() -> T + Send + 'static,
    T: Send + 'static,
{
    match task::spawn_blocking(func).await {
        Ok(res) => Ok(res),
        Err(e) => Err(AppError::from(e)),
    }
}

pub async fn acquire_semaphore<'a>(
    semaphore: &'a Semaphore,
    context: &str,
) -> AppResult<SemaphorePermit<'a>> {
    semaphore
        .acquire()
        .await
        .map_err(|e| AppError::SemaphoreAcquire(format!("Failed for '{}': {}", context, e)))
}
