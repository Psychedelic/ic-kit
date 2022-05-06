use crate::ic::StableReader;
use std::marker::PhantomData;

/// A pointer to a region of the stable storage.
pub struct Pointer<T>(u32, PhantomData<T>);

impl<T> Pointer<T> {
    pub fn new(offset: u32) -> Self {
        Pointer(offset, PhantomData::default())
    }

    /// Read and decode the content of the stable storage at the given offset.
    pub fn read(&self) -> bincode::Result<T>
    where
        T: serde::de::DeserializeOwned,
    {
        let reader = StableReader::new(self.0 as usize);
        bincode::deserialize_from(reader)
    }
}

#[cfg(test)]
mod tests {
    use crate::db::pointer::Pointer;
    use crate::ic::StableWriter;
    use crate::MockContext;
    use serde::{Deserialize, Serialize};

    #[derive(Serialize, Deserialize, Debug, PartialOrd, PartialEq)]
    struct Data {
        x: u32,
        y: u32,
        str: String,
    }

    #[test]
    fn test_pointer() {
        let d1 = Data {
            x: 17,
            y: 27,
            str: "Hello".into(),
        };

        let d2 = Data {
            x: 23,
            y: 14,
            str: "".into(),
        };

        let d3 = Data {
            x: 19,
            y: 15,
            str: "Some longer text".into(),
        };

        MockContext::new().inject();

        let mut writer = StableWriter::default();

        let d1_pointer = Pointer::<Data>::new(writer.offset() as u32);
        bincode::serialize_into(&mut writer, &d1).expect("Failed to write d1");

        let d2_pointer = Pointer::<Data>::new(writer.offset() as u32);
        bincode::serialize_into(&mut writer, &d2).expect("Failed to write d2");

        let d3_pointer = Pointer::<Data>::new(writer.offset() as u32);
        bincode::serialize_into(&mut writer, &d3).expect("Failed to write d3");

        let d1_actual = d1_pointer.read().expect("Failed to read d1");
        let d2_actual = d2_pointer.read().expect("Failed to read d2");
        let d3_actual = d3_pointer.read().expect("Failed to read d3");

        assert_eq!(d1, d1_actual);
        assert_eq!(d2, d2_actual);
        assert_eq!(d3, d3_actual);
    }
}
