// TODO(qti3e Implement management interface and types.

use ic_kit::prelude::*;

#[derive(Deserialize, Debug, Clone, PartialOrd, PartialEq, CandidType)]
pub struct CreateCanisterArgument {
    pub settings: Option<CanisterSettings>,
}

#[derive(Deserialize, Debug, Clone, PartialOrd, PartialEq, CandidType)]
pub struct CanisterSettings {
    pub controllers: Option<Vec<Principal>>,
    pub compute_allocation: Option<Nat>,
    pub memory_allocation: Option<Nat>,
    pub freezing_threshold: Option<Nat>,
}
