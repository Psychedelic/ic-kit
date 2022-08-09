use crate::canister::Canister;
use crate::types::*;
use ic_types::Principal;
use std::collections::HashMap;
use std::future::Future;
use tokio::sync::{mpsc, oneshot};

/// A local replica that contains one or several canisters.
pub struct Replica {
    sender: mpsc::UnboundedSender<ReplicaMessage>,
}

/// A message we want to send to a canister.
struct CanisterMessage {
    message: Message,
    reply_sender: Option<oneshot::Sender<CanisterReply>>,
}

enum ReplicaMessage {
    CanisterAdded {
        canister_id: Principal,
        channel: mpsc::UnboundedSender<CanisterMessage>,
    },
    CanisterRequest {
        canister_id: Principal,
        message: Message,
        reply_sender: oneshot::Sender<CanisterReply>,
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
    pub fn add_canister(&self, canister: Canister) {
        // Create a execution queue for the canister so we can send messages to the canister
        // asynchronously
        let replica_sender = self.sender.clone();
        let (tx, rx) = mpsc::unbounded_channel();
        replica_sender
            .send(ReplicaMessage::CanisterAdded {
                canister_id: canister.id(),
                channel: tx,
            })
            .unwrap_or_else(|_| panic!("ic-kit-runtime: could not send message to replica"));

        // Start the event loop for the canister.
        tokio::spawn(async move {
            let mut rx = rx;
            let mut canister = canister;
            let canister_id = canister.id();

            while let Some(message) = rx.recv().await {
                let perform_call = canister
                    .process_message(message.message, message.reply_sender)
                    .await;

                for call in perform_call {
                    let request_id = call.request_id;
                    let (tx, rx) = oneshot::channel();

                    replica_sender
                        .send(ReplicaMessage::CanisterRequest {
                            canister_id: call.callee,
                            message: call.into(),
                            reply_sender: tx,
                        })
                        .unwrap_or_else(|_| {
                            panic!("ic-kit-runtime: could not send message to replica")
                        });

                    let rs = replica_sender.clone();
                    tokio::spawn(async move {
                        let replica_sender = rs;

                        // wait for the response from the destination canister.
                        let response = rx.await.expect(
                            "ic-kit-runtime: Could not get the response of inter-canister call.",
                        );

                        let message = response.to_message(request_id);

                        replica_sender
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
        });
    }

    /// Enqueue the given request to the destination canister.
    pub fn enqueue_request(
        &self,
        canister_id: Principal,
        message: Message,
        reply_sender: oneshot::Sender<CanisterReply>,
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
    pub fn perform(&self, call: CanisterCall) -> impl Future<Output = CanisterReply> {
        let canister_id = call.callee;
        let message = Message::from(call);
        let (tx, rx) = oneshot::channel();
        self.enqueue_request(canister_id, message, tx);
        async {
            rx.await
                .expect("ic-kit-runtime: Could not retrieve the response from the call.")
        }
    }
}

impl Default for Replica {
    fn default() -> Self {
        let (sender, rx) = mpsc::unbounded_channel::<ReplicaMessage>();

        tokio::spawn(async move {
            let mut rx = rx;
            let mut canisters = HashMap::<Principal, mpsc::UnboundedSender<CanisterMessage>>::new();

            while let Some(m) = rx.recv().await {
                match m {
                    ReplicaMessage::CanisterAdded {
                        canister_id,
                        channel,
                    } => {
                        canisters.insert(canister_id, channel);
                    }
                    ReplicaMessage::CanisterRequest {
                        canister_id,
                        message,
                        reply_sender,
                    } => {
                        if let Some(chan) = canisters.get(&canister_id) {
                            chan.send(CanisterMessage {
                                message,
                                reply_sender: Some(reply_sender),
                            })
                            .unwrap_or_else(|_| {
                                panic!("ic-kit-runtime: Could not enqueue the request.")
                            });
                        } else {
                            let cycles_refunded = match message {
                                Message::CustomTask { env, .. } => env.cycles_available,
                                Message::Request { env, .. } => env.cycles_refunded,
                                Message::Reply { .. } => 0,
                            };

                            reply_sender
                                .send(CanisterReply::Reject {
                                    rejection_code: RejectionCode::DestinationInvalid,
                                    rejection_message: format!(
                                        "Canister '{}' does not exists",
                                        canister_id
                                    ),
                                    cycles_refunded,
                                })
                                .expect("ic-kit-runtime: Could not send the response.");
                        }
                    }
                    ReplicaMessage::CanisterReply {
                        canister_id,
                        message,
                    } => {
                        let chan = canisters.get(&canister_id).unwrap();
                        chan.send(CanisterMessage {
                            message,
                            reply_sender: None,
                        })
                        .unwrap_or_else(|_| {
                            panic!("ic-kit-runtime: Could not enqueue the response request.")
                        });
                    }
                }
            }
        });

        Replica { sender }
    }
}
