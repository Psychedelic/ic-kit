use crate::call::{CallBuilder, CallReply};
use crate::types::{Env, Message, RequestId};
use crate::Replica;
use ic_types::Principal;
use std::panic::{RefUnwindSafe, UnwindSafe};
use tokio::sync::oneshot;

pub struct CanisterHandle<'a> {
    pub(crate) replica: &'a Replica,
    pub(crate) canister_id: Principal,
}

impl<'a> CanisterHandle<'a> {
    /// Create a new call builder to call this canister.
    pub fn new_call<S: Into<String>>(&self, method_name: S) -> CallBuilder {
        CallBuilder::new(self.replica, self.canister_id, method_name.into())
    }

    /// Run the given custom function in the execution thread of the canister.
    pub async fn custom<F: FnOnce() + Send + RefUnwindSafe + UnwindSafe + 'static>(
        &self,
        f: F,
        env: Env,
    ) -> CallReply {
        let (tx, rx) = oneshot::channel();

        self.replica.enqueue_request(
            self.canister_id,
            Message::CustomTask {
                request_id: RequestId::new(),
                task: Box::new(f),
                env,
            },
            Some(tx),
        );

        rx.await.unwrap()
    }

    /// Run the given raw message in the canister's execution thread.
    pub async fn run_env(&self, env: Env) -> CallReply {
        let (tx, rx) = oneshot::channel();

        self.replica.enqueue_request(
            self.canister_id,
            Message::Request {
                request_id: RequestId::new(),
                env,
            },
            Some(tx),
        );

        rx.await.unwrap()
    }

    /// Runs the init hook of the canister. For more customization use [`CanisterHandle::run_env`]
    /// with [`Env::init()`].
    pub async fn init(&self) -> CallReply {
        self.run_env(Env::init()).await
    }

    /// Runs the pre_upgrade hook of the canister. For more customization use
    /// [`CanisterHandle::run_env`] with [`Env::pre_upgrade()`].
    pub async fn pre_upgrade(&self) -> CallReply {
        self.run_env(Env::pre_upgrade()).await
    }

    /// Runs the post_upgrade hook of the canister. For more customization use
    /// [`CanisterHandle::run_env`] with [`Env::post_upgrade()`].
    pub async fn post_upgrade(&self) -> CallReply {
        self.run_env(Env::post_upgrade()).await
    }

    /// Runs the post_upgrade hook of the canister. For more customization use
    /// [`CanisterHandle::run_env`] with [`Env::heartbeat()`].
    pub async fn heartbeat(&self) -> CallReply {
        self.run_env(Env::heartbeat()).await
    }
}
