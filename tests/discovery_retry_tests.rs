use extras_fetcher::discovery::retry_with_backoff;
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
