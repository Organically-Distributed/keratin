//! Portable directory durability helpers shared by Stroma storage and the
//! recovery inspection cache.
use keratin_log::util::fsync_dir;
use std::{fs, io, path::Path};

/// Sync existing metadata without changing its contents. Windows requires a
/// writable handle for FlushFileBuffers; Unix permits the existing read-only
/// open. Opening never creates a missing publication record.
pub(crate) fn sync_existing_file(path: impl AsRef<Path>) -> io::Result<()> {
    keratin_log::util::open_for_sync(path)?.sync_all()
}

#[cfg(test)]
mod tests {
    #[test]
    fn existing_metadata_sync_preserves_bytes_and_does_not_create_files() {
        let dir = keratin_log::test_dir!("metadata_sync");
        let path = dir.root.join("published");
        std::fs::write(&path, b"publication").unwrap();
        super::sync_existing_file(&path).unwrap();
        assert_eq!(std::fs::read(&path).unwrap(), b"publication");
        assert_eq!(
            super::sync_existing_file(dir.root.join("missing")).unwrap_err().kind(),
            std::io::ErrorKind::NotFound
        );
    }
}

// Files that mark the top of a storage tree: a Stroma root and a recovery
// cache namespace. Directory chains are synced up to the first of these.
fn is_tree_root(dir: &Path) -> bool {
    [
        crate::stroma::layout::MARKER,
        crate::recovery_cache::NAMESPACE_LOCK,
    ]
    .iter()
    .any(|marker| dir.join(marker).is_file())
}

/// Sync `path` and each ancestor up to the root of the storage tree that owns
/// it. Directories below the root may have been created without a durable
/// parent entry, and a retry also completes an earlier sync that failed. The
/// root's own entry was synced when the root was created, so nothing above it
/// is synced again. A path outside any storage tree syncs every ancestor.
pub(crate) fn sync_dir_chain(path: &Path) -> io::Result<()> {
    for ancestor in fs::canonicalize(path)?.ancestors() {
        fsync_dir(ancestor)?;
        if is_tree_root(ancestor) {
            break;
        }
    }
    Ok(())
}

/// Create `dir` and any missing parents, syncing the parent of each new
/// directory so it survives a crash. The parent of `dir` is synced even when
/// `dir` already exists, since a retry after a crash may find an entry that
/// was created but never made durable.
pub(crate) fn create_dir_all_synced(dir: &Path) -> io::Result<()> {
    let parent_of = |path: &Path| {
        path.parent()
            .filter(|p| !p.as_os_str().is_empty())
            .map(Path::to_path_buf)
    };
    let mut missing = Vec::new();
    let mut cursor = Some(dir.to_path_buf());
    while let Some(path) = cursor {
        if path.try_exists()? {
            break;
        }
        cursor = parent_of(&path);
        missing.push(path);
    }
    for created in missing.iter().rev() {
        match fs::create_dir(created) {
            Ok(()) => {}
            Err(e) if e.kind() == io::ErrorKind::AlreadyExists && created.is_dir() => {}
            Err(e) => return Err(e),
        }
        if let Some(parent) = parent_of(created) {
            fsync_dir(&parent)?;
        }
    }
    if missing.is_empty()
        && let Some(parent) = parent_of(dir)
    {
        fsync_dir(&parent)?;
    }
    Ok(())
}
