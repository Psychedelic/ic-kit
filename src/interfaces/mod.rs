use crate::candid::utils::{ArgumentDecoder, ArgumentEncoder};
use crate::{CallResponse, Context, Principal};

pub mod management;

/// A method description.
pub trait Method {
    const NAME: &'static str;
    type Arguments: ArgumentEncoder;
    type Response: for<'de> ArgumentDecoder<'de>;

    #[inline]
    fn perform<T: Context>(
        ctx: &'static T,
        id: Principal,
        args: Self::Arguments,
    ) -> CallResponse<Self::Response> {
        ctx.call(id, Self::NAME, args)
    }

    #[inline]
    fn perform_with_payment<T: Context>(
        ctx: &'static T,
        id: Principal,
        args: Self::Arguments,
        cycles: u64,
    ) -> CallResponse<Self::Response> {
        ctx.call_with_payment(id, Self::NAME, args, cycles)
    }
}
