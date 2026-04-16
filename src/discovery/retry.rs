use log::debug;
use std::future::Future;
use std::time::Duration;

pub async fn retry_with_backoff<T, E, F, Fut>(
    max_retries: u8,
    base_delay_ms: u64,
    mut operation: F,
) -> Result<T, E>
where
    E: std::fmt::Debug + std::fmt::Display,
    F: FnMut() -> Fut,
    Fut: Future<Output = Result<T, E>>,
{
    let mut last_error = None;

    for attempt in 0..max_retries {
        match operation().await {
            Ok(result) => return Ok(result),
            Err(e) => {
                last_error = Some(e);
                if attempt < max_retries - 1 {
                    let delay_ms = base_delay_ms * 2u64.pow(attempt as u32);
                    debug!("Retry attempt {} after {}ms", attempt + 1, delay_ms);
                    tokio::time::sleep(Duration::from_millis(delay_ms)).await;
                }
            }
        }
    }

    Err(last_error.unwrap())
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::Arc;
    use std::sync::atomic::{AtomicUsize, Ordering};

    #[tokio::test]
    async fn test_retry_success_on_first_try() {
        let attempt_count = Arc::new(AtomicUsize::new(0));
        let count = attempt_count.clone();

        let result: Result<&str, std::io::Error> = retry_with_backoff(3, 100, || async {
            count.fetch_add(1, Ordering::SeqCst);
            Ok::<_, std::io::Error>("success")
        })
        .await;

        assert!(result.is_ok());
        assert_eq!(attempt_count.load(Ordering::SeqCst), 1);
    }

    #[tokio::test]
    async fn test_retry_eventually_succeeds() {
        let attempt_count = Arc::new(AtomicUsize::new(0));
        let count = attempt_count.clone();

        let result: Result<&str, std::io::Error> = retry_with_backoff(3, 10, || async {
            count.fetch_add(1, Ordering::SeqCst);
            if count.load(Ordering::SeqCst) < 2 {
                Err(std::io::Error::new(std::io::ErrorKind::Other, "transient"))
            } else {
                Ok("success")
            }
        })
        .await;

        assert!(result.is_ok());
        assert_eq!(attempt_count.load(Ordering::SeqCst), 2);
    }

    #[tokio::test]
    async fn test_retry_exhausts_all_attempts() {
        let attempt_count = Arc::new(AtomicUsize::new(0));
        let count = attempt_count.clone();

        let result: Result<&str, std::io::Error> = retry_with_backoff(3, 10, || async {
            count.fetch_add(1, Ordering::SeqCst);
            Err(std::io::Error::new(std::io::ErrorKind::Other, "permanent"))
        })
        .await;

        assert!(result.is_err());
        assert_eq!(attempt_count.load(Ordering::SeqCst), 3);
    }
}
