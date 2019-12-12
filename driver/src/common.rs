//! This module defines some common facilities used across WaveJudge.
//!

use std::fmt::{Display, Formatter};
use std::hash::Hash;
use std::str::FromStr;

/// Represent a 12-byte identifier used by BSON and MongoDB.
#[derive(Clone, Copy, Default, Debug, PartialEq, Eq, Hash)]
pub struct ObjectId {
    /// Raw data of object IDs.
    data: [u8; 12]
}

impl FromStr for ObjectId {
    type Err = ();

    fn from_str(s: &str) -> std::result::Result<Self, Self::Err> {
        if s.len() != 24 {
            return Err(());
        }

        let mut id = ObjectId { data: [0u8; 12] };
        for i in (0..12usize).map(|x| x * 2) {
            id.data[i / 2] = u8::from_str_radix(&s[i..i + 2], 16)
                .map_err(|_| ())
                ?;
        }

        Ok(id)
    }
}

impl Display for ObjectId {
    fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
        for d in &self.data {
            f.write_fmt(format_args!("{:02x}", *d))?;
        }

        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    mod object_id {
        use super::*;

        #[test]
        fn from_str_invalid() {
            assert!(ObjectId::from_str("abca").is_err());
            assert!(ObjectId::from_str("17325193026584935r292324").is_err());
        }

        #[test]
        fn from_str_ok() {
            let example = ObjectId {
                data: [ 0x01, 0x23, 0x45, 0x67, 0x89, 0xab, 0xcd, 0xef, 0x01, 0x23, 0x45, 0x67 ]
            };
            assert_eq!(example, ObjectId::from_str("0123456789aBcDeF01234567").unwrap());
        }

        #[test]
        fn format() {
            let example = ObjectId {
                data: [ 0x01, 0x23, 0x45, 0x67, 0x89, 0xab, 0xcd, 0xef, 0x01, 0x23, 0x45, 0x67 ]
            };
            assert_eq!("0123456789abcdef01234567", format!("{}", example));
        }
    }
}
