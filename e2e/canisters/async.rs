use ic_kit::prelude::*;

#[derive(Default)]
struct Resource(u64);

#[derive(Default)]
struct NotificationsReceived(u64);

#[query]
fn inc(n: u64) -> u64 {
    n + 1
}

#[query]
fn invocation_count(r: &Resource) -> u64 {
    r.0
}

#[update]
async fn panic_after_async() {
    let x = with_mut(|r: &mut Resource| {
        r.0 += 1;
        r.0
    });

    CallBuilder::new(id(), "inc")
        .with_arg(x)
        .perform_rejection()
        .await
        .expect("failed to call self");

    ic::trap("Goodbye, cruel world.")
}

#[query]
fn notifications_received(notifications: &NotificationsReceived) -> u64 {
    notifications.0
}

#[update]
fn on_notify(notifications: &mut NotificationsReceived) {
    notifications.0 += 1;
}

#[update]
fn notify(whom: Principal, method: String) {
    CallBuilder::new(whom, method.as_str())
        .perform_one_way()
        .unwrap_or_else(|reject| {
            ic::trap(&format!(
                "failed to notify (callee={}, method={}): {:?}",
                whom, method, reject
            ))
        });
}

#[query]
fn greet(name: String) -> String {
    format!("Hello, {}", name)
}

fn main() {}
