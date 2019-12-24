//! This module defines the entities used in the REST protocol used in WaveJudge.
//!

use std::fmt::{Display, Formatter};
use std::str::FromStr;
use std::time::SystemTime;

use serde::{Serialize, Deserialize, Serializer};
use serde::de::{Deserializer, Visitor, Unexpected};

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

/// A heartbeat packet.
#[derive(Debug, Serialize, Clone)]
pub struct Heartbeat {
    /// Timestamp of the heartbeat packet. The timestamp is represented by the number of seconds
    /// elapsed from the UNIX_EPOCH (Jan. 1, 1970, 00:00:00 a.m.).
    #[serde(rename = "timestamp")]
    pub timestamp: u64,

    /// Number of CPU cores installed on this judge node.
    #[serde(rename = "cores")]
    pub cores: u32,

    /// Total physical memory installed on this judge node, in bytes.
    #[serde(rename = "totalPhysicalMemory")]
    pub total_physical_memory: u64,

    /// Free physical memory installed on this judge node, in bytes.
    #[serde(rename = "freePhysicalMemory")]
    pub free_physical_memory: u64,

    /// Total size of swap space, in bytes.
    #[serde(rename = "totalSwapSpace")]
    pub total_swap_space: u64,

    /// Size of free swap space, in bytes.
    #[serde(rename = "freeSwapSpace")]
    pub free_swap_space: u64,

    /// The size of the cached swap space.
    #[serde(rename = "cachedSwapSpace")]
    pub cached_swap_space: u64,
}

impl Heartbeat {
    /// Create a new `Heartbeat` value. This function panics if `SystemTime::duration_since`
    /// function fails when measuring elapsed number of seconds from `UNIX_EPOCH`.
    pub fn new() -> Self {
        let timestamp = SystemTime::now()
            .duration_since(SystemTime::UNIX_EPOCH)
            .expect("failed to measure elapsed time since UNIX_EPOCH")
            .as_secs();
        Heartbeat {
            timestamp,
            cores: 0,
            total_physical_memory: 0,
            free_physical_memory: 0,
            total_swap_space: 0,
            free_swap_space: 0,
            cached_swap_space: 0,
        }
    }
}

/// A language triple.
#[derive(Clone, Debug, Deserialize)]
pub struct LanguageTriple {
    /// Identifier of the language.
    #[serde(rename = "identifier")]
    pub identifier: String,

    /// Dialect of the language.
    #[serde(rename = "dialect")]
    pub dialect: String,

    /// Version of the language.
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

/// Judge mode.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Deserialize)]
pub enum JudgeMode {
    /// Standard mode.
    Standard,

    /// Special judge mode.
    SpecialJudge,

    /// Interactive mode.
    Interactive,
}

impl Default for JudgeMode {
    fn default() -> Self {
        JudgeMode::Standard
    }
}

/// Provide information about a problem.
#[derive(Clone, Debug, Deserialize)]
pub struct ProblemInfo {
    /// ID of the problem.
    #[serde(rename = "id")]
    pub id: ObjectId,

    /// Judge mode of the problem.
    #[serde(rename = "judgeMode")]
    pub judge_mode: JudgeMode,

    /// Time limit of the problem, in millisesconds.
    #[serde(rename = "timeLimit")]
    pub time_limit: u64,

    /// Memory limit of the problem, in megabytes.
    #[serde(rename = "memoryLimit")]
    pub memory_limit: u64,

    /// Source code of the jury program.
    #[serde(rename = "jurySource")]
    pub jury_src: String,

    /// Language of the jury program.
    #[serde(rename = "juryLanguage")]
    pub jury_lang: LanguageTriple,

    /// ID of the test archive.
    #[serde(rename = "archiveId")]
    pub archive_id: ObjectId,

    /// Timestamp of the problem metadata.
    #[serde(rename = "timestamp")]
    pub timestamp: u64,
}

/// Provide information about a submission.
#[derive(Clone, Debug, Deserialize)]
pub struct SubmissionInfo {
    /// ID of the submission.
    #[serde(rename = "id")]
    pub id: ObjectId,

    /// ID of the problem.
    #[serde(rename = "problemId")]
    pub problemId: ObjectId,

    /// The source code of the submission.
    #[serde(rename = "source")]
    pub source: String,

    /// Language of the submission.
    #[serde(rename = "language")]
    pub language: LanguageTriple,
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

