use sha2::{Sha256, Digest};

use std::{
    env, fs,
    io::{self, Write},
    path::{Path, PathBuf},
};

const SOURCE_DIR: &str = "../data";

fn main() {
    let source_dir = Path::new(SOURCE_DIR);
    rerun_if_changed(&source_dir).unwrap();
    write_hashes(&source_dir).unwrap();
}

fn rerun_if_changed(dir: &Path) -> io::Result<()> {
    println!("cargo:rerun-if-changed={}", dir.to_str().unwrap());

    if dir.is_dir() {
        for entry in fs::read_dir(dir)? {
            let entry = entry?;
            let path = entry.path();
            if path.is_dir() {
                rerun_if_changed(&path)?;
            }
        }
    }

    Ok(())
}

fn write_hashes(dir: &Path) -> io::Result<()> {
    let dest_path = Path::new(&env::var_os("OUT_DIR").expect("OUT_DIR not set")).join("hashes.rs");
    let mut test_file_name = PathBuf::new();
    let mut dest = fs::File::create(dest_path)?;
    dest.write(b"const FILE_HASHES: &[(&str, &str)] = &[\n")?;

    write_hashes_inner(dir, &mut test_file_name, &mut dest)?;

    dest.write(b"];\n")?;
    dest.flush()?;

    Ok(())
}

fn write_hashes_inner(
    source: &Path,
    test_file_name: &mut PathBuf,
    dest: &mut fs::File,
) -> io::Result<()> {
    if source.is_dir() {
        for entry in fs::read_dir(source)? {
            let entry = entry?;
            let path = entry.path();
            *test_file_name = test_file_name.join(path.file_name().unwrap());
            write_hashes_inner(&path, test_file_name, dest)?;
            test_file_name.pop();
        }
    } else if source.is_file() {
        let mut file = fs::File::open(source)?;
        let mut hasher = Sha256::new();
        io::copy(&mut file, &mut hasher)?;
        let hash = hasher.result();
        writeln!(
            dest,
            "  (\"{}\", \"{:x}\"),",
            test_file_name.as_path().display(),
            hash
        )?;
    }
    Ok(())
}
