//! This module maintains the archive cache, and provides facilities to access archive information.
//!

use std::collections::HashMap;
use std::convert::TryFrom;
use std::fmt::{Display, Formatter};
use std::fs::File;
use std::io::{Read, Seek, SeekFrom};
use std::path::{Path, PathBuf};
use std::str::FromStr;
use std::string::ToString;
use std::sync::Arc;

use serde::{Serialize, Deserialize};
use zip::ZipArchive;
use zip::read::ZipFile;

use crate::restful::RestfulClient;
use crate::restful::entities::ObjectId;

error_chain::error_chain! {
    types {
        Error, ErrorKind, ResultExt, Result;
    }

    links {
        Restful(crate::restful::Error, crate::restful::ErrorKind);
    }

    foreign_links {
        IoError(::std::io::Error);
        ZipError(::zip::result::ZipError);
        SerdeJsonError(::serde_json::Error);
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
            UnknownEntry(path) =>
                f.write_fmt(format_args!("unknown entry: {}", path.display()))
        }
    }
}

/// Extension of the input files inside a test archive.
const INPUT_FILE_EXTENSION: &'static str = "in";

/// Extension of the answer files inside a test archive.
const ANSWER_FILE_EXTENSION: &'static str = "ans";

/// Represent the kind of an entry in the test archive.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum TestArchiveEntryKind {
    /// The entry cannot be properly categorized.
    Unknown,

    /// The entry represents an input file.
    InputFile,

    /// The entry represents an answer file.
    AnswerFile,
}

impl TestArchiveEntryKind {
    /// Get the kind of the given entry.
    fn get_kind<'a, 'b>(entry: &'a ZipFile<'b>) -> Self {
        let entry_name = entry.sanitized_name();
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
struct TestCaseEntry {
    /// The name of the test case. The name of a test case is the portion of its file path before
    /// the extension, which should be identical to the input file and the answer file.
    ///
    /// For example, the name of the test case whose input file is "path/to/test.in" and answer
    /// file is "path/to/test.ans" is "path/to/test".
    name: String,
}

impl TestCaseEntry {
    /// Create a new `TestCaseEntry` value.
    fn new<T>(name: T) -> Self
        where T: ToString {
        TestCaseEntry {
            name: name.to_string()
        }
    }

    /// Get the path to the input file of this test case.
    fn input_file_path(&self) -> PathBuf {
        let mut p = PathBuf::from_str(&self.name).unwrap();
        p.set_extension(INPUT_FILE_EXTENSION);
        p
    }

    /// Get the path to the answer file of this test case.
    fn answer_file_path(&self) -> PathBuf {
        let mut p = PathBuf::from_str(&self.name).unwrap();
        p.set_extension(ANSWER_FILE_EXTENSION);
        p
    }
}

/// Provide metadata about a test archive.
#[derive(Debug, Serialize, Deserialize)]
struct TestArchiveMetadata {
    /// Test cases contained in the archive.
    #[serde(rename = "test_cases")]
    test_cases: Vec<TestCaseEntry>,
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
            }
        }

        builder.get_metadata()
    }
}

/// Implement a builder for `TestArchiveMetadata`.
struct TestArchiveMetadataBuilder {
    /// The test cases maintained.
    test_cases: HashMap<String, (Option<PathBuf>, Option<PathBuf>)>,
}

impl TestArchiveMetadataBuilder {
    /// Create a new `TestArchiveMetadataBuilder` instance.
    fn new() -> Self {
        TestArchiveMetadataBuilder {
            test_cases: HashMap::new(),
        }
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
            test_cases: self.test_cases.into_iter()
                .map(|tc| TestCaseEntry::new(tc.0))
                .collect()
        })
    }
}

/// Provide information about a test archive.
#[derive(Debug)]
struct TestArchive<R>
    where R: Read + Seek {
    /// The `ZipArchive` representation about the test archive.
    archive: ZipArchive<R>,

    /// The metadata about the archive.
    metadata: TestArchiveMetadata,
}

impl<R> TestArchive<R>
    where R: Read + Seek {
    /// Create a new `TestArchive` value from the given zip archive.
    fn new(mut archive: ZipArchive<R>) -> Result<Self> {
        let metadata = TestArchiveMetadata::try_from(&mut archive)?;
        Ok(TestArchive { archive, metadata })
    }

    /// Create a new `TestArchive` value from the given `Read` object.
    fn new_from_read(source: R) -> Result<Self> {
        TestArchive::new(ZipArchive::new(source)?)
    }
}

impl TestArchive<File> {
    /// Create a new `TestArchive` value from the given file. The file should be a valid zip
    /// archive.
    fn from_file<T>(path: T) -> Result<Self>
        where T: AsRef<Path> {
        let file = File::open(path)?;
        let archive = ZipArchive::new(file)?;
        TestArchive::new(archive)
    }
}

/// Provide a trait for types whose contents can be extracted into a specific directory.
trait Extractable {
    /// The error type returned from extract operation on this type.
    type Error;

    /// Extract the contents of the `ZipArchive` into the specified directory.
    fn extract_into<P>(&mut self, dir: P) -> std::result::Result<(), Self::Error>
        where P: AsRef<Path>;
}

impl<R> Extractable for ZipArchive<R>
    where R: Seek + Read {
    type Error = Error;

    fn extract_into<P>(&mut self, dir: P) -> std::result::Result<(), Self::Error>
        where P: AsRef<Path> {
        let num_files = self.len();
        for i in 0..num_files {
            let mut archive_file = self.by_index(i)?;

            let mut archive_file_path = dir.as_ref().to_owned();
            archive_file_path.push(archive_file.sanitized_name());
            let mut output_file = File::create(&archive_file_path)?;

            std::io::copy(&mut archive_file, &mut output_file)?;
        }

        Ok(())
    }
}

impl<R> Extractable for TestArchive<R>
    where R: Seek + Read {
    type Error = Error;

    fn extract_into<P>(&mut self, dir: P) -> std::result::Result<(), Self::Error>
        where P: AsRef<Path> {
        self.archive.extract_into(dir)
    }
}

/// Provide an interator over a test archive represented by `TestArchiveHandle`.
pub struct TestArchiveEntryIterator<'a> {
    handle: &'a TestArchiveHandle,
    inner: std::slice::Iter<'a, TestCaseEntry>
}

impl<'a> TestArchiveEntryIterator<'a> {
    /// Create a new `TestArchiveEntryIterator` value that iterates the test cases in the test archive
    /// represented by the given `TestArchiveHandle` value.
    pub fn new(handle: &'a TestArchiveHandle) -> Self {
        TestArchiveEntryIterator {
            handle,
            inner: handle.metadata.test_cases.iter()
        }
    }
}

impl<'a> Iterator for TestArchiveEntryIterator<'a> {
    type Item = TestCaseInfo<'a>;

    fn next(&mut self) -> Option<Self::Item> {
        self.inner.next().map(|entry| TestCaseInfo::new(self.handle, entry))
    }
}

/// Provide access to a saved test archive.
pub struct TestArchiveHandle {
    /// The directory in which the contents of the test archive are saved.
    dir: PathBuf,

    /// The metadata of the test archive.
    metadata: TestArchiveMetadata,
}

impl TestArchiveHandle {
    /// Create a new `TestArchiveHandle` value representing the test archive residing in the
    /// specific directory.
    fn new<P1, P2>(dir: P1, metadata_file_path: P2) -> Result<Self>
        where P1: AsRef<Path>, P2: AsRef<Path> {
        let mut metadata_file = File::open(&metadata_file_path)?;
        let metadata: TestArchiveMetadata = serde_json::from_reader(&mut metadata_file)?;

        Ok(TestArchiveHandle {
            dir: dir.as_ref().to_owned(),
            metadata
        })
    }

    /// Get an iterator over the test cases contained in this test archive.
    pub fn test_cases<'a>(&'a self) -> TestArchiveEntryIterator<'a> {
        TestArchiveEntryIterator::new(self)
    }
}

/// Represent a test case in a test archive.
pub struct TestCaseInfo<'a> {
    /// The handle to the test archive containing this test case.
    handle: &'a TestArchiveHandle,

    /// The test case entry in the test archive.
    test_case_entry: &'a TestCaseEntry,
}

impl<'a> TestCaseInfo<'a> {
    /// Create a new `TestCaseInfo` value.
    fn new(handle: &'a TestArchiveHandle, test_case_entry: &'a TestCaseEntry) -> Self {
        TestCaseInfo { handle, test_case_entry }
    }

    /// Get the path to the input file of this test case.
    pub fn input_file_path(&self) -> PathBuf {
        let mut p = self.handle.dir.clone();
        p.push(self.test_case_entry.input_file_path());
        p
    }

    /// Get the path to the answer file of this test case.
    pub fn answer_file_path(&self) -> PathBuf {
        let mut p = self.handle.dir.clone();
        p.push(self.test_case_entry.answer_file_path());
        p
    }

    /// Open the input file of this test case.
    pub fn open_input_file(&self) -> std::io::Result<File> {
        File::open(&self.input_file_path())
    }

    /// Open the answer file of this test case.
    pub fn open_answer_file(&self) -> std::io::Result<File> {
        File::open(&self.answer_file_path())
    }
}

/// Provide access to local archive store.
pub struct ArchiveStore {
    /// The root directory of the archive store on the local disk.
    root_dir: PathBuf,

    /// The RESTful client connected to the judge board server.
    rest: Arc<RestfulClient>,
}

impl ArchiveStore {
    /// Create a new `ArchiveStore` instance.
    pub(super) fn new<P>(dir: &P, rest: Arc<RestfulClient>) -> ArchiveStore
        where P: ?Sized + AsRef<Path> {
        let dir = dir.as_ref();
        ArchiveStore {
            root_dir: dir.to_owned(),
            rest
        }
    }

    /// Get the directory containing the content of the archive with the specified ID.
    fn get_archive_dir(&self, id: ObjectId) -> PathBuf {
        let mut dir = self.root_dir.clone();
        dir.push(id.to_string());
        dir
    }

    /// Get the path of the metadata file inside the speicified archive directory.
    fn get_metadata_file_path<P>(&self, archive_dir: P) -> PathBuf
        where P: AsRef<Path> {
        let mut path = archive_dir.as_ref().to_owned();
        path.push("metadata.json");
        path
    }

    /// Extract the content of the given test archive into the specified directory.
    fn extract_archive<R, T>(&self, mut archive: TestArchive<R>, archive_dir: T) -> Result<()>
        where R: Seek + Read, T: AsRef<Path> {
        let archive_metadata = &archive.metadata;
        log::debug!("Archive metadata extracted: {:?}", archive_metadata);

        // Create the archive directory.
        let archive_dir = archive_dir.as_ref();
        std::fs::create_dir_all(archive_dir)?;

        // Save the metadata to file: ${archive_dir}/metadata.json
        let metadata_file_path = self.get_metadata_file_path(archive_dir);
        let mut metadata_file = File::create(&metadata_file_path)?;
        serde_json::to_writer(&mut metadata_file, archive_metadata)?;
        drop(metadata_file);

        // Extract the contents of the test archive into the archive directory.
        archive.extract_into(archive_dir)?;

        Ok(())
    }

    /// Download the specified test archive, verify and extract to the specified archive directory.
    fn download_archive<T>(&self, id: ObjectId, archive_dir: T) -> Result<()>
        where T: AsRef<Path> {
        // Create a temporary file and download the test archive from the judge board server.
        log::info!("Downloading archive {}", id);
        let mut archive_file = tempfile::tempfile()?;
        self.rest.download_archive(id, &mut archive_file)?;

        log::info!("Verifying archive {}", id);
        archive_file.seek(SeekFrom::Start(0))?;
        let archive = TestArchive::new_from_read(archive_file)?;

        let archive_dir = archive_dir.as_ref();
        log::info!("Extracting archive {} into {}", id, archive_dir.display());
        self.extract_archive(archive, archive_dir)
    }

    /// Get archive with the given ID. If the archive does not exist on the local disk, this
    /// function will request the judge board to download it. This function will not return until
    /// the archive is ready or something goes wrong.
    ///
    /// `rest` is a `RestfulClient` object that has connected to the judge board through which the
    /// missing archive will be downloaded.
    pub fn get_or_download(&self, id: ObjectId) -> Result<TestArchiveHandle> {
        let archive_dir = self.get_archive_dir(id);
        if !archive_dir.exists() {
            self.download_archive(id, &archive_dir)?;
        }

        let metadata_file_path = self.get_metadata_file_path(&archive_dir);
        TestArchiveHandle::new(&archive_dir, &metadata_file_path)
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
            builder.add_input_file("tc1.in");
            builder.add_answer_file("tc1.ans");
            builder.add_input_file("subdir/tc2.in");
            builder.add_answer_file("subdir/tc2.ans");
            let metadata = builder.get_metadata().unwrap();

            let mut mask = 0u32;
            for tc in metadata.test_cases.iter() {
                if tc.name == "tc1" {
                    mask |= 1;
                } else if tc.name == "subdir/tc2" {
                    mask |= 2;
                } else {
                    assert!(false);
                }
            }

            assert_eq!(3, mask);
        }
    }
}
