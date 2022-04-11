//! A collection of DICOM files for testing DICOM parsers.
//!
//! To avoid users having to download all the files they are downloaded as they
//! are needed and cached in the `/target` directory.
//!
//! ```no_run
//! use dicom_test_files;
//!
//! dicom_test_files::path("pydicom/liver.dcm").unwrap();
//! ```
#![deny(missing_docs)]

use sha2::{Digest, Sha256};
use std::{
    env, fs, io,
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
}

impl From<io::Error> for Error {
    fn from(err: io::Error) -> Error {
        Error::Io(err)
    }
}

/// Return the local path for a given DICOM file
///
/// This function will download and cache the file locally in `target/dicom_test_files`
/// if it has not already been downloaded.
pub fn path(name: &str) -> Result<PathBuf, Error> {
    let cached_path = get_data_path().join(name);
    if !cached_path.exists() {
        download(name, &cached_path)?;
    }
    Ok(cached_path)
}

/// Return a vector of local paths to all DICOM files
///
/// This function will download and cache the file locally in `target/dicom_test_files`
/// if it has not already been downloaded.
pub fn all() -> Result<Vec<PathBuf>, Error> {
    FILE_HASHES
        .iter()
        .map(|(name, _)| path(name))
        .collect::<Result<Vec<PathBuf>, Error>>()
}

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

const GITHUB_BASE_URL: &str =
    "https://raw.githubusercontent.com/robyoung/dicom-test-files/master/data/";

fn download(name: &str, cached_path: &PathBuf) -> Result<(), Error> {
    check_hash_exists(name)?;

    let target_parent_dir = cached_path.as_path().parent().unwrap();
    fs::create_dir_all(target_parent_dir)?;

    let url = GITHUB_BASE_URL.to_owned() + name;
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

    #[test]
    fn load_all_paths() {
        let all = all().unwrap();
        assert_eq!(all.len(), 126);
    }
}
