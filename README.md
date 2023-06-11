# DICOM Test Files

[![dicom-test-files on crates.io](https://img.shields.io/crates/v/dicom-test-files.svg)](https://crates.io/crates/dicom-test-files)

This repository collects together example DICOM files from various sources.
The intention is that they can be used
for testing across many different libraries.

See the [documentation](https://docs.rs/dicom-test-files)
for instructions of use.

## Known limitations

The Rust functions cannot be used from doc-tests
as they are not executed from within the target directory.
