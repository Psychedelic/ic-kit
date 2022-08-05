/// Execute a future without blocking the current call.
#[inline(always)]
pub fn spawn<F: 'static + std::future::Future<Output = ()>>(_future: F) {
    todo!()
}
