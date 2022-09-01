//! Implementation of a Internet Computer's replica actor model. A replica can contain any number of
//! canisters and a canister, user should be able to send messages to a canister and await for the
//! response of the call. And the canister's should also be able to send messages to another canister.
//!
//! Different canister should operate in parallel, but each canister can only process one request
//! at a time.
//!
//! In this implementation this is done by starting different event loops for each canister and doing
//! cross worker communication using Tokio's mpsc channels, the Replica object itself does not hold
//! the replica's state for this reason, it only owns a mpsc sender that it can use to pass messages
//! to the replica's event loop, so messages are queued one by one.
//!
//! This also allows the canister event loops to have accesses to the replica without any borrows by
//! just sending their request to the same channel, causing the replica to process the messages.

use crate::call::{CallBuilder, CallReply};
use crate::canister::Canister;
use crate::handle::CanisterHandle;
use crate::types::*;
use ic_kit_sys::types::RejectionCode;
use ic_types::Principal;
use std::collections::HashMap;
use std::future::Future;
use std::panic::{RefUnwindSafe, UnwindSafe};
use tokio::sync::{mpsc, oneshot};

/// A local replica that contains one or several canisters.
pub struct Replica {
    // The current implementation uses a `tokio::spawn` to run an event loop for the replica,
    // the state of the replica is store in that event loop.
    sender: mpsc::UnboundedSender<ReplicaMessage>,
}

/// The state of the replica, it does not live inside the replica itself, but an instance of it
/// is created in the replica worker, and messages from the `Replica` are transmitted to this
/// object using an async channel.
#[derive(Default)]
struct ReplicaState {
    /// Map each of the current canisters to the receiver of that canister's event loop.
    canisters: HashMap<Principal, mpsc::UnboundedSender<ReplicaCanisterRequest>>,
}

/// A message that Replica wants to send to a canister to be processed.
struct ReplicaCanisterRequest {
    message: Message,
    reply_sender: Option<oneshot::Sender<CallReply>>,
}

enum ReplicaMessage {
    CanisterAdded {
        canister_id: Principal,
        channel: mpsc::UnboundedSender<ReplicaCanisterRequest>,
    },
    CanisterRequest {
        canister_id: Principal,
        message: Message,
        reply_sender: Option<oneshot::Sender<CallReply>>,
    },
    CanisterReply {
        canister_id: Principal,
        message: Message,
    },
}

impl Replica {
    /// Create a new replica with the given canister.
    pub fn new(canisters: Vec<Canister>) -> Self {
        let tmp = Replica::default();

        for canister in canisters {
            tmp.add_canister(canister);
        }

        tmp
    }

    /// Add the given canister to this replica.
    pub fn add_canister(&self, canister: Canister) -> CanisterHandle {
        let canister_id = canister.id();

        // Create a execution queue for the canister so we can send messages to the canister
        // asynchronously
        let replica = self.sender.clone();

        let (tx, rx) = mpsc::unbounded_channel();
        replica
            .send(ReplicaMessage::CanisterAdded {
                canister_id,
                channel: tx,
            })
            .unwrap_or_else(|_| panic!("ic-kit-runtime: could not send message to replica"));

        // Start the event loop for the canister.
        tokio::spawn(canister_worker(rx, replica, canister));

        CanisterHandle {
            replica: self,
            canister_id,
        }
    }

    /// Return the handle to a canister.
    pub fn get_canister(&self, canister_id: Principal) -> CanisterHandle {
        CanisterHandle {
            replica: &self,
            canister_id,
        }
    }

    /// Enqueue the given request to the destination canister.
    pub(crate) fn enqueue_request(
        &self,
        canister_id: Principal,
        message: Message,
        reply_sender: Option<oneshot::Sender<CallReply>>,
    ) {
        self.sender
            .send(ReplicaMessage::CanisterRequest {
                canister_id,
                message,
                reply_sender,
            })
            .unwrap_or_else(|_| panic!("ic-kit-runtime: could not send message to replica"));
    }

    /// Perform the given call in this replica and return a future that will be resolved once the
    /// call is executed.
    pub(crate) fn perform_call(&self, call: CanisterCall) -> impl Future<Output = CallReply> {
        let canister_id = call.callee;
        let message = Message::from(call);
        let (tx, rx) = oneshot::channel();
        self.enqueue_request(canister_id, message, Some(tx));
        async {
            rx.await
                .expect("ic-kit-runtime: Could not retrieve the response from the call.")
        }
    }

    /// Create a new call builder on the replica, that can be used to send a request to the given
    /// canister.
    pub fn new_call<S: Into<String>>(&self, id: Principal, method: S) -> CallBuilder {
        CallBuilder::new(&self, id, method.into())
    }
}

impl Default for Replica {
    /// Create an empty replica and run the start the event loop.
    fn default() -> Self {
        let (sender, rx) = mpsc::unbounded_channel::<ReplicaMessage>();
        tokio::spawn(replica_worker(rx));
        Replica { sender }
    }
}

/// Run replica's event loop, gets ReplicaMessages and performs the state transition accordingly.
async fn replica_worker(mut rx: mpsc::UnboundedReceiver<ReplicaMessage>) {
    let mut state = ReplicaState::default();

    while let Some(message) = rx.recv().await {
        match message {
            ReplicaMessage::CanisterAdded {
                canister_id,
                channel,
            } => state.canister_added(canister_id, channel),
            ReplicaMessage::CanisterRequest {
                canister_id,
                message,
                reply_sender,
            } => state.canister_request(canister_id, message, reply_sender),
            ReplicaMessage::CanisterReply {
                canister_id,
                message,
            } => state.canister_reply(canister_id, message),
        }
    }
}

/// Start a dedicated event loop for a canister, this will get CanisterMessage messages from a tokio
/// channel and perform
async fn canister_worker(
    mut rx: mpsc::UnboundedReceiver<ReplicaCanisterRequest>,
    mut replica: mpsc::UnboundedSender<ReplicaMessage>,
    mut canister: Canister,
) {
    let canister_id = canister.id();

    let mut rx = rx;
    let mut canister = canister;

    while let Some(message) = rx.recv().await {
        // Perform the message on the canister's thread, the result containing a list of
        // inter-canister call requests is returned here, so we can send each call back to
        // replica.
        let canister_requested_calls = canister
            .process_message(message.message, message.reply_sender)
            .await;

        for call in canister_requested_calls {
            // For each call a oneshot channel is created that is used to receive the response
            // from the target canister. We then await for the response in a `tokio::spawn` to not
            // block the current queue. Once the response is received we send it back as a
            // `CanisterReply` back to the replica so it can perform the routing and send the
            // response.
            // This of course could be avoided if a sender to the same rx was passed to this method.
            // TODO(qti3e) Do the optimization - we don't need to send the result to the replica
            // just so that it queues to our own `rx`.
            let request_id = call.request_id;
            let (tx, rx) = oneshot::channel();

            replica
                .send(ReplicaMessage::CanisterRequest {
                    canister_id: call.callee,
                    message: call.into(),
                    reply_sender: Some(tx),
                })
                .unwrap_or_else(|_| panic!("ic-kit-runtime: could not send message to replica"));

            let rs = replica.clone();

            tokio::spawn(async move {
                let replica = rs;

                // wait for the response from the destination canister.
                let response = rx
                    .await
                    .expect("ic-kit-runtime: Could not get the response of inter-canister call.");

                let message = response.to_message(request_id);

                // once we have the result send it as a request to the current canister.
                replica
                    .send(ReplicaMessage::CanisterReply {
                        canister_id,
                        message,
                    })
                    .unwrap_or_else(|_| {
                        panic!("ic-kit-runtime: could not send message to replica")
                    });
            });
        }
    }
}

impl ReplicaState {
    pub fn canister_added(
        &mut self,
        canister_id: Principal,
        channel: mpsc::UnboundedSender<ReplicaCanisterRequest>,
    ) {
        if self.canisters.contains_key(&canister_id) {
            panic!(
                "Canister '{}' is already defined in the replica.",
                canister_id
            )
        }

        self.canisters.insert(canister_id, channel);
    }

    pub fn canister_request(
        &mut self,
        canister_id: Principal,
        message: Message,
        reply_sender: Option<oneshot::Sender<CallReply>>,
    ) {
        if let Some(chan) = self.canisters.get(&canister_id) {
            chan.send(ReplicaCanisterRequest {
                message,
                reply_sender,
            })
            .unwrap_or_else(|_| panic!("ic-kit-runtime: Could not enqueue the request."));
        } else {
            let cycles_refunded = match message {
                Message::CustomTask { env, .. } => env.cycles_available,
                Message::Request { env, .. } => env.cycles_refunded,
                Message::Reply { .. } => 0,
            };

            reply_sender
                .unwrap()
                .send(CallReply::Reject {
                    rejection_code: RejectionCode::DestinationInvalid,
                    rejection_message: format!("Canister '{}' does not exists", canister_id),
                    cycles_refunded,
                })
                .expect("ic-kit-runtime: Could not send the response.");
        }
    }

    fn canister_reply(&mut self, canister_id: Principal, message: Message) {
        let chan = self.canisters.get(&canister_id).unwrap();
        chan.send(ReplicaCanisterRequest {
            message,
            reply_sender: None,
        })
        .unwrap_or_else(|_| panic!("ic-kit-runtime: Could not enqueue the response request."));
    }
}
