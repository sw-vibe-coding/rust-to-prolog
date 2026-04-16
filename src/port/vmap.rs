//! `Vmap<N>`: linear-scan string-keyed integer map, capacity N.
//!
//! Mirrors SNOBOL4 `VMAP = ' key:val key:val '` — a linear lookup by
//! design. Keys are `BoundedStr<16>`; values are `i32`. No hashing.

use super::bounded_arr::BoundedArr;
use super::bounded_str::BoundedStr;
use super::error::PortError;

pub type VKey = BoundedStr<16>;

#[derive(Clone, Copy)]
pub struct Vmap<const N: usize> {
    ents: BoundedArr<(VKey, i32), N>,
}

impl<const N: usize> Vmap<N> {
    pub const fn new() -> Self {
        Self { ents: BoundedArr::new() }
    }

    pub fn insert(&mut self, k: &str, v: i32) -> Result<(), PortError> {
        let key = VKey::from_str(k)?;
        for i in 0..self.ents.len() {
            let entry = self.ents.get_mut(i).expect("vmap: len invariant");
            if entry.0 == key {
                entry.1 = v;
                return Ok(());
            }
        }
        self.ents.push((key, v))
    }

    pub fn get(&self, k: &str) -> Option<i32> {
        let key = VKey::from_str(k).ok()?;
        for entry in self.ents.iter() {
            if entry.0 == key {
                return Some(entry.1);
            }
        }
        None
    }

    pub fn len(&self) -> usize {
        self.ents.len()
    }

    pub fn is_empty(&self) -> bool {
        self.ents.is_empty()
    }

    pub fn iter(&self) -> impl Iterator<Item = (&VKey, i32)> + '_ {
        self.ents.iter().map(|(k, v)| (k, *v))
    }
}

impl<const N: usize> Default for Vmap<N> {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn insert_then_get() {
        let mut m: Vmap<8> = Vmap::new();
        m.insert("a", 1).unwrap();
        m.insert("b", 2).unwrap();
        assert_eq!(m.get("a"), Some(1));
        assert_eq!(m.get("b"), Some(2));
        assert_eq!(m.get("c"), None);
        assert_eq!(m.len(), 2);
    }

    #[test]
    fn insert_overwrites_existing_key() {
        let mut m: Vmap<4> = Vmap::new();
        m.insert("x", 1).unwrap();
        m.insert("x", 99).unwrap();
        assert_eq!(m.get("x"), Some(99));
        assert_eq!(m.len(), 1);
    }

    #[test]
    fn overflow_at_capacity() {
        let mut m: Vmap<2> = Vmap::new();
        m.insert("a", 1).unwrap();
        m.insert("b", 2).unwrap();
        assert_eq!(m.insert("c", 3), Err(PortError::Overflow));
    }

    #[test]
    fn key_too_long_overflows() {
        let mut m: Vmap<4> = Vmap::new();
        let long = "0123456789abcdefg";
        assert_eq!(long.len(), 17);
        assert_eq!(m.insert(long, 1), Err(PortError::Overflow));
    }

    #[test]
    fn empty_map() {
        let m: Vmap<4> = Vmap::new();
        assert!(m.is_empty());
        assert_eq!(m.get("anything"), None);
        assert_eq!(m.iter().count(), 0);
    }

    #[test]
    fn iter_visits_all_entries() {
        let mut m: Vmap<4> = Vmap::new();
        m.insert("a", 10).unwrap();
        m.insert("b", 20).unwrap();
        let total: i32 = m.iter().map(|(_, v)| v).sum();
        assert_eq!(total, 30);
    }
}
