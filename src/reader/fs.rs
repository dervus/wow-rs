use std::path::{Path, PathBuf};
use std::io::{self, BufReader};
use std::fs::{self, File};
use reader::{ResourceReader, ResourceHandle};

#[allow(dead_code)]
pub struct FsResourceReader {
    path: PathBuf
}

impl FsResourceReader {
    #[allow(dead_code)]
    pub fn new(path: &Path) -> FsResourceReader {
        FsResourceReader { path: path.to_owned() }
    }
}

impl ResourceReader for FsResourceReader {
    fn open(&self, name: &str) -> io::Result<Box<ResourceHandle>> {
        let target = find_resource_path(&self.path, name)?;
        let file = File::open(target)?;
        let reader = BufReader::new(file);
        Ok(Box::new(reader))
    }
}

fn find_resource_path(dir: &Path, target: &str) -> io::Result<PathBuf> {
    let mut path = dir.to_owned();

    for component in target.split(|c| c == '/' || c == '\\') {
        path = find_dir_entry(&path, &component)?;
    }

    Ok(path)
}

fn find_dir_entry(dir: &Path, target: &str) -> io::Result<PathBuf> {
    let target_lowercase = target.to_lowercase();

    for entry_result in fs::read_dir(dir)? {
        let entry = entry_result?;

        let mut candidate_lowercase = entry.file_name()
            .into_string()
            .map(|s| s.to_lowercase())
            .map_err(|_| io_error!(Other, "unable to decode OsString"))?;

        if candidate_lowercase == target_lowercase {
            return Ok(entry.path());
        }
    }

    Err(io_error!(NotFound, "directory {:?} does not contain file {:?}", dir, target))
}