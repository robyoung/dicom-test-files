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
use test_file::{TestFile, Compression};
use std::{
    borrow::Cow,
    env::{self, VarError},
    fs, io,
    path::{Path, PathBuf},
};

mod entries;

pub(crate) mod test_file;


use entries::FILE_ENTRIES;

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
    /// Feature "zstd" is required for this file 
    ZstdRequired,
}

impl From<io::Error> for Error {
    fn from(err: io::Error) -> Error {
        Error::Io(err)
    }
}

type Result<T, E = Error> = std::result::Result<T, E>;

fn lookup(name: &str) -> Option<&'static TestFile> {
    FILE_ENTRIES.iter().find(|entry| entry.name == name)
}

/// Fetch a DICOM file by its relative path (`name`)
/// if it has not been downloaded yet,
/// and return its path in the local file system.
///
/// This function will download and cache the file locally in
/// `target/dicom_test_files`.
pub fn path(name: &str) -> Result<PathBuf, Error> {
    let entry = lookup(name).ok_or(Error::NotFound)?;
    let cached_path = get_data_path().join(entry.name);
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
    FILE_ENTRIES
        .iter()
        .map(|TestFile { name, ..}| path(name))
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
    let file_entry = lookup(name).ok_or(Error::NotFound)?;

    let target_parent_dir = cached_path.as_path().parent().unwrap();
    fs::create_dir_all(target_parent_dir)?;

    let url = base_url().map_err(Error::ResolveUrl)?.to_owned() + file_entry.real_file_name();
    let resp = ureq::get(&url)
        .call()
        .map_err(|e| Error::Download(format!("Failed to download {}: {}", url, e)))?;

    // write into temporary file first
    let tempdir = tempfile::tempdir_in(target_parent_dir)?;
    let mut tempfile_path = tempdir.into_path();
    tempfile_path.push("tmpfile");

    {
        let mut target = fs::File::create(&tempfile_path)?;
        std::io::copy(&mut resp.into_reader(), &mut target)?;
    }

    check_hash(&tempfile_path, file_entry)?;
    match file_entry.compression {
        Compression::None => {
            // move to target destination
            fs::rename(tempfile_path, cached_path.as_path())?;
        },
        Compression::Zstd => {
            // decode and write to target destination
            write_zstd(tempfile_path.as_path(), cached_path.as_path())?;

            // remove temporary file
            fs::remove_file(tempfile_path).unwrap_or_else(|e| {
                eprintln!("[dicom-test-files] Failed to remove temporary file: {}", e);
            });
        }
    }

    Ok(())
}

#[cfg(feature = "zstd")]
fn write_zstd(source_path: impl AsRef<Path>, cached_path: impl AsRef<Path>) -> Result<()> {
    let mut decoder = zstd::Decoder::new(fs::File::open(source_path)?)?;
    let mut target = fs::File::create(cached_path)?;
    std::io::copy(&mut decoder, &mut target)?;
    Ok(())
}

#[cfg(not(feature = "zstd"))]
fn write_zstd(_source_path: impl AsRef<Path>, _cached_path: impl AsRef<Path>) -> Result<()> {
    Err(Error::ZstdRequired)
}

fn check_hash(path: impl AsRef<Path>, file_entry: &TestFile) -> Result<()> {
    let mut file = fs::File::open(path.as_ref())?;
    let mut hasher = Sha256::new();
    io::copy(&mut file, &mut hasher)?;
    let hash = hasher.finalize();

    if format!("{:x}", hash) != file_entry.hash {
        fs::remove_file(path)?;
        return Err(Error::InvalidHash);
    }

    Ok(())
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

    #[cfg(feature = "zstd")]
    #[test]
    fn load_path_wg04_unc_1() {
        const FILE: &str = "WG04/REF/NM1_UNC";
        // ensure it does not exist beforehand
        let cached_path = get_data_path().join(FILE);
        let _ = fs::remove_file(cached_path);

        let path = path(FILE).unwrap();
        let path = path.as_path();

        assert_eq!(path.file_name().unwrap(), "NM1_UNC");
        assert!(path.exists());

        let metadata = std::fs::metadata(path).unwrap();
        // check size
        assert_eq!(metadata.len(), 527066);
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
