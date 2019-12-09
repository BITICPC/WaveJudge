//! This module maintains the archive cache, and provides facilities to access archive information.
//!

use std::collections::HashMap;
use std::convert::TryFrom;
use std::fmt::{Display, Formatter};
use std::fs::File;
use std::io::{Read, Seek};
use std::path::{Path, PathBuf};

use serde::{Serialize, Deserialize};
use zip::ZipArchive;
use zip::read::ZipFile;

error_chain::error_chain! {
    types {
        Error, ErrorKind, ResultExt, Result;
    }

    foreign_links {
        IoError(::std::io::Error);
        ZipError(::zip::result::ZipError);
    }

    errors {
        BadTestArchive(corruption: TestArchiveCorruption) {
            description("bad test archive"),
            display("bad test archive: {}", corruption)
        }
    }
}

/// Represent the reason why a test archive is considered to be corrupted.
#[derive(Debug, Clone)]
pub enum TestArchiveCorruption {
    /// Some input file is missing.
    MissingInputFile(PathBuf),

    /// Some answer file is missing.
    MissingAnswerFile(PathBuf),

    /// The test archive contains both th checker file and the interactor file.
    RedundantCheckerOrInteractor,

    /// Some entry cannot be categorized.
    UnknownEntry(PathBuf),
}

impl Display for TestArchiveCorruption {
    fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
        use TestArchiveCorruption::*;
        match self {
            MissingInputFile(path) =>
                f.write_fmt(format_args!("missing input file for entry: {}", path.display())),
            MissingAnswerFile(path) =>
                f.write_fmt(format_args!("missing answer file for entry: {}", path.display())),
            RedundantCheckerOrInteractor =>
                f.write_str("redundant checker or interactor"),
            UnknownEntry(path) =>
                f.write_fmt(format_args!("unknown entry: {}", path.display()))
        }
    }
}

/// Represent the kind of an entry in the test archive.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum TestArchiveEntryKind {
    /// The entry cannot be properly categorized.
    Unknown,

    /// The entry represents an input file.
    InputFile,

    /// The entry represents an answer file.
    AnswerFile,

    /// The entry represents the checker file.
    CheckerFile,

    /// The entry represents the interactor file.
    InteractorFile,
}

impl TestArchiveEntryKind {
    /// Get the kind of the given entry.
    fn get_kind<'a, 'b>(entry: &'a ZipFile<'b>) -> Self {
        const INPUT_FILE_EXTENSION: &'static str = "in";
        const ANSWER_FILE_EXTENSION: &'static str = "ans";
        const CHECKER_FILE_STEM: &'static str = "checker";
        const INTERACTOR_FILE_STEM: &'static str = "interactor";

        let entry_name = entry.sanitized_name();
        if entry_name.file_stem()
            .and_then(|stem| Some(stem == CHECKER_FILE_STEM))
            .unwrap_or(false) {
            return TestArchiveEntryKind::CheckerFile;
        }
        if entry_name.file_stem()
            .and_then(|stem| Some(stem == INTERACTOR_FILE_STEM))
            .unwrap_or(false) {
            return TestArchiveEntryKind::InteractorFile;
        }

        if entry_name.extension()
            .and_then(|ext| Some(ext == INPUT_FILE_EXTENSION))
            .unwrap_or(false) {
            return TestArchiveEntryKind::InputFile;
        }

        if entry_name.extension()
            .and_then(|ext| Some(ext == ANSWER_FILE_EXTENSION))
            .unwrap_or(false) {
            return TestArchiveEntryKind::AnswerFile;
        }

        TestArchiveEntryKind::Unknown
    }
}

/// Provide extension functions for `Path`.
trait PathExt {
    /// Returns a new `String` value holding the content of this `Path` value until the extension
    /// part. This function panics if the current `Path` contains invalid UTF-8 characters.
    fn strip_extension(self) -> String;
}

impl<'a> PathExt for &'a Path {
    fn strip_extension(self) -> String {
        let parent = match self.parent() {
            Some(p) => p,
            None => return self.file_stem().unwrap().to_str().unwrap().to_owned()
        };

        if parent.to_string_lossy().len() == 0 {
            return self.file_stem().unwrap().to_str().unwrap().to_owned();
        }

        format!("{}/{}",
            parent.to_str().unwrap(),
            self.file_stem().unwrap().to_str().unwrap())
    }
}

/// Provide metadata about a test case in the test archive.
#[derive(Debug, Serialize, Deserialize)]
pub struct TestCaseEntry {
    /// Path of the input file in the test archive, relative to the root of the archive.
    #[serde(rename = "input_file_name")]
    pub input_file_name: PathBuf,

    /// Path of the answer file in the test archive, relative to the root of the archive.
    #[serde(rename = "answer_file_name")]
    pub answer_file_name: PathBuf
}

impl TestCaseEntry {
    /// Create a new `TestCaseEntry` value.
    fn new<T1, T2>(input_file_name: T1, answer_file_name: T2) -> Self
        where T1: Into<PathBuf>, T2: Into<PathBuf> {
        TestCaseEntry {
            input_file_name: input_file_name.into(),
            answer_file_name: answer_file_name.into()
        }
    }
}

/// Provide metadata about a test archive.
#[derive(Debug, Serialize, Deserialize)]
pub struct TestArchiveMetadata {
    /// Path of the checker source file, relative to the root of the archive.
    #[serde(rename = "checker_file_name")]
    pub checker_file_name: Option<PathBuf>,

    /// Path of the interactor source file, relative to the root of the archive.
    #[serde(rename = "interactor_file_name")]
    pub interactor_file_name: Option<PathBuf>,

    /// Test cases contained in the archive.
    #[serde(rename = "test_cases")]
    pub test_cases: Vec<TestCaseEntry>,
}

impl<'a, R> TryFrom<&'a mut ZipArchive<R>> for TestArchiveMetadata
    where R: Read + Seek {
    type Error = Error;

    fn try_from(archive: &'a mut ZipArchive<R>) -> Result<Self> {
        let mut builder = TestArchiveMetadataBuilder::new();

        let archive_len = archive.len();
        for i in 0..archive_len {
            let archive_file = archive.by_index(i)?;
            let archive_file_path = archive_file.sanitized_name();

            match TestArchiveEntryKind::get_kind(&archive_file) {
                TestArchiveEntryKind::Unknown => {
                    return Err(Error::from(
                        ErrorKind::BadTestArchive(
                            TestArchiveCorruption::UnknownEntry(archive_file_path))));
                },
                TestArchiveEntryKind::InputFile => {
                    builder.add_input_file(archive_file_path);
                },
                TestArchiveEntryKind::AnswerFile => {
                    builder.add_answer_file(archive_file_path);
                },
                TestArchiveEntryKind::CheckerFile => {
                    builder.set_checker_file(archive_file_path)?;
                },
                TestArchiveEntryKind::InteractorFile => {
                    builder.set_interactor_file(archive_file_path)?;
                },
            }
        }

        builder.get_metadata()
    }
}

/// Implement a builder for `TestArchiveMetadata`.
struct TestArchiveMetadataBuilder {
    /// The checker file.
    checker_file: Option<PathBuf>,

    /// The interactor file.
    interactor_file: Option<PathBuf>,

    /// The test cases maintained.
    test_cases: HashMap<String, (Option<PathBuf>, Option<PathBuf>)>,
}

impl TestArchiveMetadataBuilder {
    /// Create a new `TestArchiveMetadataBuilder` instance.
    fn new() -> Self {
        TestArchiveMetadataBuilder {
            checker_file: None,
            interactor_file: None,
            test_cases: HashMap::new(),
        }
    }

    /// Checks that neither `self.checker_file` nor `self.interactor_file` is `Some(..)`. Returns
    /// `Err` if not satisfied.
    fn ensure_no_checker_or_interactor(&self) -> Result<()> {
        if !self.checker_file.is_none() || !self.interactor_file.is_none() {
            return Err(Error::from(
                ErrorKind::BadTestArchive(TestArchiveCorruption::RedundantCheckerOrInteractor)));
        }

        Ok(())
    }

    /// Set the path to the checker file. This function returns `Err` if either a checker or an
    /// interactor has already been set.
    fn set_checker_file<T>(&mut self, checker_file: T) -> Result<()>
        where T: Into<PathBuf> {
        self.ensure_no_checker_or_interactor()?;
        self.checker_file = Some(checker_file.into());
        Ok(())
    }

    /// Set the path to the interactor file. This function returns `Err` if either a checker or an
    /// interactor has already been set.
    fn set_interactor_file<T>(&mut self, interactor_file: T) -> Result<()>
        where T: Into<PathBuf> {
        self.ensure_no_checker_or_interactor()?;
        self.interactor_file = Some(interactor_file.into());
        Ok(())
    }

    /// Add an input file to the metadata.
    fn add_input_file<T>(&mut self, input_file: T)
        where T: Into<PathBuf> {
        let input_file = input_file.into();
        let test_case_name = input_file.strip_extension();

        match self.test_cases.get_mut(&test_case_name) {
            Some(record) => {
                record.0 = Some(input_file);
            },
            None => {
                self.test_cases.insert(test_case_name, (Some(input_file), None));
            }
        };
    }

    /// Add an answer file to the metadata.
    fn add_answer_file<T>(&mut self, answer_file: T)
        where T: Into<PathBuf> {
        let answer_file = answer_file.into();
        let test_case_name = answer_file.strip_extension();

        match self.test_cases.get_mut(&test_case_name) {
            Some(record) => {
                record.1 = Some(answer_file);
            },
            None => {
                self.test_cases.insert(test_case_name, (None, Some(answer_file)));
            }
        };
    }

    /// Checks all values in `self.test_cases` matches the pattern `(Some(..), Some(..))`. This
    /// function returns `Err` if not satisfied.
    fn ensure_test_cases_integrity(&self) -> Result<()> {
        for tc in self.test_cases.values() {
            match tc {
                (Some(..), Some(..)) => continue,
                (Some(input_file), None) =>
                    return Err(Error::from(ErrorKind::BadTestArchive(
                        TestArchiveCorruption::MissingAnswerFile(input_file.clone())))),
                (None, Some(answer_file)) =>
                    return Err(Error::from(ErrorKind::BadTestArchive(
                        TestArchiveCorruption::MissingInputFile(answer_file.clone())))),
                _ => unreachable!()
            };
        }

        Ok(())
    }

    /// Build the metadata value.
    fn get_metadata(self) -> Result<TestArchiveMetadata> {
        self.ensure_test_cases_integrity()?;

        Ok(TestArchiveMetadata {
            checker_file_name: self.checker_file,
            interactor_file_name: self.interactor_file,
            test_cases: self.test_cases.into_iter()
                .map(|tc| TestCaseEntry::new((tc.1).0.unwrap(), (tc.1).1.unwrap()))
                .collect()
        })
    }
}

/// Provide information about a test archive.
#[derive(Debug)]
pub struct TestArchive<R>
    where R: Read + Seek {
    /// The `ZipArchive` representation about the test archive.
    archive: ZipArchive<R>,

    /// The metadata about the archive.
    pub metadata: TestArchiveMetadata,
}

impl<R> TestArchive<R>
    where R: Read + Seek {
    /// Create a new `TestArchive` value from the given zip archive.
    pub fn new(mut archive: ZipArchive<R>) -> Result<Self> {
        let metadata = TestArchiveMetadata::try_from(&mut archive)?;
        Ok(TestArchive { archive, metadata })
    }
}

impl TestArchive<File> {
    /// Create a new `TestArchive` value from the given file. The file should be a valid zip
    /// archive.
    pub fn from_file<T>(path: T) -> Result<Self>
        where T: AsRef<Path> {
        let file = File::open(path)?;
        let archive = ZipArchive::new(file)?;
        TestArchive::new(archive)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    mod path_ext_tests {
        use super::*;

        use std::str::FromStr;

        #[test]
        fn strip_extension_no_parent_no_ext() {
            let path = PathBuf::from_str("hello").unwrap();
            assert_eq!("hello", path.strip_extension());
        }

        #[test]
        fn strip_extension_no_parent() {
            let path = PathBuf::from_str("hello.world").unwrap();
            assert_eq!("hello", path.strip_extension());
        }

        #[test]
        fn strip_extension_relative_no_parent_no_ext() {
            let path = PathBuf::from_str("path/to/hello").unwrap();
            assert_eq!("path/to/hello", path.strip_extension());
        }

        #[test]
        fn strip_extension_relative_no_parent() {
            let path = PathBuf::from_str("path/to/hello.world").unwrap();
            assert_eq!("path/to/hello", path.strip_extension());
        }

        #[test]
        fn strip_extension_abs_no_ext() {
            let path = PathBuf::from_str("/path/to/hello").unwrap();
            assert_eq!("/path/to/hello", path.strip_extension());
        }

        #[test]
        fn strip_extension_abs() {
            let path = PathBuf::from_str("/path/to/hello.world").unwrap();
            assert_eq!("/path/to/hello", path.strip_extension());
        }
    }

    mod test_archive_metadata_builder_tests {
        use super::*;

        #[test]
        fn set_checker_file_ok() {
            let mut builder = TestArchiveMetadataBuilder::new();
            assert!(builder.set_checker_file("path/to/checker").is_ok());

            let metadata = builder.get_metadata().unwrap();
            assert_eq!(PathBuf::from("path/to/checker"), metadata.checker_file_name.unwrap());
        }

        #[test]
        fn set_checker_file_dup_checker() {
            let mut builder = TestArchiveMetadataBuilder::new();
            builder.set_checker_file("path/to/checker").unwrap();
            assert!(builder.set_checker_file("path/to/checker").is_err());
        }

        #[test]
        fn set_checker_file_dup_interactor() {
            let mut builder = TestArchiveMetadataBuilder::new();
            builder.set_interactor_file("path/to/interactor").unwrap();
            assert!(builder.set_checker_file("path/to/checker").is_err());
        }

        #[test]
        fn set_interactor_ok() {
            let mut builder = TestArchiveMetadataBuilder::new();
            assert!(builder.set_interactor_file("path/to/interactor").is_ok());

            let metadata = builder.get_metadata().unwrap();
            assert_eq!(PathBuf::from("path/to/interactor"), metadata.interactor_file_name.unwrap());
        }

        #[test]
        fn set_interactor_dup_checker() {
            let mut builder = TestArchiveMetadataBuilder::new();
            builder.set_checker_file("path/to/checker").unwrap();
            assert!(builder.set_interactor_file("path/to/interactor").is_err());
        }

        #[test]
        fn set_interactor_dup_interactor() {
            let mut builder = TestArchiveMetadataBuilder::new();
            builder.set_interactor_file("path/to/interactor").unwrap();
            assert!(builder.set_interactor_file("path/to/interactor").is_err());
        }

        #[test]
        fn miss_input_file() {
            let mut builder = TestArchiveMetadataBuilder::new();
            builder.add_answer_file("path/to/answer.ans");
            assert!(builder.get_metadata().is_err());
        }

        #[test]
        fn miss_answer_file() {
            let mut builder = TestArchiveMetadataBuilder::new();
            builder.add_input_file("path/to/input.in");
            assert!(builder.get_metadata().is_err());
        }

        #[test]
        fn normal() {
            let mut builder = TestArchiveMetadataBuilder::new();
            builder.set_checker_file("checker.cpp").unwrap();
            builder.add_input_file("tc1.in");
            builder.add_answer_file("tc1.ans");
            builder.add_input_file("subdir/tc2.in");
            builder.add_answer_file("subdir/tc2.ans");
            let metadata = builder.get_metadata().unwrap();

            assert_eq!(PathBuf::from("checker.cpp"), metadata.checker_file_name.unwrap());
            assert!(metadata.interactor_file_name.is_none());

            let mut mask = 0u32;
            for tc in metadata.test_cases.iter() {
                if tc.input_file_name == PathBuf::from("tc1.in") {
                    mask |= 1u32;
                    assert_eq!(PathBuf::from("tc1.ans"), tc.answer_file_name);
                } else if tc.input_file_name == PathBuf::from("subdir/tc2.in") {
                    mask |= 2u32;
                    assert_eq!(PathBuf::from("subdir/tc2.ans"), tc.answer_file_name);
                } else {
                    assert!(false);
                }
            }

            assert_eq!(3, mask);
        }
    }
}
