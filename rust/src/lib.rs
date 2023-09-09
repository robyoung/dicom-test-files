//! A collection of DICOM files for testing DICOM parsers.
//!
//! To avoid users having to download all the files they are downloaded as they
//! are needed and cached in the `/target` directory.
//!
//! The [`path`] function will automatically download the requested file
//! and return a file path.
//!
//! ```no_run
//! use dicom_test_files::path;
//!
//! # fn main() -> Result<(), dicom_test_files::Error> {
//! let liver = path("pydicom/liver.dcm")?;
//! // then open the file as you will (e.g. using DICOM-rs)
//! # /*
//! let dicom_data = dicom::object::open(liver);
//! # */
//! # Ok(())
//! # }
//! ```
//! 
//! ## Source of data
//! 
//! By default,
//! all data sets are hosted in
//! the `dicom-test-files` project's [main repository][1],
//! in the `data` folder.
//! Inspect this folder to know what DICOM test files are available.
//!
//! To override this source,
//! you can set the environment variable `DICOM_TEST_FILES_URL`
//! to the base path of the data set's raw contents
//! (usually ending with `data` or `data/`).
//! 
//! ```sh
//! set DICOM_TEST_FILES_URL=https://raw.githubusercontent.com/Me/dicom-test-files/new/more-dicom/data
//! cargo test
//! ```
//! 
//! [1]: https://github.com/robyoung/dicom-test-files/tree/master/data

#![deny(missing_docs)]

use sha2::{Digest, Sha256};
use std::{
    borrow::Cow,
    env::{self, VarError},
    fs, io,
    path::{Path, PathBuf},
};

mod hashes;

use crate::hashes::FILE_HASHES;

/// Error type for test_dicom_files
#[derive(Debug)]
pub enum Error {
    /// Returned when the provided name does not exist in the hash list
    ///
    /// If you are sure it does exist you may need to update to a newer version dicom_test_files.
    NotFound,
    /// Returned when the hash of the downloaded file does not match the previously generated hash
    ///
    /// This may mean you need to update to a newer version of dicom_test_files.
    InvalidHash,
    /// Returned when the file cannot be downloaded. Contains the generated URL.
    Download(String),
    /// Wrapped errors from std::io
    Io(io::Error),
    /// Failed to resolve data source URL
    ResolveUrl(VarError),
}

impl From<io::Error> for Error {
    fn from(err: io::Error) -> Error {
        Error::Io(err)
    }
}

/// Fetch a DICOM file by its relative path (`name`)
/// if it has not been downloaded yet,
/// and return its path in the local file system.
///
/// This function will download and cache the file locally in
/// `target/dicom_test_files`.
pub fn path(name: &str) -> Result<PathBuf, Error> {
    let cached_path = get_data_path().join(name);
    if !cached_path.exists() {
        download(name, &cached_path)?;
    }
    Ok(cached_path)
}

/// Return a vector of local paths to all DICOM test files available.
///
/// This function will download any test file not yet in the file system
/// and cache the files locally to `target/dicom_test_files`.
///
/// Note that this operation may be unnecessarily expensive.
/// Retrieving only the files that you need via [`path`] is preferred.
#[deprecated(note = "Too expensive. Use `path` for the files that you need.")]
pub fn all() -> Result<Vec<PathBuf>, Error> {
    FILE_HASHES
        .iter()
        .map(|(name, _)| path(name))
        .collect::<Result<Vec<PathBuf>, Error>>()
}

/// Determine the target data path
pub(crate) fn get_data_path() -> PathBuf {
    let mut target_dir = PathBuf::from(
        env::current_exe()
            .expect("exe path")
            .parent()
            .expect("exe parent"),
    );
    while target_dir.file_name() != Some(std::ffi::OsStr::new("target")) {
        if !target_dir.pop() {
            panic!("Cannot find target directory");
        }
    }
    target_dir.join("dicom_test_files")
}

const DEFAULT_GITHUB_BASE_URL: &str =
    "https://raw.githubusercontent.com/robyoung/dicom-test-files/master/data/";

const RAW_GITHUBUSERCONTENT_URL: &str = "https://raw.githubusercontent.com";

/// Determine the base URL in this environment.
///
/// When this is part of a pull request to the project,
/// use the contents provided through the pull request's head branch.
fn base_url() -> Result<Cow<'static, str>, VarError> {
    if let Ok(url) = std::env::var("DICOM_TEST_FILES_URL") {
        if url != "" {
            let url = if !url.ends_with("/") {
                format!("{url}/")
            } else {
                url
            };
            return Ok(url.into());
        }
    }

    // CI: always true on GitHub Actions
    let ci = std::env::var("CI").unwrap_or_default();
    if ci == "true" {
        // GITHUB_REPOSITORY
        let github_repository = std::env::var("GITHUB_REPOSITORY").unwrap_or_default();

        // only do this if target repository is dicom-test-files
        if github_repository.ends_with("/dicom-test-files") {
            // GITHUB_EVENT_NAME: can be pull_request
            let github_event_name = std::env::var("GITHUB_EVENT_NAME")?;
            if github_event_name == "pull_request" {
                // GITHUB_HEAD_REF: name of the branch when it's a pull request
                let github_head_ref = std::env::var("GITHUB_HEAD_REF")?;
                let url = format!(
                    "{}/{}/{}/data/",
                    RAW_GITHUBUSERCONTENT_URL, github_repository, github_head_ref
                );

                return Ok(url.into());
            }
        }
    }

    Ok(DEFAULT_GITHUB_BASE_URL.into())
}

fn download(name: &str, cached_path: &PathBuf) -> Result<(), Error> {
    check_hash_exists(name)?;

    let target_parent_dir = cached_path.as_path().parent().unwrap();
    fs::create_dir_all(target_parent_dir)?;

    let url = base_url().map_err(Error::ResolveUrl)?.to_owned() + name;
    let resp = ureq::get(&url)
        .call()
        .map_err(|e| Error::Download(format!("Failed to download {}: {}", url, e)))?;

    // write into temporary file first
    let tempdir = tempfile::tempdir_in(target_parent_dir)?;
    let mut tempfile_path = tempdir.into_path();
    tempfile_path.push("tmpfile");

    {
        let mut target = fs::File::create(tempfile_path.as_path())?;
        std::io::copy(&mut resp.into_reader(), &mut target)?;
    }

    // move to target destination
    fs::rename(tempfile_path, cached_path.as_path())?;

    check_hash(cached_path.as_path(), name)?;

    Ok(())
}

fn check_hash_exists(name: &str) -> Result<(), Error> {
    for (hash_name, _) in FILE_HASHES.iter() {
        if *hash_name == name {
            return Ok(());
        }
    }
    Err(Error::NotFound)
}

fn check_hash(path: &Path, name: &str) -> Result<(), Error> {
    let mut file = fs::File::open(path)?;
    let mut hasher = Sha256::new();
    io::copy(&mut file, &mut hasher)?;
    let hash = hasher.finalize();

    for (hash_name, file_hash) in FILE_HASHES.iter() {
        if *hash_name == name {
            if format!("{:x}", hash) == *file_hash {
                return Ok(());
            } else {
                fs::remove_file(path)?;
                return Err(Error::InvalidHash);
            }
        }
    }

    unreachable!("file existance was checked before downloading");
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn load_a_single_path_1() {
        // ensure it does not exist
        let cached_path = get_data_path().join("pydicom/liver.dcm");
        let _ = fs::remove_file(cached_path);

        let path = path("pydicom/liver.dcm").unwrap();
        let path = path.as_path();

        assert_eq!(path.file_name().unwrap(), "liver.dcm");
        assert!(path.exists());
    }

    #[test]
    fn load_a_single_path_wg04_1() {
        const FILE: &str = "WG04/JPLY/NM1_JPLY";
        // ensure it does not exist
        let cached_path = get_data_path().join(FILE);
        let _ = fs::remove_file(cached_path);

        let path = path(FILE).unwrap();
        let path = path.as_path();

        assert_eq!(path.file_name().unwrap(), "NM1_JPLY");
        assert!(path.exists());

        let metadata = std::fs::metadata(path).unwrap();
        // check size
        assert_eq!(metadata.len(), 9844);
    }

    fn load_a_single_path_2() {
        // ensure it does not exist
        let cached_path = get_data_path().join("pydicom/CT_small.dcm");
        let _ = fs::remove_file(cached_path);

        let path = path("pydicom/CT_small.dcm").unwrap();
        let path = path.as_path();

        assert_eq!(path.file_name().unwrap(), "CT_small.dcm");
        assert!(path.exists());
    }

    #[test]
    fn load_a_single_path_concurrent() {
        let handles: Vec<_> = (0..4)
            .map(|_| std::thread::spawn(load_a_single_path_2))
            .collect();
        for h in handles {
            h.join().unwrap();
        }
    }
}
