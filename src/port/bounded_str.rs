//! `BoundedStr<N>`: fixed-capacity UTF-8 string, `Copy`, stack-only.
//!
//! Mirrors a SNOBOL4 fixed-width string slot. Constructor validates
//! capacity; `as_str` returns the live prefix.

use super::error::PortError;

#[derive(Clone, Copy, Eq)]
pub struct BoundedStr<const N: usize> {
    buf: [u8; N],
    len: u8,
}

impl<const N: usize> BoundedStr<N> {
    pub const fn new() -> Self {
        Self { buf: [0u8; N], len: 0 }
    }

    pub fn from_str(s: &str) -> Result<Self, PortError> {
        let bytes = s.as_bytes();
        if bytes.len() > N || bytes.len() > u8::MAX as usize {
            return Err(PortError::Overflow);
        }
        let mut buf = [0u8; N];
        let mut i = 0;
        while i < bytes.len() {
            buf[i] = bytes[i];
            i += 1;
        }
        Ok(Self { buf, len: bytes.len() as u8 })
    }

    pub fn as_str(&self) -> &str {
        let n = self.len as usize;
        std::str::from_utf8(&self.buf[..n]).expect("bounded_str: utf8 invariant")
    }

    pub fn len(&self) -> usize {
        self.len as usize
    }

    pub fn is_empty(&self) -> bool {
        self.len == 0
    }
}

impl<const N: usize> Default for BoundedStr<N> {
    fn default() -> Self {
        Self::new()
    }
}

impl<const N: usize> PartialEq for BoundedStr<N> {
    fn eq(&self, other: &Self) -> bool {
        self.as_str() == other.as_str()
    }
}

impl<const N: usize> core::fmt::Debug for BoundedStr<N> {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        write!(f, "BoundedStr({:?})", self.as_str())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn roundtrip_basic() {
        let s = BoundedStr::<16>::from_str("hello").unwrap();
        assert_eq!(s.as_str(), "hello");
        assert_eq!(s.len(), 5);
        assert!(!s.is_empty());
    }

    #[test]
    fn empty_defaults() {
        let s: BoundedStr<8> = BoundedStr::new();
        assert!(s.is_empty());
        assert_eq!(s.as_str(), "");
        assert_eq!(s.len(), 0);
        assert_eq!(s, BoundedStr::<8>::default());
    }

    #[test]
    fn overflow_returns_err() {
        let r = BoundedStr::<4>::from_str("hello");
        assert_eq!(r, Err(PortError::Overflow));
    }

    #[test]
    fn exact_fit_at_capacity() {
        let s = BoundedStr::<5>::from_str("hello").unwrap();
        assert_eq!(s.as_str(), "hello");
        assert_eq!(s.len(), 5);
    }

    #[test]
    fn equality_by_content() {
        let a = BoundedStr::<16>::from_str("x").unwrap();
        let b = BoundedStr::<16>::from_str("x").unwrap();
        let c = BoundedStr::<16>::from_str("y").unwrap();
        assert_eq!(a, b);
        assert_ne!(a, c);
    }

    #[test]
    fn value_is_copy() {
        let s = BoundedStr::<4>::from_str("ab").unwrap();
        let t = s;
        let u = s;
        assert_eq!(t.as_str(), "ab");
        assert_eq!(u.as_str(), "ab");
    }
}
