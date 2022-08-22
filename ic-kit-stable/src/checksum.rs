/// A u40 value that uses the extra padding of u64 for a checksum.
#[repr(packed)]
pub struct CheckedU40(u64);

impl CheckedU40 {
    /// Create a new checked u40 value.
    ///
    /// # Panics
    ///
    /// If the provided value is larger than 40bits, or if it's zero.
    pub fn new(value: u64) -> Self {
        if value & 0xffffff0000000000 > 0 {
            panic!("only 40bit integers are supported.");
        }

        if value == 0 {
            panic!("zero for checksum is not supported.")
        }

        let a = (value & 0xff00000000) >> 32;
        let b = (value & 0x00ff000000) >> 24;
        let c = (value & 0x0000ff0000) >> 16;
        let d = (value & 0x000000ff00) >> 8;
        let e = value & 0x00000000ff;
        let x = a ^ b ^ c;
        let y = c ^ d ^ e;
        let z = x ^ y;
        let s = (x << 16) | (y << 8) | z;
        let r = (s << 40) | value;

        CheckedU40(r)
    }

    /// Verify and unpack the wrapped value.
    pub fn verify(&self) -> Option<u64> {
        let value = self.0;

        // we don't support zero.
        if value == 0 {
            return None;
        }

        let x = (value & 0xff00000000000000) >> 56;
        let y = (value & 0x00ff000000000000) >> 48;
        let z = (value & 0x0000ff0000000000) >> 40;
        let a = (value & 0xff00000000) >> 32;
        let b = (value & 0x00ff000000) >> 24;
        let c = (value & 0x0000ff0000) >> 16;
        let d = (value & 0x000000ff00) >> 8;
        let e = value & 0x00000000ff;

        let xx = a ^ b ^ c;
        let yy = c ^ d ^ e;
        let zz = x ^ y;

        if xx == x && yy == y && zz == z {
            Some(value & 0x000000ffffffffff)
        } else {
            None
        }
    }

    /// Return the protected number without any checks.
    pub fn unchecked(&self) -> u64 {
        self.0 & 0x000000ffffffffff
    }
}

#[cfg(test)]
mod tests {
    use crate::checksum::CheckedU40;

    #[test]
    fn test_verify() {
        assert_eq!(CheckedU40::new(0x34653443ab).verify(), Some(0x34653443ab));
        assert_eq!(CheckedU40(0x34653443ab).verify(), None);

        for i in 0..1_000 {
            let n = ((0x121212 * i) << 40) | (0x1010101010 * (i + 0x12345));
            assert_eq!(CheckedU40(n).verify(), None);

            let n = n & 0x000000ffffffffff;
            assert_eq!(CheckedU40::new(n).verify(), Some(n));
        }

        assert_eq!(
            CheckedU40::new(0x000000ffffffffff).verify(),
            Some(0x000000ffffffffff)
        );
    }

    #[test]
    #[should_panic]
    fn test_zero() {
        CheckedU40::new(0);
    }

    #[test]
    fn test_unchecked() {
        for i in 0..1_000 {
            let n = ((0x121212 * i) << 40) | (0x1010101010 * (i + 0x12345));
            let n = n & 0x000000ffffffffff;
            assert_eq!(CheckedU40::new(n).unchecked(), n);
        }
    }

    #[test]
    #[should_panic]
    fn test_non_40bit() {
        CheckedU40::new(0x000000ffffffffff + 1);
    }

    #[test]
    fn test_invalid() {
        assert_eq!(CheckedU40(0xffffffffffffffff).verify(), None);
        assert_eq!(CheckedU40(0).verify(), None);
    }
}
