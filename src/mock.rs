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
            cycles: 0,
            cycles_refunded: 0,
            storage: BTreeMap::new(),
            stable: Vec::new(),
            certified_data: None,
            canisters: BTreeMap::default(),
            default_handler: None,
        }
    }

    /// Set the ID of the current canister.
    #[inline]
    pub fn with_id(mut self, id: Principal) -> Self {
        self.id = id;
        self
    }

    /// Set the balance of the current canister.
    #[inline]
    pub fn with_balance(mut self, cycles: u64) -> Self {
        self.balance = cycles;
        self
    }

    /// Set the caller for the current call.
    #[inline]
    pub fn with_caller(mut self, caller: Principal) -> Self {
        self.caller = caller;
        self
    }

    /// Make the given amount of cycles available for the call.
    #[inline]
    pub fn with_cycles(mut self, cycles: u64) -> Self {
        self.cycles = cycles;
        self
    }

    /// Store the given version of the data in storage.
    #[inline]
    pub fn with_data<T: 'static>(mut self, data: T) -> Self {
        let type_id = std::any::TypeId::of::<T>();
        self.storage.insert(type_id, Box::new(data));
        self
    }

    /// Store the given data in the stable storage in this context.
    #[inline]
    pub fn with_stable<T: Serialize>(mut self, data: T) -> Self
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

    /// Use this context as the default context for this thread.
    #[inline]
    pub fn inject(self) -> &'static mut Self {
        inject(self);
        get_context()
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
    fn msg_cycles_accept(&mut self, cycles: u64) -> u64 {
        if cycles > self.cycles {
            let r = self.cycles;
            self.cycles = 0;
            self.balance += r;
            r
        } else {
            self.cycles -= cycles;
            self.balance += cycles;
            cycles
        }
    }

    #[inline]
    fn msg_cycles_refunded(&self) -> u64 {
        self.cycles_refunded
    }

    #[inline]
    fn get_mut<T: 'static + Default>(&mut self) -> &mut T {
        let type_id = std::any::TypeId::of::<T>();
        self.storage
            .entry(type_id)
            .or_insert_with(|| Box::new(T::default()))
            .downcast_mut()
            .expect("Unexpected value of invalid type.")
    }

    #[inline]
    fn delete<T: 'static + Default>(&mut self) -> bool {
        let type_id = std::any::TypeId::of::<T>();
        self.storage.remove(&type_id).is_some()
    }

    #[inline]
    fn stable_store<T>(&mut self, data: T) -> Result<(), candid::Error>
    where
        T: ArgumentEncoder,
    {
        self.stable = encode_args(data)?;
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
        &'static mut self,
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

        self.balance -= cycles;

        let maybe_cb = self
            .canisters
            .get(&id)
            .map(|c| c.methods.get(method))
            .flatten();

        // Create the context for the new call.
        let mut ctx = MockContext::new()
            .with_id(id.clone())
            .with_cycles(cycles)
            // Set the caller to the current canister.
            .with_caller(self.id.clone());

        self.is_reply_callback_mode = true;

        let res = if let Some(cb) = maybe_cb {
            cb(&mut ctx, args_raw)
        } else if let Some(cb) = &self.default_handler {
            cb(&mut ctx, method.to_string(), args_raw)
        } else {
            self.balance += cycles;
            panic!("Method {} not found on canister \"{}\"", method, id);
        };

        // Take the cycles that are not consumed as refunded.
        self.cycles_refunded = ctx.cycles;
        self.balance += ctx.cycles;

        Box::pin(async move { res })
    }

    #[inline]
    fn set_certified_data(&mut self, data: &[u8]) {
        if data.len() > 32 {
            panic!("Data certificate has more than 32 bytes.");
        }

        self.certified_data = Some(data.to_vec())
    }

    #[inline]
    fn data_certificate(&self) -> Option<Vec<u8>> {
        match &self.certified_data {
            Some(v) => Some(v.clone()),
            None => None,
        }
    }
}
