//! A set of mock principal ids.

use ic_types::Principal;
use lazy_static::lazy_static;

lazy_static! {
    pub static ref ALICE: Principal = Principal::self_authenticating("ALICE");
    pub static ref BOB: Principal = Principal::self_authenticating("BOB");
    pub static ref JOHN: Principal = Principal::self_authenticating("JOHN");
    pub static ref PARSA: Principal = Principal::self_authenticating("PARSA");
    pub static ref OZ: Principal = Principal::self_authenticating("OZ");
}
