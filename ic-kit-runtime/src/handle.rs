use futures::executor::block_on;
use ic0::Ic0CallHandler;
use ic_kit_sys::ic0;
use ic_kit_sys::ic0::runtime::*;
use std::panic::set_hook;
use std::thread::sleep;
use std::time::Duration;

#[test]
fn x() {
    let (mut response_tx, rx) = tokio::sync::mpsc::channel(8);
    let (tx, mut request_rx) = tokio::sync::mpsc::channel(8);

    std::thread::spawn(|| {
        let handle = RuntimeHandle::new(rx, tx);
        ic0::register_handler(handle);
        set_hook(Box::new(|_| println!("Canister got trapped xD")));

        loop {
            sleep(Duration::from_secs(1));
            let size = unsafe { ic0::msg_arg_data_size() };
            println!("Canister: Message size = {}", size);
        }
    });

    let mut counter = 0;

    block_on(async {
        while let Some(req) = request_rx.recv().await {
            counter += 1;

            if counter == 5 {
                response_tx
                    .send(Response::Trap)
                    .await
                    .expect("Could not send message to canister.");
            }

            match req {
                Request::msg_arg_data_size {} => {
                    response_tx
                        .send(Response::Isize(100))
                        .await
                        .expect("Could not send message to canister.");
                }
                _ => unimplemented!(),
            }
        }

        println!("We shut down the canister but still can run form here!");
    })
}
