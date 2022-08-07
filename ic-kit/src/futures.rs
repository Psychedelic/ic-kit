// Code from Dfinity's ic-cdk because there is no other way to do this xD
//
// This is not from a single file in the ic-cdk, but i jut put this pieces together here, with
// almost zero modifications.
// The only actual logical change that needed to be made in this layer was that the CallFuture does
// not need to deal with deserializing the response or even retrieving the rejection code.
// Our version of CallFuture is None future that can only act as a signal. The logic for getting the
// response using ic0::msg_arg_data_* can be implemented else where depending on the user level
// needs.
use candid::Principal;
use ic_kit_sys::ic0;
use std::future::Future;
use std::pin::Pin;
use std::sync::atomic::{AtomicBool, Ordering};
use std::task::{Context, Poll, Waker};

#[cfg(target_arch = "wasm32-unknown-unknown")]
#[allow(dead_code)]
mod rc {
    use std::cell::{RefCell, RefMut};
    use std::future::Future;
    use std::pin::Pin;
    use std::rc::Rc;
    use std::task::{Context, Poll};

    pub(crate) type InnerCell<T> = RefCell<T>;

    /// A reference counted cell. This is a specific implementation that is
    /// both Send and Sync, but does not rely on Mutex and Arc in WASM as
    /// the actual implementation of Mutex can break in async flows.
    pub(crate) struct WasmCell<T>(Rc<InnerCell<T>>);

    /// In order to be able to have an async method that returns the
    /// result of a call to another canister, we need that result to
    /// be Send + Sync, but Rc and RefCell are not.
    ///
    /// Since inside a canister there isn't actual concurrent access to
    /// the referenced cell or the reference counted container, it is
    /// safe to force these to be Send/Sync.
    unsafe impl<T> Send for WasmCell<T> {}
    unsafe impl<T> Sync for WasmCell<T> {}

    impl<T> WasmCell<T> {
        pub fn new(val: T) -> Self {
            WasmCell(Rc::new(InnerCell::new(val)))
        }
        pub fn into_raw(self) -> *const InnerCell<T> {
            Rc::into_raw(self.0)
        }
        #[allow(clippy::missing_safety_doc)]
        pub unsafe fn from_raw(ptr: *const InnerCell<T>) -> Self {
            Self(Rc::from_raw(ptr))
        }
        pub fn borrow_mut(&self) -> RefMut<'_, T> {
            self.0.borrow_mut()
        }
        pub fn as_ptr(&self) -> *const InnerCell<T> {
            self.0.as_ptr() as *const _
        }
    }

    impl<O, T: Future<Output = O>> Future for WasmCell<T> {
        type Output = O;

        #[allow(unused_mut)]
        fn poll(mut self: Pin<&mut Self>, ctx: &mut Context<'_>) -> Poll<Self::Output> {
            unsafe { Pin::new_unchecked(&mut *self.0.borrow_mut()) }.poll(ctx)
        }
    }

    impl<T> Clone for WasmCell<T> {
        fn clone(&self) -> Self {
            WasmCell(Rc::clone(&self.0))
        }
    }
}

#[cfg(not(target_arch = "wasm32-unknown-unknown"))]
#[allow(dead_code)]
mod rc {
    use std::future::Future;
    use std::pin::Pin;
    use std::sync::{Arc, Mutex, MutexGuard};
    use std::task::{Context, Poll};

    pub(crate) type InnerCell<T> = Mutex<T>;

    /// A reference counted cell. This is a specific implementation that is
    /// both Send and Sync, but does not rely on Mutex and Arc in WASM as
    /// the actual implementation of Mutex can break in async flows.
    pub(crate) struct WasmCell<T>(Arc<InnerCell<T>>);

    impl<T> WasmCell<T> {
        pub fn new(val: T) -> Self {
            WasmCell(Arc::new(InnerCell::new(val)))
        }
        pub fn into_raw(self) -> *const InnerCell<T> {
            Arc::into_raw(self.0)
        }
        #[allow(clippy::missing_safety_doc)]
        pub unsafe fn from_raw(ptr: *const InnerCell<T>) -> Self {
            Self(Arc::from_raw(ptr))
        }
        pub fn borrow_mut(&self) -> MutexGuard<'_, T> {
            self.0.lock().unwrap()
        }
        pub fn as_ptr(&self) -> *const InnerCell<T> {
            Arc::<_>::as_ptr(&self.0)
        }
    }

    impl<O, T: Future<Output = O>> Future for WasmCell<T> {
        type Output = O;

        #[allow(unused_mut)]
        fn poll(mut self: Pin<&mut Self>, ctx: &mut Context<'_>) -> Poll<Self::Output> {
            unsafe { Pin::new_unchecked(&mut *self.0.lock().unwrap()) }.poll(ctx)
        }
    }

    impl<T> Clone for WasmCell<T> {
        fn clone(&self) -> Self {
            WasmCell(Arc::clone(&self.0))
        }
    }
}

use rc::{InnerCell, WasmCell};

struct CallFutureState {
    ready: bool,
    waker: Option<Waker>,
}

/// A simple state-less future that is resolved when any of the call's callbacks are called.
pub(crate) struct CallFuture {
    // We basically use Rc instead of Arc (since we're single threaded), and use
    // RefCell instead of Mutex (because we cannot lock in WASM).
    state: WasmCell<CallFutureState>,
}

impl Future for CallFuture {
    type Output = ();

    fn poll(self: Pin<&mut Self>, context: &mut Context<'_>) -> Poll<Self::Output> {
        let self_ref = Pin::into_ref(self);
        let mut state = self_ref.state.borrow_mut();

        if state.ready {
            Poll::Ready(())
        } else {
            state.waker = Some(context.waker().clone());
            Poll::Pending
        }
    }
}

impl CallFuture {
    /// Resolve the future. This will make the future to be at a Ready state when polled.
    pub fn mark_ready(self) -> Self {
        {
            self.state.borrow_mut().ready = true;
        }
        self
    }

    /// Returns true if the call future is already resolved.
    pub fn is_ready(&self) -> bool {
        self.state.borrow_mut().ready
    }
}

/// Perform a ic0::call_new and return the call future for it. Additionally this method invokes
/// the `ic0::call_on_cleanup` to set the future cleanup method.
pub(crate) unsafe fn call_new(canister_id: Principal, method: &str) -> CallFuture {
    let callee = canister_id.as_slice();
    let state = WasmCell::new(CallFutureState {
        ready: false,
        waker: None,
    });
    let state_ptr = WasmCell::into_raw(state.clone());

    ic0::call_new(
        callee.as_ptr() as isize,
        callee.len() as isize,
        method.as_ptr() as isize,
        method.len() as isize,
        callback as usize as isize,
        state_ptr as isize,
        callback as usize as isize,
        state_ptr as isize,
    );

    ic0::call_on_cleanup(cleanup as usize as isize, state_ptr as isize);

    CallFuture { state }
}

/// The callback from IC dereferences the future from a raw pointer, assigns the
/// result and calls the waker. We cannot use a closure here because we pass raw
/// pointers to the System and back.
fn callback(state_ptr: *const InnerCell<CallFutureState>) {
    let state = unsafe { WasmCell::from_raw(state_ptr) };
    // Make sure to un-borrow_mut the state.
    {
        state.borrow_mut().ready = true;
    }
    let w = state.borrow_mut().waker.take();
    if let Some(waker) = w {
        // This is all to protect this little guy here which will call the poll() which
        // borrow_mut() the state as well. So we need to be careful to not double-borrow_mut.
        waker.wake()
    }
}

/// This function is called when [callback] was just called with the same parameter, and trapped.
/// We can't guarantee internal consistency at this point, but we can at least e.g. drop mutex guards.
/// Waker is a very opaque API, so the best we can do is set a global flag and proceed normally.
fn cleanup(state_ptr: *const InnerCell<CallFutureState>) {
    let state = unsafe { WasmCell::from_raw(state_ptr) };
    // We set the call result, even though it won't be read on the
    // default executor, because we can't guarantee it was called on
    // our executor. However, we are not allowed to inspect
    // reject_code() inside of a cleanup callback, so always set the
    // result to a reject.
    //
    // Borrowing does not trap - the rollback from the
    // previous trap ensures that the WasmCell can be borrowed again.
    state.borrow_mut().ready = true;
    let w = state.borrow_mut().waker.take();
    if let Some(waker) = w {
        // Flag that we do not want to actually wake the task - we
        // want to drop it *without* executing it.
        CLEANUP.store(true, Ordering::Relaxed);
        waker.wake();
        CLEANUP.store(false, Ordering::Relaxed);
    }
}

/// Must be called on every top-level future corresponding to a method call of a
/// canister by the IC.
///
/// Saves the pointer to the future on the heap and kickstarts the future by
/// polling it once. During the polling we also need to provide the waker
/// callback which is triggered after the future made progress.
/// The waker would then poll the future one last time to advance it to
/// the final state. For that, we pass the future pointer to the waker, so that
/// it can be restored into a box from a raw pointer and then dropped if not
/// needed anymore.
///
/// Technically, we store 2 pointers on the heap: the pointer to the future
/// itself, and a pointer to that pointer. The reason for this is that the waker
/// API requires us to pass one thin pointer, while a a pointer to a `dyn Trait`
/// can only be fat. So we create one additional thin pointer, pointing to the
/// fat pointer and pass it instead.
#[inline]
pub fn spawn<F: 'static + Future<Output = ()>>(future: F) {
    let future_ptr = Box::into_raw(Box::new(future));
    let future_ptr_ptr: *mut *mut dyn Future<Output = ()> = Box::into_raw(Box::new(future_ptr));
    let mut pinned_future = unsafe { Pin::new_unchecked(&mut *future_ptr) };
    if pinned_future
        .as_mut()
        .poll(&mut Context::from_waker(&waker::waker(
            future_ptr_ptr as *const (),
        )))
        .is_ready()
    {
        unsafe {
            let _ = Box::from_raw(future_ptr);
            let _ = Box::from_raw(future_ptr_ptr);
        }
    }
}

pub(crate) static CLEANUP: AtomicBool = AtomicBool::new(false);

// This module contains the implementation of a waker we're using for waking
// top-level futures (the ones returned by canister methods). The waker polls
// the future once and re-pins it on the heap, if it's pending. If the future is
// done, we do nothing. Hence, it will be unallocated once we exit the scope and
// we're not interested in the result, as it can only be a unit `()` if the
// waker was used as intended.
mod waker {
    use super::*;
    use std::{
        sync::atomic::Ordering,
        task::{RawWaker, RawWakerVTable, Waker},
    };
    type FuturePtr = *mut dyn Future<Output = ()>;

    static MY_VTABLE: RawWakerVTable = RawWakerVTable::new(clone, wake, wake_by_ref, drop);

    #[inline(always)]
    fn raw_waker(ptr: *const ()) -> RawWaker {
        RawWaker::new(ptr, &MY_VTABLE)
    }

    #[inline(always)]
    fn clone(ptr: *const ()) -> RawWaker {
        raw_waker(ptr)
    }

    // Our waker will be called only if one of the response callbacks is triggered.
    // Then, the waker will restore the future from the pointer we passed into the
    // waker inside the `kickstart` method and poll the future again. If the future
    // is pending, we leave it on the heap. If it's ready, we deallocate the
    // pointer. If CLEANUP is set, then we're recovering from a callback trap, and
    // want to drop the future without executing any more of it.
    #[inline(always)]
    unsafe fn wake(ptr: *const ()) {
        let boxed_future_ptr_ptr = Box::from_raw(ptr as *mut FuturePtr);
        let future_ptr: FuturePtr = *boxed_future_ptr_ptr;
        let boxed_future = Box::from_raw(future_ptr);
        let mut pinned_future = Pin::new_unchecked(&mut *future_ptr);
        if !CLEANUP.load(Ordering::Relaxed)
            && pinned_future
                .as_mut()
                .poll(&mut Context::from_waker(&waker::waker(ptr)))
                .is_pending()
        {
            Box::into_raw(boxed_future_ptr_ptr);
            Box::into_raw(boxed_future);
        }
    }

    #[inline(always)]
    fn wake_by_ref(_: *const ()) {}

    #[inline(always)]
    fn drop(_: *const ()) {}

    #[inline(always)]
    pub fn waker(ptr: *const ()) -> Waker {
        unsafe { Waker::from_raw(raw_waker(ptr)) }
    }
}
