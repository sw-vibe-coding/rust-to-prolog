//! `BoundedArr<T, N>`: stack-allocated fixed-capacity array.
//!
//! Mirrors a SNOBOL4 ARRAY with explicit length discipline. `push`
//! fails with `PortError::Overflow` at capacity — no reallocation.

use super::error::PortError;

#[derive(Clone, Copy)]
pub struct BoundedArr<T: Copy, const N: usize> {
    data: [Option<T>; N],
    len: usize,
}

impl<T: Copy, const N: usize> BoundedArr<T, N> {
    pub const fn new() -> Self {
        Self { data: [None; N], len: 0 }
    }

    pub fn push(&mut self, v: T) -> Result<(), PortError> {
        if self.len >= N {
            return Err(PortError::Overflow);
        }
        self.data[self.len] = Some(v);
        self.len += 1;
        Ok(())
    }

    pub fn get(&self, i: usize) -> Option<&T> {
        if i >= self.len {
            return None;
        }
        self.data[i].as_ref()
    }

    pub fn get_mut(&mut self, i: usize) -> Option<&mut T> {
        if i >= self.len {
            return None;
        }
        self.data[i].as_mut()
    }

    pub fn len(&self) -> usize {
        self.len
    }

    pub fn is_empty(&self) -> bool {
        self.len == 0
    }

    pub fn capacity(&self) -> usize {
        N
    }

    pub fn iter(&self) -> impl Iterator<Item = &T> + '_ {
        self.data[..self.len].iter().filter_map(|x| x.as_ref())
    }
}

impl<T: Copy, const N: usize> Default for BoundedArr<T, N> {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn push_then_get() {
        let mut a: BoundedArr<i32, 4> = BoundedArr::new();
        a.push(10).unwrap();
        a.push(20).unwrap();
        assert_eq!(a.get(0), Some(&10));
        assert_eq!(a.get(1), Some(&20));
        assert_eq!(a.get(2), None);
        assert_eq!(a.len(), 2);
        assert_eq!(a.capacity(), 4);
    }

    #[test]
    fn overflow_at_capacity() {
        let mut a: BoundedArr<u8, 2> = BoundedArr::new();
        a.push(1).unwrap();
        a.push(2).unwrap();
        assert_eq!(a.push(3), Err(PortError::Overflow));
        assert_eq!(a.len(), 2);
    }

    #[test]
    fn empty_array() {
        let a: BoundedArr<i32, 4> = BoundedArr::new();
        assert!(a.is_empty());
        assert_eq!(a.len(), 0);
        assert_eq!(a.get(0), None);
        assert_eq!(a.iter().count(), 0);
    }

    #[test]
    fn get_mut_updates_in_place() {
        let mut a: BoundedArr<i32, 4> = BoundedArr::new();
        a.push(5).unwrap();
        *a.get_mut(0).unwrap() = 7;
        assert_eq!(a.get(0), Some(&7));
        assert!(a.get_mut(1).is_none());
    }

    #[test]
    fn iter_visits_live_slots_only() {
        let mut a: BoundedArr<i32, 4> = BoundedArr::new();
        a.push(1).unwrap();
        a.push(2).unwrap();
        a.push(3).unwrap();
        let sum: i32 = a.iter().sum();
        assert_eq!(sum, 6);
        assert_eq!(a.iter().count(), 3);
    }
}
