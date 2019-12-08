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
}
