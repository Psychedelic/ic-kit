use crate::Principal;
use ic_kit_sys::types::CallError;
use std::future::Future;

// TODO(qti3e) Move this to management module.
#[derive(Debug, Clone, PartialOrd, PartialEq, CandidType)]
pub enum InstallMode {
    #[serde(rename = "install")]
    Install,
    #[serde(rename = "reinstall")]
    Reinstall,
    #[serde(rename = "upgrade")]
    Upgrade,
}

#[derive(Debug, Clone, PartialOrd, PartialEq, CandidType)]
pub struct InstallCodeArgument {
    pub mode: InstallMode,
    pub canister_id: Principal,
    #[serde(with = "serde_bytes")]
    pub wasm_module: &'static [u8],
    pub arg: Vec<u8>,
}

/// A canister.
pub trait KitCanister {
    /// Create a new instance of this canister using the provided canister id.
    #[cfg(not(target_family = "wasm"))]
    fn build(canister_id: candid::Principal) -> ic_kit_runtime::Canister;

    /// Create a new instance of this canister with the anonymous principal id.
    #[cfg(not(target_family = "wasm"))]
    fn anonymous() -> ic_kit_runtime::Canister {
        Self::build(candid::Principal::anonymous())
    }

    /// The candid description of the canister.
    fn candid() -> String;
}

/// A dynamic canister is a canister that can be dynamically created and installed.
pub trait KitDynamicCanister: KitCanister {
    /// Should return the wasm binary of the canister.
    fn get_canister_wasm() -> &'static [u8];

    #[cfg(not(target_family = "wasm"))]
    fn install_code(
        canister_id: Principal,
        mode: InstallMode,
    ) -> Box<dyn Future<Output = Result<(), CallError>>> {
    }
}
