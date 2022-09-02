//! Implementation of a Internet Computer's replica actor model. A replica can contain any number of
//! canisters, a user should be able to send messages to a canister and await for the response of
//! the call. And the any canister should also be able to send messages to another canister.
//!
//! Different canister operate in parallel, but each canister can only process one request at a time.
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
use std::collections::{HashMap, HashSet};
use std::future::Future;
use std::panic::{RefUnwindSafe, UnwindSafe};
use tokio::sync::{mpsc, oneshot};

/// A local replica that contains one or several canisters.
pub struct Replica {
    // The current implementation uses a `tokio::spawn` to run an event loop for the replica,
    // the state of the replica is store in that event loop.
    sender: mpsc::UnboundedSender<ReplicaWorkerMessage>,
}

/// The state of the replica, it does not live inside the replica itself, but an instance of it
/// is created in the replica worker, and messages from the `Replica` are transmitted to this
/// object using an async channel.
struct ReplicaState {
    /// The worker to the current replica state.
    sender: mpsc::UnboundedSender<ReplicaWorkerMessage>,
    /// Map each of the current canisters to the receiver of that canister's event loop.
    canisters: HashMap<Principal, mpsc::UnboundedSender<CanisterWorkerMessage>>,
    /// The reserved canister principal ids.
    created: HashSet<Principal>,
}

/// A message received by the canister worker.
enum CanisterWorkerMessage {
    Message {
        message: CanisterMessage,
        reply_sender: Option<oneshot::Sender<CallReply>>,
    },
}

/// A message received by the replica worker.
enum ReplicaWorkerMessage {
    CreateCanister {
        reply_sender: oneshot::Sender<Principal>,
    },
    InstallCode {
        canister: Canister,
    },
    CanisterRequest {
        canister_id: Principal,
        message: CanisterMessage,
        reply_sender: Option<oneshot::Sender<CallReply>>,
    },
    CanisterReply {
        canister_id: Principal,
        message: CanisterMessage,
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

        self.sender
            .send(ReplicaWorkerMessage::InstallCode { canister })
            .unwrap_or_else(|_| panic!("ic-kit-runtime: could not send message to replica"));

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
        message: CanisterMessage,
        reply_sender: Option<oneshot::Sender<CallReply>>,
    ) {
        self.sender
            .send(ReplicaWorkerMessage::CanisterRequest {
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
        let message = CanisterMessage::from(call);
        let (tx, rx) = oneshot::channel();
        self.enqueue_request(canister_id, message, Some(tx));
        async {
            rx.await.unwrap_or_else(|e| {
                panic!(
                    "ic-kit-runtime: Could not retrieve the response from the call. {:?}",
                    e
                )
            })
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
        let (sender, rx) = mpsc::unbounded_channel::<ReplicaWorkerMessage>();
        tokio::spawn(replica_worker(sender.clone(), rx));
        Replica { sender }
    }
}

/// Run replica's event loop, gets ReplicaMessages and performs the state transition accordingly.
async fn replica_worker(
    sender: mpsc::UnboundedSender<ReplicaWorkerMessage>,
    mut rx: mpsc::UnboundedReceiver<ReplicaWorkerMessage>,
) {
    let mut state = ReplicaState {
        sender,
        canisters: Default::default(),
        created: Default::default(),
    };

    while let Some(message) = rx.recv().await {
        match message {
            ReplicaWorkerMessage::CreateCanister { reply_sender } => {
                let id = state.create_canister();
                reply_sender
                    .send(id)
                    .expect("Could not send back to result for the canister create request.");
            }
            ReplicaWorkerMessage::InstallCode { canister } => {
                state.install_code(canister);
            }
            ReplicaWorkerMessage::CanisterRequest {
                canister_id,
                message,
                reply_sender,
            } => state.canister_request(canister_id, message, reply_sender),
            ReplicaWorkerMessage::CanisterReply {
                canister_id,
                message,
            } => state.canister_reply(canister_id, message),
        }
    }
}

/// Start a dedicated event loop for a canister, this will get CanisterMessage messages from a tokio
/// channel and perform
async fn canister_worker(
    mut rx: mpsc::UnboundedReceiver<CanisterWorkerMessage>,
    mut replica: mpsc::UnboundedSender<ReplicaWorkerMessage>,
    mut canister: Canister,
) {
    while let Some(message) = rx.recv().await {
        match message {
            CanisterWorkerMessage::Message {
                message,
                reply_sender,
            } => perform_canister_request(&mut canister, &mut replica, message, reply_sender).await,
        };
    }
}

async fn perform_canister_request(
    canister: &mut Canister,
    replica: &mut mpsc::UnboundedSender<ReplicaWorkerMessage>,
    message: CanisterMessage,
    reply_sender: Option<oneshot::Sender<CallReply>>,
) {
    let canister_id = canister.id();

    // Perform the message on the canister's thread, the result containing a list of
    // inter-canister call requests is returned here, so we can send each call back to
    // replica.
    let canister_requested_calls = canister.process_message(message, reply_sender).await;

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
            .send(ReplicaWorkerMessage::CanisterRequest {
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
                .send(ReplicaWorkerMessage::CanisterReply {
                    canister_id,
                    message,
                })
                .unwrap_or_else(|_| panic!("ic-kit-runtime: could not send message to replica"));
        });
    }
}

impl ReplicaState {
    /// Return the first unused canister id.
    fn get_next_canister_id(&mut self) -> Principal {
        let mut id = self.created.len() as u64;

        loop {
            let canister_id = canister_id(id);

            if !self.canisters.contains_key(&canister_id) {
                break canister_id;
            }

            id += 1;
        }
    }

    /// Create a new canister by reserving a canister id.
    pub fn create_canister(&mut self) -> Principal {
        let canister_id = self.get_next_canister_id();
        self.created.insert(canister_id);
        canister_id
    }

    /// Install the given canister.
    pub fn install_code(&mut self, canister: Canister) {
        let canister_id = canister.id();

        if self.canisters.contains_key(&canister_id) {
            panic!(
                "Canister '{}' is already defined in the replica.",
                canister_id
            )
        }

        let (tx, rx) = mpsc::unbounded_channel();
        tokio::spawn(canister_worker(rx, self.sender.clone(), canister));

        self.canisters.insert(canister_id, tx);
    }

    pub fn canister_request(
        &mut self,
        canister_id: Principal,
        message: CanisterMessage,
        reply_sender: Option<oneshot::Sender<CallReply>>,
    ) {
        if let Some(chan) = self.canisters.get(&canister_id) {
            chan.send(CanisterWorkerMessage::Message {
                message,
                reply_sender,
            })
            .unwrap_or_else(|_| panic!("ic-kit-runtime: Could not enqueue the request."));
        } else {
            let cycles_refunded = match message {
                CanisterMessage::CustomTask { env, .. } => env.cycles_available,
                CanisterMessage::Request { env, .. } => env.cycles_refunded,
                CanisterMessage::Reply { .. } => 0,
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

    fn canister_reply(&mut self, canister_id: Principal, message: CanisterMessage) {
        let chan = self.canisters.get(&canister_id).unwrap();
        chan.send(CanisterWorkerMessage::Message {
            message,
            reply_sender: None,
        })
        .unwrap_or_else(|_| panic!("ic-kit-runtime: Could not enqueue the response request."));
    }
}

const fn canister_id(id: u64) -> Principal {
    let mut data = [0_u8; 10];

    // Specify explicitly the length, so as to assert at compile time that a u64
    // takes exactly 8 bytes
    let val: [u8; 8] = id.to_be_bytes();

    // for-loops in const fn are not supported
    data[0] = val[0];
    data[1] = val[1];
    data[2] = val[2];
    data[3] = val[3];
    data[4] = val[4];
    data[5] = val[5];
    data[6] = val[6];
    data[7] = val[7];

    // Even though not defined in the interface spec, add another 0x1 to the array
    // to create a sub category that could be used in future.
    data[8] = 0x01;

    // Add the Principal's TYPE_OPAQUE tag.
    data[9] = 0x01;

    Principal::from_slice(&data)
}
