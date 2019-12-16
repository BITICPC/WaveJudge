//! This module provide some utility functions.
//!

use std::cmp::Ordering;
use std::sync::atomic::{AtomicBool, Ordering as AtomicOrdering};

/// Retrieves the greater one among the given two objects. If the two objects are considered equal,
/// then `lhs` will be returned.
pub fn max<'a, T>(lhs: &'a T, rhs: &'a T) -> &'a T
    where T: ?Sized + Ord {
    match lhs.cmp(rhs) {
        Ordering::Equal | Ordering::Greater => lhs,
        Ordering::Less => rhs
    }
}

/// Perform an unchecked bitcast from input type `I` to output type `O`. This function panics if the
/// sizes of `I` and `O` are not the same.
pub fn bitcast<I, O>(input: I) -> O
    where I: Copy, O: Copy {
    if std::mem::size_of::<I>() != std::mem::size_of::<O>() {
        panic!("Sizes of I and O are not the same.");
    }

    unsafe { *((&input as *const I) as *const O) }
}

/// Provide a `Once` value similar to `std::sync::Once` but additoinally allows return value from
/// the user provided function.
pub struct Once {
    /// An atomic boolean value indicating whether the `call_once` function has been called on this
    /// value.
    state: AtomicBool,
}

impl Once {
    /// Create a new `Once` value.
    pub const fn new() -> Self {
        Once {
            state: AtomicBool::new(false)
        }
    }

    /// Call the given closure if the `call_once` function has not been called already. This
    /// function returns `Some(v)` where `v` is the return value of `closure` if the given closure
    /// is executed or `None` if the given closure is not executed.
    pub fn call_once<F, R>(&self, closure: F) -> Option<R>
        where F: FnOnce() -> R {
        if !self.state.swap(true, AtomicOrdering::SeqCst) {
            Some(closure())
        } else {
            None
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn max_lhs() {
        let lhs = 5;
        let rhs = 3;
        assert_eq!((&lhs) as *const _, max(&lhs, &rhs) as *const _);
    }

    #[test]
    fn max_rhs() {
        let lhs = 3;
        let rhs = 5;
        assert_eq!((&rhs) as *const _, max(&lhs, &rhs) as *const _);
    }

    #[test]
    fn max_eq() {
        let lhs = 3;
        let rhs = 3;
        assert_eq!((&lhs) as *const _, max(&lhs, &rhs) as *const _);
    }

    #[test]
    #[should_panic]
    fn bitcast_different_size() {
        bitcast::<i32, u64>(10);
    }

    #[test]
    fn bitcast_ok() {
        assert_eq!(std::u64::MAX, bitcast::<i64, u64>(-1));
    }
}
