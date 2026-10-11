use super::{
    BuildSetError, IDENTITY_LIMIT, MANIFEST_LIMIT, Manifest, PART_LIMIT, Part, REQUIRED, Result,
    SelectedSet, VisibleSet, compare_identity, content_parts, hex, parse_digest, parse_identity,
    role_limit, safe_relative,
};
use df_types::BuildIdentity;
use sha2::{Digest, Sha256};
use std::{
    collections::BTreeSet,
    fs::{self, File, Metadata, OpenOptions},
    io::{self, Read, Write},
    os::unix::fs::{DirBuilderExt, MetadataExt, OpenOptionsExt, PermissionsExt},
    path::{Path, PathBuf},
    sync::atomic::{AtomicU64, Ordering},
};

const REFERENCE_LIMIT: usize = 128;
const TREE_LIMIT: usize = PART_LIMIT * 8 + 4;
static NEXT: AtomicU64 = AtomicU64::new(0);

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(super) enum Phase {
    FileCreated(usize),
    FileWritten(usize),
    FileSynced(usize),
    ManifestWritten,
    ManifestSynced,
    SetPlaced,
    BeforeReferenceRename,
    AfterReferenceRename,
    AfterReferenceSync,
}

fn unique(prefix: &str) -> String {
    format!(
        ".{prefix}-{}-{}",
        std::process::id(),
        NEXT.fetch_add(1, Ordering::Relaxed)
    )
}

fn same(expected: &Metadata, observed: &Metadata) -> bool {
    expected.dev() == observed.dev()
        && expected.ino() == observed.ino()
        && expected.uid() == observed.uid()
        && expected.mode() == observed.mode()
        && expected.is_file() == observed.is_file()
        && expected.is_dir() == observed.is_dir()
        && !observed.file_type().is_symlink()
        && (!expected.is_file()
            || (expected.len() == observed.len()
                && observed.nlink() == 1
                && expected.mtime() == observed.mtime()
                && expected.mtime_nsec() == observed.mtime_nsec()
                && expected.ctime() == observed.ctime()
                && expected.ctime_nsec() == observed.ctime_nsec()))
}

fn canonical_directory(path: &Path, mode: u32) -> Result<Metadata> {
    if !path.is_absolute() || path.as_os_str().len() > 4096 || fs::canonicalize(path)? != path {
        return Err(BuildSetError::UnsafePath);
    }
    let metadata = fs::symlink_metadata(path)?;
    if !metadata.is_dir() || metadata.file_type().is_symlink() || metadata.mode() & 0o777 != mode {
        return Err(BuildSetError::UnsafePath);
    }
    Ok(metadata)
}

fn regular(path: &Path, maximum: u64) -> Result<Metadata> {
    if !path.is_absolute() || path.as_os_str().len() > 4096 || fs::canonicalize(path)? != path {
        return Err(BuildSetError::UnsafePath);
    }
    let metadata = fs::symlink_metadata(path)?;
    if !metadata.is_file() || metadata.file_type().is_symlink() || metadata.nlink() != 1 {
        return Err(BuildSetError::UnsafePath);
    }
    if metadata.len() > maximum {
        return Err(BuildSetError::Limit);
    }
    Ok(metadata)
}

fn open_regular(path: &Path, maximum: u64) -> Result<(File, Metadata)> {
    let before = regular(path, maximum)?;
    let file = File::open(path)?;
    if !same(&before, &file.metadata()?) || !same(&before, &fs::symlink_metadata(path)?) {
        return Err(BuildSetError::OwnershipChanged);
    }
    Ok((file, before))
}

fn finish_read(path: &Path, file: &File, before: &Metadata) -> Result<()> {
    if !same(before, &file.metadata()?) || !same(before, &fs::symlink_metadata(path)?) {
        return Err(BuildSetError::OwnershipChanged);
    }
    Ok(())
}

pub(super) fn read_external(path: &Path, maximum: usize) -> Result<Vec<u8>> {
    let (mut file, before) = open_regular(path, maximum as u64)?;
    let mut bytes = Vec::new();
    (&mut file)
        .take(maximum as u64 + 1)
        .read_to_end(&mut bytes)?;
    if bytes.len() > maximum {
        return Err(BuildSetError::Limit);
    }
    finish_read(path, &file, &before)?;
    Ok(bytes)
}

pub(super) fn observe_file(path: &Path, maximum: u64) -> Result<(u64, [u8; 32])> {
    let (mut file, before) = open_regular(path, maximum)?;
    let mut hash = Sha256::new();
    let mut length = 0_u64;
    let mut buffer = [0; 16 * 1024];
    loop {
        let read = file.read(&mut buffer)?;
        if read == 0 {
            break;
        }
        length = length
            .checked_add(read as u64)
            .ok_or(BuildSetError::Limit)?;
        if length > maximum {
            return Err(BuildSetError::Limit);
        }
        hash.update(&buffer[..read]);
    }
    finish_read(path, &file, &before)?;
    if length != before.len() {
        return Err(BuildSetError::DigestMismatch);
    }
    Ok((length, hash.finalize().into()))
}

fn inside(root: &Path, relative: &str) -> Result<PathBuf> {
    safe_relative(relative)?;
    let root_metadata = fs::symlink_metadata(root)?;
    let mut path = root.to_path_buf();
    for component in Path::new(relative).components() {
        path.push(component.as_os_str());
        let metadata = fs::symlink_metadata(&path)?;
        if metadata.file_type().is_symlink()
            || metadata.dev() != root_metadata.dev()
            || metadata.uid() != root_metadata.uid()
            || (!metadata.is_file() && !metadata.is_dir())
            || (metadata.is_file() && metadata.nlink() != 1)
        {
            return Err(BuildSetError::UnsafePath);
        }
    }
    Ok(path)
}

fn tree(root: &Path, files: &BTreeSet<String>, immutable: bool) -> Result<()> {
    let root_metadata = fs::symlink_metadata(root)?;
    let mut directories = BTreeSet::from([String::from("web/assets")]);
    for name in files {
        safe_relative(name)?;
        let mut parent = Path::new(name).parent();
        while let Some(path) = parent.filter(|p| !p.as_os_str().is_empty()) {
            directories.insert(path.to_str().ok_or(BuildSetError::UnsafePath)?.into());
            parent = path.parent();
        }
    }
    let mut pending = vec![root.to_path_buf()];
    let mut observed = BTreeSet::new();
    let mut count = 0;
    while let Some(directory) = pending.pop() {
        for entry in fs::read_dir(directory)? {
            count += 1;
            if count > TREE_LIMIT {
                return Err(BuildSetError::Limit);
            }
            let path = entry?.path();
            let relative = path
                .strip_prefix(root)
                .map_err(|_| BuildSetError::UnsafePath)?
                .to_str()
                .ok_or(BuildSetError::UnsafePath)?
                .to_owned();
            safe_relative(&relative)?;
            let metadata = fs::symlink_metadata(&path)?;
            if metadata.file_type().is_symlink()
                || metadata.dev() != root_metadata.dev()
                || metadata.uid() != root_metadata.uid()
            {
                return Err(BuildSetError::UnsafePath);
            }
            if metadata.is_dir() {
                if !directories.contains(&relative)
                    || (immutable && metadata.mode() & 0o777 != 0o500)
                {
                    return Err(BuildSetError::ComponentClosure);
                }
                pending.push(path);
            } else if metadata.is_file() && metadata.nlink() == 1 {
                if !files.contains(&relative)
                    || (immutable
                        && metadata.mode() & 0o777
                            != if relative == "native/df-transport-fixture" {
                                0o500
                            } else {
                                0o400
                            })
                {
                    return Err(BuildSetError::ComponentClosure);
                }
                observed.insert(relative);
            } else {
                return Err(BuildSetError::UnsafePath);
            }
        }
    }
    if &observed != files {
        return Err(BuildSetError::ComponentClosure);
    }
    Ok(())
}

fn verify_content(root: &Path, manifest: &Manifest) -> Result<()> {
    let bytes = read_external(&inside(root, "web/content-manifest.txt")?, MANIFEST_LIMIT)?;
    let expected = content_parts(&bytes, &manifest.identity)?;
    let observed: Vec<_> = manifest
        .parts
        .iter()
        .filter(|p| p.role.starts_with("asset:"))
        .cloned()
        .collect();
    if expected != observed {
        return Err(BuildSetError::ComponentClosure);
    }
    Ok(())
}

fn verify_parts(root: &Path, manifest: &Manifest, immutable: bool) -> Result<()> {
    let mut files: BTreeSet<_> = manifest.parts.iter().map(|p| p.path.clone()).collect();
    files.insert(
        if immutable {
            "manifest.txt"
        } else {
            "identity.txt"
        }
        .into(),
    );
    tree(root, &files, immutable)?;
    for part in &manifest.parts {
        if observe_file(&inside(root, &part.path)?, role_limit(part)?)? != (part.len, part.digest) {
            return Err(BuildSetError::DigestMismatch);
        }
    }
    verify_content(root, manifest)?;
    Ok(())
}

pub(super) fn seal(input: &Path, expected: &BuildIdentity) -> Result<Manifest> {
    let before = canonical_directory(input, 0o700)?;
    let observed = parse_identity(&read_external(
        &inside(input, "identity.txt")?,
        IDENTITY_LIMIT,
    )?)?;
    compare_identity(expected, &observed)?;
    let mut parts = Vec::with_capacity(REQUIRED.len());
    for (role, path, maximum) in REQUIRED {
        let (len, digest) = observe_file(&inside(input, path)?, maximum)?;
        parts.push(Part {
            role: role.into(),
            path: path.into(),
            len,
            digest,
        });
    }
    parts.extend(content_parts(
        &read_external(&inside(input, "web/content-manifest.txt")?, MANIFEST_LIMIT)?,
        expected,
    )?);
    let manifest = Manifest {
        identity: expected.clone(),
        parts,
    };
    manifest.validate()?;
    verify_parts(input, &manifest, false)?;
    if !same(&before, &fs::symlink_metadata(input)?) {
        return Err(BuildSetError::OwnershipChanged);
    }
    Ok(manifest)
}

pub(super) fn write_sealed(path: &Path, bytes: &[u8]) -> Result<()> {
    if !path.is_absolute() || path.as_os_str().len() > 4096 || bytes.len() > MANIFEST_LIMIT {
        return Err(BuildSetError::UnsafePath);
    }
    let parent = path.parent().ok_or(BuildSetError::UnsafePath)?;
    canonical_directory(parent, 0o700)?;
    let mut file = OpenOptions::new()
        .write(true)
        .create_new(true)
        .mode(0o600)
        .open(path)?;
    file.write_all(bytes)?;
    file.set_permissions(fs::Permissions::from_mode(0o400))?;
    file.sync_all()?;
    File::open(parent)?.sync_all()?;
    Ok(())
}

struct Root {
    path: PathBuf,
    identity: Metadata,
}

impl Root {
    fn open(path: &Path) -> Result<Self> {
        Ok(Self {
            path: path.into(),
            identity: canonical_directory(path, 0o700)?,
        })
    }
    fn check(&self) -> Result<()> {
        if !same(&self.identity, &fs::symlink_metadata(&self.path)?)
            || fs::canonicalize(&self.path)? != self.path
        {
            return Err(BuildSetError::OwnershipChanged);
        }
        Ok(())
    }
}

pub(super) struct Publisher {
    root: Root,
    lock: File,
    lock_identity: Metadata,
    sets_identity: Metadata,
}

impl Publisher {
    pub(super) fn acquire(path: &Path) -> Result<Self> {
        if !path.is_absolute() || path.as_os_str().len() > 4096 {
            return Err(BuildSetError::UnsafePath);
        }
        match fs::DirBuilder::new().mode(0o700).create(path) {
            Ok(()) => (),
            Err(e) if e.kind() == io::ErrorKind::AlreadyExists => (),
            Err(e) => return Err(e.into()),
        }
        let root = Root::open(path)?;
        let lock_path = path.join("publisher.lock");
        let lock = match OpenOptions::new()
            .read(true)
            .write(true)
            .create_new(true)
            .mode(0o600)
            .open(&lock_path)
        {
            Ok(file) => file,
            Err(error) if error.kind() == io::ErrorKind::AlreadyExists => {
                let before = regular(&lock_path, 0)?;
                let file = OpenOptions::new().read(true).write(true).open(&lock_path)?;
                if !same(&before, &file.metadata()?) {
                    return Err(BuildSetError::OwnershipChanged);
                }
                file
            }
            Err(error) => return Err(error.into()),
        };
        let lock_identity = lock.metadata()?;
        if lock_identity.uid() != root.identity.uid()
            || lock_identity.mode() & 0o777 != 0o600
            || !lock_identity.is_file()
            || lock_identity.nlink() != 1
            || !same(&lock_identity, &fs::symlink_metadata(&lock_path)?)
        {
            return Err(BuildSetError::OwnershipChanged);
        }
        lock.try_lock().map_err(|_| BuildSetError::PublisherBusy)?;
        let sets = path.join("sets");
        match fs::DirBuilder::new().mode(0o700).create(&sets) {
            Ok(()) => File::open(path)?.sync_all()?,
            Err(e) if e.kind() == io::ErrorKind::AlreadyExists => (),
            Err(e) => return Err(e.into()),
        }
        let sets_identity = canonical_directory(&sets, 0o700)?;
        if sets_identity.uid() != root.identity.uid() || sets_identity.dev() != root.identity.dev()
        {
            return Err(BuildSetError::OwnershipChanged);
        }
        let owner = Self {
            root,
            lock,
            lock_identity,
            sets_identity,
        };
        owner.check()?;
        Ok(owner)
    }

    fn check(&self) -> Result<()> {
        self.root.check()?;
        if !same(&self.lock_identity, &self.lock.metadata()?)
            || !same(
                &self.lock_identity,
                &fs::symlink_metadata(self.root.path.join("publisher.lock"))?,
            )
            || !same(
                &self.sets_identity,
                &fs::symlink_metadata(self.root.path.join("sets"))?,
            )
        {
            return Err(BuildSetError::OwnershipChanged);
        }
        Ok(())
    }

    pub(super) fn publish(
        &self,
        input: &Path,
        bytes: &[u8],
        step: &mut dyn FnMut(Phase) -> Result<()>,
    ) -> Result<[u8; 32]> {
        self.check()?;
        let input_identity = canonical_directory(input, 0o700)?;
        if input.starts_with(&self.root.path) || self.root.path.starts_with(input) {
            return Err(BuildSetError::UnsafePath);
        }
        let manifest = Manifest::parse(bytes)?;
        let observed = parse_identity(&read_external(
            &inside(input, "identity.txt")?,
            IDENTITY_LIMIT,
        )?)?;
        compare_identity(&manifest.identity, &observed)?;
        verify_parts(input, &manifest, false)?;
        let previous = current_record(&self.root.path)?;
        // A malformed or incomplete current selection is an explicit recovery problem;
        // publication never masks it by choosing a newer directory.
        if previous.is_some() {
            resolve(&self.root.path)?;
        }
        let id: [u8; 32] = Sha256::digest(bytes).into();
        let destination = self.root.path.join("sets").join(hex(&id));
        if destination.exists() {
            select(&self.root.path, id)?;
        } else {
            let staging = self.root.path.join(unique("stage"));
            fs::DirBuilder::new().mode(0o700).create(&staging)?;
            for (index, part) in manifest.parts.iter().enumerate() {
                copy_part(input, &staging, part, index, step)?;
            }
            let path = staging.join("manifest.txt");
            let mut file = OpenOptions::new()
                .write(true)
                .create_new(true)
                .mode(0o600)
                .open(path)?;
            file.write_all(bytes)?;
            step(Phase::ManifestWritten)?;
            file.set_permissions(fs::Permissions::from_mode(0o400))?;
            file.sync_all()?;
            step(Phase::ManifestSynced)?;
            freeze_directories(&staging)?;
            if !same(&input_identity, &fs::symlink_metadata(input)?) {
                return Err(BuildSetError::OwnershipChanged);
            }
            self.check()?;
            // Darwin requires the moved directory to remain writable for rename.
            // Only this held publication owner writes sets. The target is never replaced.
            fs::rename(&staging, &destination)?;
            fs::set_permissions(&destination, fs::Permissions::from_mode(0o500))?;
            File::open(&destination)?.sync_all()?;
            verify_parts(&destination, &manifest, true)?;
            File::open(self.root.path.join("sets"))?.sync_all()?;
            step(Phase::SetPlaced)?;
        }
        let path = self.root.path.join(unique("current"));
        let mut file = OpenOptions::new()
            .write(true)
            .create_new(true)
            .mode(0o600)
            .open(&path)?;
        file.write_all(format!("DF-CURRENT-BUILD-V1\n{}\n", hex(&id)).as_bytes())?;
        file.set_permissions(fs::Permissions::from_mode(0o400))?;
        file.sync_all()?;
        self.check()?;
        if !reference_unchanged(&previous, &current_record(&self.root.path)?) {
            return Err(BuildSetError::OwnershipChanged);
        }
        step(Phase::BeforeReferenceRename)?;
        self.check()?;
        if !reference_unchanged(&previous, &current_record(&self.root.path)?) {
            return Err(BuildSetError::OwnershipChanged);
        }
        // This rename is the sole visibility point. Nothing below it reports rejection
        // or attempts rollback without classifying the complete observed reference.
        if fs::rename(path, self.root.path.join("current")).is_err() {
            return Err(self.after_visibility(id, &previous));
        }
        let completed: Result<()> = (|| {
            step(Phase::AfterReferenceRename)?;
            File::open(&self.root.path)?.sync_all()?;
            step(Phase::AfterReferenceSync)?;
            self.check()?;
            Ok(())
        })();
        if completed.is_err() {
            return Err(self.after_visibility(id, &previous));
        }
        Ok(id)
    }

    fn after_visibility(&self, candidate: [u8; 32], previous: &Option<Reference>) -> BuildSetError {
        let previous_id = previous
            .as_ref()
            .and_then(|record| reference_id(&record.bytes).ok());
        let visible = match resolve(&self.root.path) {
            Ok(selected) if selected.id == candidate => VisibleSet::Candidate,
            Ok(selected) if Some(selected.id) == previous_id => VisibleSet::Previous,
            Ok(_) => VisibleSet::OtherComplete,
            Err(_) => VisibleSet::Unavailable,
        };
        BuildSetError::AfterVisibility(visible)
    }
}

fn copy_part(
    input: &Path,
    staging: &Path,
    part: &Part,
    index: usize,
    step: &mut dyn FnMut(Phase) -> Result<()>,
) -> Result<()> {
    let source = inside(input, &part.path)?;
    let (mut reader, before) = open_regular(&source, role_limit(part)?)?;
    let target = staging.join(&part.path);
    let parent = target.parent().ok_or(BuildSetError::UnsafePath)?;
    fs::DirBuilder::new()
        .recursive(true)
        .mode(0o700)
        .create(parent)?;
    let mut writer = OpenOptions::new()
        .write(true)
        .create_new(true)
        .mode(0o600)
        .open(target)?;
    step(Phase::FileCreated(index))?;
    let mut hash = Sha256::new();
    let mut len = 0_u64;
    let mut buffer = [0; 16 * 1024];
    loop {
        let read = reader.read(&mut buffer)?;
        if read == 0 {
            break;
        }
        len += read as u64;
        if len > part.len {
            return Err(BuildSetError::DigestMismatch);
        }
        writer.write_all(&buffer[..read])?;
        hash.update(&buffer[..read]);
    }
    finish_read(&source, &reader, &before)?;
    let digest: [u8; 32] = hash.finalize().into();
    if len != part.len || digest != part.digest {
        return Err(BuildSetError::DigestMismatch);
    }
    step(Phase::FileWritten(index))?;
    writer.set_permissions(fs::Permissions::from_mode(if part.role == "native" {
        0o500
    } else {
        0o400
    }))?;
    writer.sync_all()?;
    step(Phase::FileSynced(index))?;
    Ok(())
}

fn freeze_directories(root: &Path) -> Result<()> {
    let mut pending = vec![root.to_path_buf()];
    let mut directories = Vec::new();
    while let Some(directory) = pending.pop() {
        if directories.len() > TREE_LIMIT {
            return Err(BuildSetError::Limit);
        }
        for entry in fs::read_dir(&directory)? {
            let path = entry?.path();
            if fs::symlink_metadata(&path)?.is_dir() {
                pending.push(path);
            }
        }
        directories.push(directory);
    }
    for path in directories.into_iter().rev() {
        if path != root {
            fs::set_permissions(&path, fs::Permissions::from_mode(0o500))?;
        }
        File::open(path)?.sync_all()?;
    }
    Ok(())
}

struct Reference {
    bytes: Vec<u8>,
    identity: Metadata,
}

fn reference_unchanged(expected: &Option<Reference>, observed: &Option<Reference>) -> bool {
    match (expected, observed) {
        (None, None) => true,
        (Some(a), Some(b)) => a.bytes == b.bytes && same(&a.identity, &b.identity),
        _ => false,
    }
}

fn current_record(root: &Path) -> Result<Option<Reference>> {
    let path = root.join("current");
    match fs::symlink_metadata(&path) {
        Ok(_) => {
            let (mut file, identity) = open_regular(&path, REFERENCE_LIMIT as u64)?;
            let root_identity = fs::symlink_metadata(root)?;
            if identity.uid() != root_identity.uid()
                || identity.dev() != root_identity.dev()
                || identity.mode() & 0o777 != 0o400
            {
                return Err(BuildSetError::OwnershipChanged);
            }
            let mut bytes = Vec::new();
            (&mut file)
                .take(REFERENCE_LIMIT as u64 + 1)
                .read_to_end(&mut bytes)?;
            if bytes.len() > REFERENCE_LIMIT {
                return Err(BuildSetError::Limit);
            }
            finish_read(&path, &file, &identity)?;
            Ok(Some(Reference { bytes, identity }))
        }
        Err(error) if error.kind() == io::ErrorKind::NotFound => Ok(None),
        Err(error) => Err(error.into()),
    }
}

pub(super) fn resolve(root: &Path) -> Result<SelectedSet> {
    let owner = Root::open(root)?;
    let record = current_record(root)?.ok_or(BuildSetError::InvalidFormat)?;
    let id = reference_id(&record.bytes)?;
    let selected = select(root, id)?;
    owner.check()?;
    Ok(selected)
}

fn reference_id(bytes: &[u8]) -> Result<[u8; 32]> {
    let mut lines = super::text(bytes, REFERENCE_LIMIT)?.lines();
    if lines.next() != Some("DF-CURRENT-BUILD-V1") {
        return Err(BuildSetError::InvalidFormat);
    }
    let id = parse_digest(lines.next().ok_or(BuildSetError::InvalidFormat)?)?;
    if lines.next().is_some() {
        return Err(BuildSetError::InvalidFormat);
    }
    Ok(id)
}

pub(super) fn select(root: &Path, id: [u8; 32]) -> Result<SelectedSet> {
    let owner = Root::open(root)?;
    let sets = root.join("sets");
    let sets_metadata = canonical_directory(&sets, 0o700)?;
    if sets_metadata.dev() != owner.identity.dev() || sets_metadata.uid() != owner.identity.uid() {
        return Err(BuildSetError::UnsafePath);
    }
    let directory = sets.join(hex(&id));
    let directory_identity = canonical_directory(&directory, 0o500)?;
    if directory_identity.dev() != owner.identity.dev()
        || directory_identity.uid() != owner.identity.uid()
    {
        return Err(BuildSetError::UnsafePath);
    }
    let bytes = read_external(&directory.join("manifest.txt"), MANIFEST_LIMIT)?;
    let observed: [u8; 32] = Sha256::digest(&bytes).into();
    if observed != id {
        return Err(BuildSetError::DigestMismatch);
    }
    let manifest = Manifest::parse(&bytes)?;
    verify_parts(&directory, &manifest, true)?;
    owner.check()?;
    if !same(&directory_identity, &fs::symlink_metadata(&directory)?) {
        return Err(BuildSetError::OwnershipChanged);
    }
    Ok(SelectedSet {
        root: root.into(),
        id,
        directory,
        manifest,
        manifest_bytes: bytes,
    })
}

pub(super) fn read_checked(root: &Path, part: &Part) -> Result<Vec<u8>> {
    let bytes = read_external(&inside(root, &part.path)?, role_limit(part)? as usize)?;
    let observed: [u8; 32] = Sha256::digest(&bytes).into();
    if bytes.len() as u64 != part.len || observed != part.digest {
        return Err(BuildSetError::DigestMismatch);
    }
    Ok(bytes)
}
