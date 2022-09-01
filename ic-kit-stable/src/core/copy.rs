/// A marker for any type that its content can just be copied to stable storage as is.
pub trait StableCopy {}

impl<T: Copy> StableCopy for T {}
