//! This module defines some common facilities used across WaveJudge.
//!

use std::fmt::{Display, Formatter};
use std::hash::Hash;
use std::str::FromStr;
use std::string::ToString;

use serde::{Serialize, Serializer, Deserialize, Deserializer};
use serde::de::{Visitor, Unexpected};

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

impl Serialize for ObjectId {
    fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
        where S: Serializer {
        serializer.serialize_str(&self.to_string())
    }
}

impl<'de> Deserialize<'de> for ObjectId {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
        where D: Deserializer<'de> {
        deserializer.deserialize_str(ObjectIdDeserializeVisitor)
    }
}

struct ObjectIdDeserializeVisitor;

impl<'de> Visitor<'de> for ObjectIdDeserializeVisitor {
    type Value = ObjectId;

    fn expecting(&self, formatter: &mut Formatter) -> std::fmt::Result {
        formatter.write_str("a 24-character string consisting of hexadecimal digits")
    }

    fn visit_str<E>(self, v: &str) -> Result<Self::Value, E>
        where E: serde::de::Error {
        match ObjectId::from_str(v) {
            Ok(id) => Ok(id),
            Err(..) => Err(E::invalid_value(Unexpected::Str(v), &self))
        }
    }

    fn visit_borrowed_str<E>(self, v: &'de str) -> Result<Self::Value, E>
        where E: serde::de::Error {
        match ObjectId::from_str(v) {
            Ok(id) => Ok(id),
            Err(..) => Err(E::invalid_value(Unexpected::Str(v), &self))
        }
    }

    fn visit_string<E>(self, v: String) -> Result<Self::Value, E>
        where E: serde::de::Error {
        self.visit_str(&v)
    }
}

/// Represent a language triple.
#[derive(Debug, Clone, Eq, PartialEq, Hash, Serialize, Deserialize)]
pub struct LanguageTriple {
    /// The identifier of the language.
    #[serde(rename = "identifier")]
    pub identifier: String,

    /// The dialect of the language.
    #[serde(rename = "dialect")]
    pub dialect: String,

    /// The version of the language.
    #[serde(rename = "version")]
    pub version: String,
}

impl LanguageTriple {
    /// Create a new `LanguageTriple` value.
    pub fn new<T1, T2, T3>(identifier: T1, dialect: T2, version: T3) -> Self
        where T1: Into<String>, T2: Into<String>, T3: Into<String> {
        LanguageTriple {
            identifier: identifier.into(),
            dialect: dialect.into(),
            version: version.into(),
        }
    }
}

impl Into<judge::languages::LanguageIdentifier> for LanguageTriple {
    fn into(self) -> judge::languages::LanguageIdentifier {
        use judge::languages::{LanguageIdentifier, LanguageBranch};
        LanguageIdentifier::new(self.identifier, LanguageBranch::new(self.dialect, self.version))
    }
}

impl From<judge::languages::LanguageIdentifier> for LanguageTriple {
    fn from(identifier: judge::languages::LanguageIdentifier) -> Self {
        LanguageTriple {
            identifier: identifier.language().to_owned(),
            dialect: identifier.dialect().to_owned(),
            version: identifier.version().to_owned(),
        }
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

        #[test]
        fn serialize() {
            let example = ObjectId {
                data: [ 0x01, 0x23, 0x45, 0x67, 0x89, 0xab, 0xcd, 0xef, 0x01, 0x23, 0x45, 0x67 ]
            };
            assert_eq!("\"0123456789abcdef01234567\"", serde_json::to_string(&example).unwrap());
        }

        #[test]
        fn deserialize() {
            let example = ObjectId {
                data: [ 0x01, 0x23, 0x45, 0x67, 0x89, 0xab, 0xcd, 0xef, 0x01, 0x23, 0x45, 0x67 ]
            };
            assert_eq!(example,
                serde_json::from_str::<ObjectId>("\"0123456789abcdef01234567\"").unwrap());
        }
    }
}
