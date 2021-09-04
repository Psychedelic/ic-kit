use crate::inject::{get_context, inject};
use crate::interface::{CallResponse, Context};
use ic_cdk::api::call::CallResult;
use ic_cdk::export::candid::utils::{ArgumentDecoder, ArgumentEncoder};
use ic_cdk::export::candid::{decode_args, encode_args};
use ic_cdk::export::{candid, Principal};
use serde::Serialize;
use std::any::{Any, TypeId};
use std::collections::BTreeMap;
use std::time::{SystemTime, UNIX_EPOCH};

/// A context that could be used to fake/control the behaviour of the IC when testing the canister.
pub struct MockContext {
    /// ID of the current canister.
    id: Principal,
    /// The balance of the canister. By default set to 100TC.
    balance: u64,
    /// The caller principal passed to the calls, by default `anonymous` is used.
    caller: Principal,
    /// Determines if a call was made or not.
    is_reply_callback_mode: bool,
    /// Whatever the canister called trap or not.
    trapped: bool,
    /// Available cycles sent by the caller.
    cycles: u64,
    /// Cycles refunded by the previous call.
    cycles_refunded: u64,
    /// The storage tree for the current context.
    storage: BTreeMap<TypeId, Box<dyn Any>>,
    /// The stable storage data.
    stable: Vec<u8>,
    /// The certified data.
    certified_data: Option<Vec<u8>>,
    /// The canisters defined in this context.
    canisters: BTreeMap<Principal, MockCanister>,
    /// The default handler which gets called when the canister is not found.
    default_handler: Option<Box<dyn Fn(&mut MockContext, String, Vec<u8>) -> CallResult<Vec<u8>>>>,
}

pub struct MockCanister {
    methods: BTreeMap<String, Box<dyn Fn(&mut MockContext, Vec<u8>) -> CallResult<Vec<u8>>>>,
}

impl MockContext {
    /// Create a new mock context which could be injected for testing.
    #[inline]
    pub fn new() -> Self {
        Self {
            id: Principal::from_text("sgymv-uiaaa-aaaaa-aaaia-cai").unwrap(),
            balance: 100_000_000_000_000,
            caller: Principal::anonymous(),
            is_reply_callback_mode: false,
            trapped: false,
            cycles: 0,
            cycles_refunded: 0,
            storage: BTreeMap::new(),
            stable: Vec::new(),
            certified_data: None,
            canisters: BTreeMap::default(),
            default_handler: None,
        }
    }

    /// Set the ID of the canister.
    ///
    /// # Example
    ///
    /// ```
    /// use ic_kit::*;
    ///
    /// let id = Principal::from_text("ai7t5-aibaq-aaaaa-aaaaa-c").unwrap();
    ///
    /// MockContext::new()
    ///     .with_id(id.clone())
    ///     .inject();
    ///
    /// let ic = get_context();
    /// assert_eq!(ic.id(), id);
    /// ```
    #[inline]
    pub fn with_id(mut self, id: Principal) -> Self {
        self.id = id;
        self
    }

    /// Set the balance of the canister.
    ///
    /// # Example
    ///
    /// ```
    /// use ic_kit::*;
    ///
    /// MockContext::new()
    ///     .with_balance(1000)
    ///     .inject();
    ///
    /// let ic = get_context();
    /// assert_eq!(ic.balance(), 1000);
    /// ```
    #[inline]
    pub fn with_balance(mut self, cycles: u64) -> Self {
        self.balance = cycles;
        self
    }

    /// Set the caller for the current call.
    ///
    /// # Example
    ///
    /// ```
    /// use ic_kit::*;
    ///
    /// let alice = Principal::from_text("ai7t5-aibaq-aaaaa-aaaaa-c").unwrap();
    ///
    /// MockContext::new()
    ///     .with_caller(alice.clone())
    ///     .inject();
    ///
    /// let ic = get_context();
    /// assert_eq!(ic.caller(), alice);
    /// ```
    #[inline]
    pub fn with_caller(mut self, caller: Principal) -> Self {
        self.caller = caller;
        self
    }

    /// Make the given amount of cycles available for the call. This amount of cycles will
    /// be deduced if the call accepts them or will be refunded. If the canister accepts any
    /// cycles the balance of the canister will be increased.
    ///
    /// # Example
    ///
    /// ```
    /// use ic_kit::*;
    ///
    /// MockContext::new()
    ///     .with_msg_cycles(1000)
    ///     .inject();
    ///
    /// let ic = get_context();
    /// assert_eq!(ic.msg_cycles_available(), 1000);
    /// ic.msg_cycles_accept(300);
    /// assert_eq!(ic.msg_cycles_available(), 700);
    /// ```
    #[inline]
    pub fn with_msg_cycles(mut self, cycles: u64) -> Self {
        self.cycles = cycles;
        self
    }

    /// Initialize the context with the given value inserted in the storage.
    ///
    /// # Example
    ///
    /// ```
    /// use ic_kit::*;
    ///
    /// MockContext::new()
    ///     .with_data(String::from("Hello"))
    ///     .inject();
    ///
    /// let ic = get_context();
    /// assert_eq!(ic.get::<String>(), &"Hello".to_string());
    /// ```
    #[inline]
    pub fn with_data<T: 'static>(mut self, data: T) -> Self {
        let type_id = std::any::TypeId::of::<T>();
        self.storage.insert(type_id, Box::new(data));
        self
    }

    /// Initialize the context with the given value inserted into the stable storage.
    ///
    /// # Example
    ///
    /// ```
    /// use ic_kit::*;
    ///
    /// MockContext::new()
    ///     .with_stable(("Bella".to_string(), ))
    ///     .inject();
    ///
    /// let ic = get_context();
    /// assert_eq!(ic.stable_restore::<(String, )>(), Ok(("Bella".to_string(), )));
    /// ```
    #[inline]
    pub fn with_stable<T: Serialize>(self, data: T) -> Self
    where
        T: ArgumentEncoder,
    {
        self.stable_store(data)
            .expect("Encoding stable data failed.");
        self
    }

    /// Set the certified data of the canister.
    #[inline]
    pub fn with_certified_data(mut self, data: Vec<u8>) -> Self {
        assert!(data.len() < 32);
        self.certified_data = Some(data);
        self
    }

    /// Add the given canister with the given id to this context.
    #[inline]
    pub fn with_canister(mut self, id: Principal, canister: MockCanister) -> Self {
        self.canisters.insert(id, canister);
        self
    }

    /// Define a call handler that could be used for any canister/method that is not found in the
    /// registered canisters.
    #[inline]
    pub fn with_default_handler<
        T: for<'de> ArgumentDecoder<'de>,
        R: ArgumentEncoder,
        F: 'static + Fn(&mut MockContext, String, T) -> CallResult<R>,
    >(
        mut self,
        handler: F,
    ) -> Self {
        self.default_handler = Some(Box::new(move |ctx, method, bytes| {
            let args = decode_args(&bytes).expect("Failed to decode arguments.");
            handler(ctx, method, args).map(|r| encode_args(r).expect("Failed to encode response."))
        }));
        self
    }

    /// Creates a mock context with a default handler that accepts the given amount of cycles
    /// on every request.
    #[inline]
    pub fn with_accept_cycles_handler(mut self, cycles: u64) -> Self {
        self.use_accept_cycles_handler(cycles);
        self
    }

    /// Creates a mock context with a default handler that refunds the given amount of cycles
    /// on every request.
    #[inline]
    pub fn with_refund_cycles_handler(mut self, cycles: u64) -> Self {
        self.use_refund_cycles_handler(cycles);
        self
    }

    /// Use this context as the default context for this thread.
    #[inline]
    pub fn inject(self) -> &'static mut Self {
        inject(self);
        get_context()
    }

    /// This is how we do interior mutability for MockContext. Since the context is only accessible
    /// by only one thread, it is safe to do it here.
    #[inline]
    fn as_mut(&self) -> &mut Self {
        unsafe {
            let const_ptr = self as *const Self;
            let mut_ptr = const_ptr as *mut Self;
            &mut *mut_ptr
        }
    }
}

impl MockContext {
    /// Reset the state after a call.
    #[inline]
    pub fn call_state_reset(&mut self) {
        self.is_reply_callback_mode = false;
        self.trapped = false;
    }

    /// Clear the storage.
    #[inline]
    pub fn clear_storage(&mut self) {
        self.storage.clear()
    }

    /// Update the balance of the canister.
    #[inline]
    pub fn update_balance(&mut self, cycles: u64) {
        self.balance = cycles;
    }

    /// Update the cycles of the next message.
    #[inline]
    pub fn update_msg_cycles(&mut self, cycles: u64) {
        self.cycles = cycles;
    }

    /// Update the caller for the next message.
    #[inline]
    pub fn update_caller(&mut self, caller: Principal) {
        self.caller = caller;
    }

    /// Set the default handler to be a method that accepts the given amount of cycles on every
    /// request.
    #[inline]
    pub fn use_accept_cycles_handler(&mut self, cycles: u64) {
        self.default_handler = Some(Box::new(move |ctx, _, _| {
            ctx.msg_cycles_accept(cycles);
            Ok(encode_args(()).unwrap())
        }));
    }

    /// Set the default handler to be a method that refunds the given amount of cycles on every
    /// request.
    #[inline]
    pub fn use_refund_cycles_handler(&mut self, cycles: u64) {
        self.default_handler = Some(Box::new(move |ctx, _, _| {
            let available = ctx.msg_cycles_available();
            if available < cycles {
                panic!(
                    "Can not refund {} cycles when there is only {} cycles available.",
                    cycles, available
                );
            }
            ctx.msg_cycles_accept(available - cycles);
            Ok(encode_args(()).unwrap())
        }));
    }
}

impl MockCanister {
    /// Create a new mock canister.
    #[inline]
    pub fn new() -> Self {
        Self {
            methods: BTreeMap::default(),
        }
    }

    /// Mock the implementation of a certain method on the canister.
    #[inline]
    pub fn with_method<
        T: for<'de> ArgumentDecoder<'de>,
        R: ArgumentEncoder,
        F: 'static + Fn(&mut MockContext, T) -> CallResult<R>,
    >(
        mut self,
        name: &str,
        handler: F,
    ) -> Self {
        self.methods.insert(
            name.to_string(),
            Box::new(move |ctx, bytes| {
                let args = decode_args(&bytes).expect("Failed to decode arguments.");
                handler(ctx, args).map(|r| encode_args(r).expect("Failed to encode response."))
            }),
        );
        self
    }
}

impl Context for MockContext {
    #[inline]
    fn trap(&self, message: &str) -> ! {
        self.as_mut().trapped = true;
        panic!("Canister {} trapped with message: {}", self.id, message);
    }

    #[inline]
    fn print<S: AsRef<str>>(&self, s: S) {
        println!("{} : {}", self.id, s.as_ref())
    }

    #[inline]
    fn id(&self) -> Principal {
        self.id.clone()
    }

    #[inline]
    fn time(&self) -> u64 {
        SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .expect("Time went backwards")
            .as_nanos() as u64
    }

    #[inline]
    fn balance(&self) -> u64 {
        self.balance
    }

    #[inline]
    fn caller(&self) -> Principal {
        if self.is_reply_callback_mode {
            panic!(
                "Canister {} violated contract: \"{}\" cannot be executed in reply callback mode",
                self.id(),
                "ic0_msg_caller_size"
            )
        }

        self.caller.clone()
    }

    #[inline]
    fn msg_cycles_available(&self) -> u64 {
        self.cycles
    }

    #[inline]
    fn msg_cycles_accept(&self, cycles: u64) -> u64 {
        let mut_ref = self.as_mut();
        if cycles > mut_ref.cycles {
            let r = mut_ref.cycles;
            mut_ref.cycles = 0;
            mut_ref.balance += r;
            r
        } else {
            mut_ref.cycles -= cycles;
            mut_ref.balance += cycles;
            cycles
        }
    }

    #[inline]
    fn msg_cycles_refunded(&self) -> u64 {
        self.cycles_refunded
    }

    #[inline]
    fn store<T: 'static + Default>(&self, data: T) {
        let type_id = TypeId::of::<T>();
        self.as_mut().storage.insert(type_id, Box::new(data));
    }

    #[inline]
    fn get_mut<T: 'static + Default>(&self) -> &mut T {
        let type_id = std::any::TypeId::of::<T>();
        self.as_mut()
            .storage
            .entry(type_id)
            .or_insert_with(|| Box::new(T::default()))
            .downcast_mut()
            .expect("Unexpected value of invalid type.")
    }

    #[inline]
    fn delete<T: 'static + Default>(&self) -> bool {
        let type_id = std::any::TypeId::of::<T>();
        self.as_mut().storage.remove(&type_id).is_some()
    }

    #[inline]
    fn stable_store<T>(&self, data: T) -> Result<(), candid::Error>
    where
        T: ArgumentEncoder,
    {
        self.as_mut().stable = encode_args(data)?;
        Ok(())
    }

    #[inline]
    fn stable_restore<T>(&self) -> Result<T, String>
    where
        T: for<'de> ArgumentDecoder<'de>,
    {
        use candid::de::IDLDeserialize;
        let bytes = &self.stable;
        let mut de = IDLDeserialize::new(bytes.as_slice()).map_err(|e| format!("{:?}", e))?;
        let res = ArgumentDecoder::decode(&mut de).map_err(|e| format!("{:?}", e))?;
        // The idea here is to ignore an error that comes from Candid, because we have trailing
        // bytes.
        let _ = de.done();
        Ok(res)
    }

    fn call_raw(
        &'static self,
        id: Principal,
        method: &'static str,
        args_raw: Vec<u8>,
        cycles: u64,
    ) -> CallResponse<Vec<u8>> {
        if cycles > self.balance {
            panic!(
                "Calling canister {} with {} cycles when there is only {} cycles available.",
                id, cycles, self.balance
            );
        }

        let mut_ref = self.as_mut();

        mut_ref.balance -= cycles;

        let maybe_cb = self
            .canisters
            .get(&id)
            .map(|c| c.methods.get(method))
            .flatten();

        // Create the context for the new call.
        let mut ctx = MockContext::new()
            .with_id(id.clone())
            .with_msg_cycles(cycles)
            // Set the caller to the current canister.
            .with_caller(self.id.clone());

        mut_ref.is_reply_callback_mode = true;

        let res: CallResult<Vec<u8>> = if let Some(cb) = maybe_cb {
            cb(&mut ctx, args_raw)
        } else if let Some(cb) = &self.default_handler {
            cb(&mut ctx, method.to_string(), args_raw)
        } else {
            mut_ref.balance += cycles;
            panic!("Method {} not found on canister \"{}\"", method, id);
        };

        let refund = if res.is_err() {
            // Refund all of the cycles that were sent.
            cycles
        } else {
            // Take the cycles that are not consumed as refunded.
            ctx.cycles
        };

        mut_ref.cycles_refunded = refund;
        mut_ref.balance += refund;

        Box::pin(async move { res })
    }

    #[inline]
    fn set_certified_data(&self, data: &[u8]) {
        if data.len() > 32 {
            panic!("Data certificate has more than 32 bytes.");
        }

        self.as_mut().certified_data = Some(data.to_vec());
    }

    #[inline]
    fn data_certificate(&self) -> Option<Vec<u8>> {
        match &self.certified_data {
            Some(v) => Some(v.clone()),
            None => None,
        }
    }
}

#[cfg(test)]
mod tests {
    /// A simple canister implementation which helps the testing.
    mod canister {
        use crate::interfaces::management::WithCanisterId;
        use crate::interfaces::*;
        use crate::Context;
        use crate::{get_context, Principal};
        use std::collections::BTreeMap;

        /// An update method that returns the principal id of the caller.
        pub fn whoami() -> Principal {
            let ic = get_context();
            ic.caller()
        }

        /// An update method that returns the principal id of the canister.
        pub fn canister_id() -> Principal {
            let ic = get_context();
            ic.id()
        }

        /// An update method that returns the balance of the canister.
        pub fn balance() -> u64 {
            let ic = get_context();
            ic.balance()
        }

        /// An update method that returns the number of cycles provided by the user in the call.
        pub fn msg_cycles_available() -> u64 {
            let ic = get_context();
            ic.msg_cycles_available()
        }

        /// An update method that accepts the given number of cycles from the caller, the number of
        /// accepted cycles is returned.
        pub fn msg_cycles_accept(cycles: u64) -> u64 {
            let ic = get_context();
            ic.msg_cycles_accept(cycles)
        }

        pub type Counter = BTreeMap<u64, i64>;

        /// An update method that increments one to the given key, the new value is returned.
        pub fn increment(key: u64) -> i64 {
            let ic = get_context();
            let count = ic.get_mut::<Counter>().entry(key).or_insert(0);
            *count += 1;
            *count
        }

        /// An update method that decrement one from the given key. The new value is returned.
        pub fn decrement(key: u64) -> i64 {
            let ic = get_context();
            let count = ic.get_mut::<Counter>().entry(key).or_insert(0);
            *count -= 1;
            *count
        }

        pub async fn withdraw(canister_id: Principal, amount: u64) -> Result<(), String> {
            let ic = get_context();
            let user_balance = ic.get_mut::<u64>();

            if amount > *user_balance {
                return Err(format!("Insufficient balance."));
            }

            *user_balance -= amount;

            match management::DepositCycles::perform_with_payment(
                ic,
                Principal::management_canister(),
                (WithCanisterId { canister_id },),
                amount,
            )
            .await
            {
                Ok(()) => {
                    *user_balance += ic.msg_cycles_refunded();
                    Ok(())
                }
                Err((code, msg)) => {
                    assert_eq!(amount, ic.msg_cycles_refunded());
                    *user_balance += amount;
                    Err(format!(
                        "An error happened during the call: {}: {}",
                        code as u8, msg
                    ))
                }
            }
        }

        pub fn user_balance() -> u64 {
            let ic = get_context();
            *ic.get::<u64>()
        }

        pub fn pre_upgrade() {
            let ic = get_context();
            let map = ic.get::<Counter>();
            ic.stable_store((map,))
                .expect("Failed to write to stable storage");
        }

        pub fn post_upgrade() {
            let ic = get_context();
            if let Ok((map,)) = ic.stable_restore() {
                ic.store::<Counter>(map);
            }
        }
    }

    /// Some mock principal ids.
    mod users {
        use crate::Principal;

        pub fn bob() -> Principal {
            Principal::from_text("ai7t5-aibaq-aaaaa-aaaaa-c").unwrap()
        }

        pub fn john() -> Principal {
            Principal::from_text("hozae-racaq-aaaaa-aaaaa-c").unwrap()
        }
    }

    use crate::Principal;
    use crate::{Context, MockContext};

    #[test]
    fn test_with_id() {
        MockContext::new()
            .with_id(Principal::management_canister())
            .inject();

        assert_eq!(canister::canister_id(), Principal::management_canister());
    }

    #[test]
    fn test_balance() {
        let ctx = MockContext::new().with_balance(1000).inject();

        assert_eq!(canister::balance(), 1000);

        ctx.update_balance(2000);
        assert_eq!(canister::balance(), 2000);
    }

    #[test]
    fn test_caller() {
        let ctx = MockContext::new().with_caller(users::john()).inject();

        assert_eq!(canister::whoami(), users::john());

        ctx.update_caller(users::bob());
        assert_eq!(canister::whoami(), users::bob());
    }

    #[test]
    fn test_msg_cycles() {
        let ctx = MockContext::new().with_msg_cycles(1000).inject();

        assert_eq!(canister::msg_cycles_available(), 1000);

        ctx.update_msg_cycles(50);
        assert_eq!(canister::msg_cycles_available(), 50);
    }

    #[test]
    fn test_msg_cycles_accept() {
        let ctx = MockContext::new()
            .with_msg_cycles(1000)
            .with_balance(240)
            .inject();

        assert_eq!(canister::msg_cycles_accept(100), 100);
        assert_eq!(ctx.msg_cycles_available(), 900);
        assert_eq!(ctx.balance(), 340);

        ctx.update_msg_cycles(50);
        assert_eq!(canister::msg_cycles_accept(100), 50);
        assert_eq!(ctx.msg_cycles_available(), 0);
        assert_eq!(ctx.balance(), 390);
    }

    #[test]
    fn test_storage_simple() {
        MockContext::new().inject();
        assert_eq!(canister::increment(0), 1);
        assert_eq!(canister::increment(0), 2);
        assert_eq!(canister::increment(0), 3);
        assert_eq!(canister::increment(1), 1);
        assert_eq!(canister::decrement(0), 2);
        assert_eq!(canister::decrement(2), -1);
    }

    #[test]
    fn test_storage() {
        let ctx = MockContext::new()
            .with_data({
                let mut map = canister::Counter::default();
                map.insert(0, 12);
                map.insert(1, 17);
                map
            })
            .inject();
        assert_eq!(canister::increment(0), 13);
        assert_eq!(canister::decrement(1), 16);

        ctx.store({
            let mut map = canister::Counter::default();
            map.insert(0, 12);
            map.insert(1, 17);
            map
        });

        assert_eq!(canister::increment(0), 13);
        assert_eq!(canister::decrement(1), 16);

        ctx.clear_storage();

        assert_eq!(canister::increment(0), 1);
        assert_eq!(canister::decrement(1), -1);
    }

    #[test]
    fn stable_storage() {
        let ctx = MockContext::new()
            .with_data({
                let mut map = canister::Counter::default();
                map.insert(0, 2);
                map.insert(1, 27);
                map.insert(2, 5);
                map.insert(3, 17);
                map
            })
            .inject();

        canister::pre_upgrade();
        ctx.clear_storage();
        canister::post_upgrade();

        let counter = ctx.get::<canister::Counter>();
        let data: Vec<(u64, i64)> = counter
            .iter()
            .map(|(k, v)| (k.clone(), v.clone()))
            .collect();
        assert_eq!(data, vec![(0, 2), (1, 27), (2, 5), (3, 17)]);

        assert_eq!(canister::increment(0), 3);
        assert_eq!(canister::decrement(1), 26);
    }

    #[async_std::test]
    async fn withdraw_accept() {
        MockContext::new()
            .with_accept_cycles_handler(200)
            .with_data(1000u64)
            .with_balance(2000)
            .inject();

        assert_eq!(canister::user_balance(), 1000);

        canister::withdraw(users::bob(), 100).await.unwrap();

        // The user balance needs to be decremented.
        assert_eq!(canister::user_balance(), 900);
        // The canister balance needs to be decremented.
        assert_eq!(canister::balance(), 1900);
    }

    #[async_std::test]
    async fn withdraw_accept_portion() {
        MockContext::new()
            .with_accept_cycles_handler(50)
            .with_data(1000u64)
            .with_balance(2000)
            .inject();

        assert_eq!(canister::user_balance(), 1000);

        canister::withdraw(users::bob(), 100).await.unwrap();

        // The user balance needs to be decremented.
        assert_eq!(canister::user_balance(), 950);
        // The canister balance needs to be decremented.
        assert_eq!(canister::balance(), 1950);
    }

    #[async_std::test]
    async fn withdraw_accept_zero() {
        MockContext::new()
            .with_accept_cycles_handler(0)
            .with_data(1000u64)
            .with_balance(2000)
            .inject();

        assert_eq!(canister::user_balance(), 1000);

        canister::withdraw(users::bob(), 100).await.unwrap();

        // The balance should not be decremented.
        assert_eq!(canister::user_balance(), 1000);
        assert_eq!(canister::balance(), 2000);
    }

    #[async_std::test]
    async fn with_refund() {
        MockContext::new()
            .with_refund_cycles_handler(30)
            .with_data(1000u64)
            .with_balance(2000)
            .inject();

        canister::withdraw(users::bob(), 100).await.unwrap();
        assert_eq!(canister::user_balance(), 930);
        assert_eq!(canister::balance(), 1930);
    }
}
