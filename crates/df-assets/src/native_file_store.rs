use crate::{
    AssetManifest, AssetReadStore, AssetStore, DurableObject, PublicationError, PublishedBinding,
    StoreError,
};
use df_types::OperationId;
use sha2::{Digest, Sha256};
use std::fs::{self, File, OpenOptions};
use std::io::{Read, Seek, SeekFrom, Write};
use std::path::{Path, PathBuf};

/// Native file-backed byte port. The caller configures a durable root outside caches.
pub struct NativeFileStore {
    root: PathBuf,
    max_object_bytes: u64,
}

/// Private staging handle. Its existence does not imply metadata publication.
pub struct StagedUpload {
    root: PathBuf,
    path: PathBuf,
    operation: OperationId,
}

impl NativeFileStore {
    pub fn new(root: impl AsRef<Path>, max_object_bytes: u64) -> Result<Self, StoreError> {
        fs::create_dir_all(root.as_ref())?;
        let root = root.as_ref().canonicalize()?;
        fs::create_dir_all(root.join("staging"))?;
        fs::create_dir_all(root.join("objects"))?;
        fs::create_dir_all(root.join("promotion"))?;
        File::open(&root)?.sync_all()?;
        Ok(Self {
            root,
            max_object_bytes,
        })
    }

    fn object_path(&self, digest: &[u8; 32]) -> PathBuf {
        self.root.join("objects").join(hex(digest))
    }

    /// Reopen the same private staging object after a lost acknowledgement or process restart.
    /// Publication still rechecks its complete bytes and metadata authority.
    pub fn resume_staged(&self, operation: OperationId) -> Result<StagedUpload, StoreError> {
        let path = self.root.join("staging").join(hex(operation.as_bytes()));
        File::open(&path)?;
        Ok(StagedUpload {
            root: self.root.clone(),
            path,
            operation,
        })
    }

    fn check_staged(&self, staged: &StagedUpload) -> Result<(), StoreError> {
        if staged.root != self.root {
            return Err(StoreError::StagingConflict);
        }
        Ok(())
    }

    /// Confirm that a metadata-bound object remains complete and matches its exact manifest.
    pub fn confirm(&self, object: DurableObject) -> Result<(), StoreError> {
        let path = self.object_path(object.digest());
        verify_file(
            &path,
            AssetManifest {
                byte_len: object.byte_len(),
                sha256: *object.digest(),
            },
        )
    }
}

impl AssetStore for NativeFileStore {
    type Staged = StagedUpload;

    fn check_expected(&self, expected: AssetManifest) -> Result<(), StoreError> {
        if expected.byte_len > self.max_object_bytes {
            return Err(StoreError::Capacity);
        }
        Ok(())
    }

    fn stage(
        &self,
        operation: OperationId,
        reader: &mut dyn Read,
    ) -> Result<StagedUpload, StoreError> {
        let path = self.root.join("staging").join(hex(operation.as_bytes()));
        let mut file = OpenOptions::new()
            .write(true)
            .create_new(true)
            .open(&path)
            .map_err(|error| {
                if error.kind() == std::io::ErrorKind::AlreadyExists {
                    StoreError::StagingConflict
                } else {
                    StoreError::Io(error)
                }
            })?;
        let staged_result = (|| {
            let mut length = 0_u64;
            let mut buffer = [0_u8; 65536];
            loop {
                let read = reader.read(&mut buffer)?;
                if read == 0 {
                    break;
                }
                length = length
                    .checked_add(u64::try_from(read).map_err(|_| StoreError::Capacity)?)
                    .ok_or(StoreError::Capacity)?;
                if length > self.max_object_bytes {
                    return Err(StoreError::Capacity);
                }
                file.write_all(&buffer[..read])?;
            }
            file.flush()?;
            file.sync_all()?;
            Ok(())
        })();
        if let Err(error) = staged_result {
            let _ = fs::remove_file(&path);
            return Err(error);
        }
        File::open(self.root.join("staging"))?.sync_all()?;
        Ok(StagedUpload {
            root: self.root.clone(),
            path,
            operation,
        })
    }

    fn staged_len(&self, staged: &StagedUpload) -> Result<u64, StoreError> {
        self.check_staged(staged)?;
        Ok(File::open(&staged.path)?.metadata()?.len())
    }

    fn staged_operation(&self, staged: &StagedUpload) -> Result<OperationId, StoreError> {
        self.check_staged(staged)?;
        Ok(staged.operation)
    }

    fn matches_published<M: Eq>(
        &self,
        staged: &StagedUpload,
        published: &PublishedBinding<M>,
    ) -> Result<bool, StoreError> {
        self.check_staged(staged)?;
        self.confirm(published.object)?;
        if published.object.byte_len() != published.bytes.byte_len
            || published.object.digest() != &published.bytes.sha256
        {
            return Err(StoreError::BackingIntegrity);
        }
        same_contents(&staged.path, &self.object_path(published.object.digest()))
    }

    fn verify_and_promote(
        &self,
        staged: &StagedUpload,
        expected: AssetManifest,
    ) -> Result<DurableObject, PublicationError> {
        self.check_staged(staged).map_err(PublicationError::Store)?;
        if expected.byte_len > self.max_object_bytes {
            return Err(PublicationError::Store(StoreError::Capacity));
        }
        let temporary = self
            .root
            .join("promotion")
            .join(hex(staged.operation.as_bytes()));
        let mut owns_temporary = false;
        let result = (|| {
            let mut input = File::open(&staged.path).map_err(StoreError::from)?;
            let mut output = OpenOptions::new()
                .write(true)
                .create_new(true)
                .open(&temporary)
                .map_err(StoreError::from)?;
            owns_temporary = true;
            let mut hasher = Sha256::new();
            let mut length = 0_u64;
            let mut buffer = [0_u8; 65536];
            loop {
                let read = input.read(&mut buffer).map_err(StoreError::from)?;
                if read == 0 {
                    break;
                }
                length = length
                    .checked_add(u64::try_from(read).map_err(|_| PublicationError::Incomplete)?)
                    .ok_or(PublicationError::Incomplete)?;
                if length > self.max_object_bytes {
                    return Err(PublicationError::Store(StoreError::Capacity));
                }
                if length > expected.byte_len {
                    return Err(PublicationError::Incomplete);
                }
                output
                    .write_all(&buffer[..read])
                    .map_err(StoreError::from)?;
                hasher.update(&buffer[..read]);
            }
            if length != expected.byte_len {
                return Err(PublicationError::Incomplete);
            }
            let digest: [u8; 32] = hasher.finalize().into();
            if digest != expected.sha256 {
                return Err(PublicationError::HashMismatch);
            }
            let mut permissions = output.metadata().map_err(StoreError::from)?.permissions();
            permissions.set_readonly(true);
            output
                .set_permissions(permissions)
                .map_err(StoreError::from)?;
            output.flush().map_err(StoreError::from)?;
            output.sync_all().map_err(StoreError::from)?;
            let destination = self.object_path(&expected.sha256);
            match fs::hard_link(&temporary, &destination) {
                Ok(()) => {
                    File::open(self.root.join("objects"))
                        .and_then(|dir| dir.sync_all())
                        .map_err(StoreError::from)?;
                }
                Err(error) if error.kind() == std::io::ErrorKind::AlreadyExists => {
                    verify_file(&destination, expected).map_err(PublicationError::Store)?;
                    if !same_contents(&temporary, &destination).map_err(PublicationError::Store)? {
                        return Err(PublicationError::Store(StoreError::BackingIntegrity));
                    }
                    File::open(&destination)
                        .and_then(|object| object.sync_all())
                        .map_err(StoreError::from)?;
                    File::open(self.root.join("objects"))
                        .and_then(|dir| dir.sync_all())
                        .map_err(StoreError::from)?;
                }
                Err(error) => return Err(PublicationError::Store(StoreError::Io(error))),
            }
            Ok(DurableObject::from_manifest(expected))
        })();
        if owns_temporary {
            let removed = fs::remove_file(&temporary);
            if result.is_ok() {
                removed.map_err(StoreError::from)?;
                File::open(self.root.join("promotion"))
                    .and_then(|dir| dir.sync_all())
                    .map_err(StoreError::from)?;
            }
        }
        result
    }
}

fn verify_file(path: &Path, expected: AssetManifest) -> Result<(), StoreError> {
    let mut file = File::open(path).map_err(|error| {
        if error.kind() == std::io::ErrorKind::NotFound {
            StoreError::BackingMissing
        } else {
            StoreError::Io(error)
        }
    })?;
    let mut hasher = Sha256::new();
    let mut length = 0_u64;
    let mut buffer = [0_u8; 65536];
    loop {
        let read = file.read(&mut buffer)?;
        if read == 0 {
            break;
        }
        length = length
            .checked_add(u64::try_from(read).map_err(|_| StoreError::BackingIntegrity)?)
            .ok_or(StoreError::BackingIntegrity)?;
        hasher.update(&buffer[..read]);
    }
    let digest: [u8; 32] = hasher.finalize().into();
    if length != expected.byte_len || digest != expected.sha256 {
        return Err(StoreError::BackingIntegrity);
    }
    Ok(())
}

fn same_contents(left: &Path, right: &Path) -> Result<bool, StoreError> {
    let mut left = File::open(left)?;
    let mut right = File::open(right)?;
    let mut remaining = left.metadata()?.len();
    if remaining != right.metadata()?.len() {
        return Ok(false);
    }
    let mut left_bytes = [0_u8; 65536];
    let mut right_bytes = [0_u8; 65536];
    while remaining != 0 {
        let length =
            usize::try_from(remaining.min(65536)).map_err(|_| StoreError::BackingIntegrity)?;
        left.read_exact(&mut left_bytes[..length])?;
        right.read_exact(&mut right_bytes[..length])?;
        if left_bytes[..length] != right_bytes[..length] {
            return Ok(false);
        }
        remaining -= u64::try_from(length).map_err(|_| StoreError::BackingIntegrity)?;
    }
    Ok(true)
}

fn hex(bytes: &[u8]) -> String {
    const DIGITS: &[u8; 16] = b"0123456789abcdef";
    let mut text = String::with_capacity(bytes.len() * 2);
    for byte in bytes {
        text.push(char::from(DIGITS[usize::from(byte >> 4)]));
        text.push(char::from(DIGITS[usize::from(byte & 0x0f)]));
    }
    text
}

impl AssetReadStore for NativeFileStore {
    type Reader = File;

    fn open_verified(
        &self,
        object: DurableObject,
        manifest: AssetManifest,
    ) -> Result<File, StoreError> {
        self.check_expected(manifest)?;
        if object.byte_len() != manifest.byte_len || object.digest() != &manifest.sha256 {
            return Err(StoreError::BackingIntegrity);
        }
        let mut file = File::open(self.object_path(object.digest())).map_err(|error| {
            if error.kind() == std::io::ErrorKind::NotFound {
                StoreError::BackingMissing
            } else {
                StoreError::Io(error)
            }
        })?;
        if file.metadata()?.len() != manifest.byte_len {
            return Err(StoreError::BackingIntegrity);
        }
        let mut hasher = Sha256::new();
        let mut length = 0_u64;
        let mut buffer = [0_u8; 65536];
        loop {
            let read = file.read(&mut buffer)?;
            if read == 0 {
                break;
            }
            length = length
                .checked_add(u64::try_from(read).map_err(|_| StoreError::BackingIntegrity)?)
                .ok_or(StoreError::BackingIntegrity)?;
            if length > manifest.byte_len {
                return Err(StoreError::BackingIntegrity);
            }
            hasher.update(&buffer[..read]);
        }
        let digest: [u8; 32] = hasher.finalize().into();
        if length != manifest.byte_len || digest != manifest.sha256 {
            return Err(StoreError::BackingIntegrity);
        }
        file.seek(SeekFrom::Start(0))?;
        Ok(file)
    }
}
