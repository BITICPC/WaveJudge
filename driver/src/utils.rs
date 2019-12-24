//! This module provide some utility functions.
//!

use std::cmp::Ordering;

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
