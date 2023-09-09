//! Test file base data structures

use std::{borrow::Cow, ffi::{OsStr, OsString}};
#[derive(Debug)]
pub enum Compression {
    /// no compression
    None,
    /// Zstandard compression
    Zstd,
}

/// Test file descriptor
#[derive(Debug)]
pub struct TestFile {
    /// path identifier to the test file
    pub name: &'static str,
    /// whether the file was subjected to compression
    pub compression: Compression,
    /// SHA-256 hash of the file's data (post-compression)
    pub hash: &'static str,
}

impl TestFile {
    pub const fn new(name: &'static str, compression: Compression, hash: &'static str) -> Self {
        Self {
            name,
            compression,
            hash,
        }
    }

    pub const fn none(name: &'static str, hash: &'static str) -> Self {
        Self::new(name, Compression::None, hash)
    }

    pub const fn zstd(name: &'static str, hash: &'static str) -> Self {
        Self::new(name, Compression::Zstd, hash)
    }

    pub fn real_file_name(&self) -> Cow<'static, str> {
        match self.compression {
            Compression::None => Cow::Borrowed(self.name),
            Compression::Zstd => Cow::Owned(format!("{}.zst", self.name)),
        }
    }

    pub fn real_os_file_name(&self) -> Cow<'static, OsStr> {
        match self.compression {
            Compression::None => Cow::Borrowed(OsStr::new(self.name)),
            Compression::Zstd => Cow::Owned(OsString::from(format!("{}.zst", self.name))),
        }
    }
}
