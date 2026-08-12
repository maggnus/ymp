use sha2::{Digest, Sha256};
use std::fs::{self, File, OpenOptions};
use std::io::{Read, Write};
use std::path::{Path, PathBuf};
use thiserror::Error;
use uuid::Uuid;

#[derive(Debug, Error)]
pub enum ObjectStoreError {
    #[error("object-store I/O error: {0}")]
    Io(#[from] std::io::Error),
    #[error("invalid SHA-256 object digest: {0}")]
    InvalidDigest(String),
    #[error("object is unavailable: {0}")]
    Missing(String),
    #[error("object bytes do not match their digest: {0}")]
    DigestMismatch(String),
}

#[derive(Clone, Debug)]
pub struct ObjectStore {
    root: PathBuf,
}

impl ObjectStore {
    pub fn open(root: impl AsRef<Path>) -> Result<Self, ObjectStoreError> {
        let root = root.as_ref().to_path_buf();
        fs::create_dir_all(&root)?;
        Ok(Self { root })
    }

    pub fn put(&self, bytes: &[u8]) -> Result<String, ObjectStoreError> {
        let digest = hex::encode(Sha256::digest(bytes));
        let target = self.path_for(&digest)?;
        if target.exists() {
            self.verify(&digest)?;
            return Ok(digest);
        }

        let parent = target.parent().expect("object path always has a parent");
        fs::create_dir_all(parent)?;
        let temporary = parent.join(format!(".{}.tmp", Uuid::new_v4()));
        let mut file = OpenOptions::new()
            .create_new(true)
            .write(true)
            .open(&temporary)?;
        file.write_all(bytes)?;
        file.flush()?;
        file.sync_all()?;
        fs::rename(&temporary, &target)?;
        File::open(parent)?.sync_all()?;
        Ok(digest)
    }

    pub fn read(&self, digest: &str) -> Result<Vec<u8>, ObjectStoreError> {
        let path = self.path_for(digest)?;
        if !path.is_file() {
            return Err(ObjectStoreError::Missing(digest.to_owned()));
        }
        let mut bytes = Vec::new();
        File::open(path)?.read_to_end(&mut bytes)?;
        let actual = hex::encode(Sha256::digest(&bytes));
        if actual != digest {
            return Err(ObjectStoreError::DigestMismatch(digest.to_owned()));
        }
        Ok(bytes)
    }

    pub fn verify(&self, digest: &str) -> Result<(), ObjectStoreError> {
        self.read(digest).map(|_| ())
    }

    pub fn path_for(&self, digest: &str) -> Result<PathBuf, ObjectStoreError> {
        if digest.len() != 64
            || !digest
                .bytes()
                .all(|byte| byte.is_ascii_digit() || (b'a'..=b'f').contains(&byte))
        {
            return Err(ObjectStoreError::InvalidDigest(digest.to_owned()));
        }
        Ok(self.root.join(&digest[..2]).join(&digest[2..]))
    }

    pub fn root(&self) -> &Path {
        &self.root
    }
}
