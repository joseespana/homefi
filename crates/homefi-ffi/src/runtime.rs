//! The tokio runtime every async export runs on.
//!
//! UniFFI polls a Rust future on the thread of whoever awaits it: from Kotlin
//! that is often `Dispatchers.Main` (`viewModelScope.launch {}`). Exports
//! therefore do nothing but hand their body to [`run`], which spawns it on
//! [`RT`], an owned multi-thread runtime: parsing, mapping and disk work
//! between awaits never runs on the app's main thread, and one slow call
//! cannot stall the others (as it could on a single-thread fallback runtime).

use std::future::Future;
use std::pin::Pin;
use std::sync::LazyLock;
use std::task::{Context, Poll};

use tokio::runtime::Runtime;
use tokio::task::JoinHandle;

/// Phones: a few workers are plenty for network I/O and small CPU work
/// (tags, cover decoding and scans go to the blocking pool).
const MIN_WORKERS: usize = 2;
const MAX_WORKERS: usize = 4;
const MAX_BLOCKING: usize = 32;

/// Shared by every export, the share hub's server and the guest connection.
pub(crate) static RT: LazyLock<Runtime> = LazyLock::new(|| {
    let workers =
        std::thread::available_parallelism().map(|n| n.get()).unwrap_or(MIN_WORKERS).clamp(MIN_WORKERS, MAX_WORKERS);
    tokio::runtime::Builder::new_multi_thread()
        .worker_threads(workers)
        .max_blocking_threads(MAX_BLOCKING)
        .thread_name("homefi-rt")
        .enable_all()
        .build()
        // no runtime means no app at all: there is nothing to fall back to
        .expect("the HomeFi runtime starts")
});

/// Runs `fut` on [`RT`] and waits for it from any executor (or none, from a
/// plain thread with `block_on`). Dropping the returned future, as a
/// cancelled coroutine does, cancels the task too. A panic in the task is
/// passed on to the caller (UniFFI turns it into an exception).
pub(crate) async fn run<F>(fut: F) -> F::Output
where
    F: Future + Send + 'static,
    F::Output: Send + 'static,
{
    match (AbortOnDrop(RT.spawn(fut))).await {
        Ok(out) => out,
        Err(e) if e.is_panic() => std::panic::resume_unwind(e.into_panic()),
        // only when the runtime shuts down, which a static one never does
        Err(e) => std::panic::panic_any(e.to_string()),
    }
}

/// A task handle that cancels its task when dropped.
struct AbortOnDrop<T>(JoinHandle<T>);

impl<T> Future for AbortOnDrop<T> {
    type Output = Result<T, tokio::task::JoinError>;

    fn poll(mut self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<Self::Output> {
        Pin::new(&mut self.0).poll(cx)
    }
}

impl<T> Drop for AbortOnDrop<T> {
    fn drop(&mut self) {
        self.0.abort();
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::atomic::{AtomicBool, Ordering};
    use std::sync::Arc;
    use std::time::Duration;

    #[test]
    fn runs_tokio_work_from_a_plain_thread_off_that_thread() {
        let caller = std::thread::current().id();
        let (ran_on, slept) = futures::executor::block_on(run(async move {
            // timers, spawn_blocking and spawn need a tokio context
            tokio::time::sleep(Duration::from_millis(5)).await;
            let blocking = tokio::task::spawn_blocking(|| 2).await.unwrap();
            let spawned = tokio::spawn(async { 3 }).await.unwrap();
            (std::thread::current().id(), blocking + spawned)
        }));
        assert_ne!(ran_on, caller, "the body runs on the runtime, not on the caller's thread");
        assert_eq!(slept, 5);
    }

    #[test]
    fn dropping_the_call_cancels_the_task() {
        let finished = Arc::new(AtomicBool::new(false));
        let flag = finished.clone();
        let call = run(async move {
            tokio::time::sleep(Duration::from_millis(200)).await;
            flag.store(true, Ordering::SeqCst);
        });
        // poll once so it is spawned, then cancel (a cancelled coroutine)
        let mut call = Box::pin(call);
        let waker = futures::task::noop_waker();
        assert!(call.as_mut().poll(&mut Context::from_waker(&waker)).is_pending());
        drop(call);
        std::thread::sleep(Duration::from_millis(400));
        assert!(!finished.load(Ordering::SeqCst));
    }

    #[test]
    #[should_panic(expected = "boom")]
    fn panics_reach_the_caller() {
        futures::executor::block_on(run(async { panic!("boom") }));
    }
}
