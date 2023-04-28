use std::fs::File;
use std::io;
use std::path::{Path, PathBuf};

pub(super) struct ProtectedFile {
    path: PathBuf,
    identity: Option<(u64, u64)>,
}

impl ProtectedFile {
    pub fn new(path: &Path, file: &File) -> io::Result<Self> {
        Ok(Self {
            path: std::fs::canonicalize(path)?,
            identity: identity(file)?,
        })
    }

    pub fn check_path(&self, path: &Path) -> io::Result<()> {
        match std::fs::canonicalize(path) {
            Ok(path) => self.check(&path, identity_at(&path)?),
            Err(error) if error.kind() == io::ErrorKind::NotFound => Ok(()),
            Err(error) => Err(error),
        }
    }

    pub fn check_file(&self, path: &Path, file: &File) -> io::Result<()> {
        self.check(&std::fs::canonicalize(path)?, identity(file)?)
    }

    fn check(&self, path: &Path, identity: Option<(u64, u64)>) -> io::Result<()> {
        if path == self.path || self.identity.is_some() && self.identity == identity {
            Err(io::Error::new(
                io::ErrorKind::InvalidInput,
                "manual recording must not overwrite the automatic recording",
            ))
        } else {
            Ok(())
        }
    }
}

#[cfg(unix)]
fn identity(file: &File) -> io::Result<Option<(u64, u64)>> {
    use std::os::unix::fs::MetadataExt;
    let metadata = file.metadata()?;
