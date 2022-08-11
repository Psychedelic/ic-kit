use crate::futures;

/// Execute a future without blocking the current call. The given future is polled once initially
/// to kickstart the async calls.
#[inline(always)]
pub fn spawn<F: 'static + std::future::Future<Output = ()>>(future: F) {
    futures::spawn(future)
}
