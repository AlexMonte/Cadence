//! Same-directory temporary writes and a final atomic rename.

use std::{
    io::Write,
    path::{Path, PathBuf},
    sync::atomic::{AtomicU64, Ordering},
};

static NEXT_TEMP_FILE: AtomicU64 = AtomicU64::new(0);

struct TemporaryFile(PathBuf);
impl Drop for TemporaryFile {
    fn drop(&mut self) {
        let _ = std::fs::remove_file(&self.0);
    }
}

pub(crate) fn atomic_write(path: &Path, bytes: &[u8]) -> Result<(), super::PersistenceError> {
    let directory = super::samples::project_directory(path);
    std::fs::create_dir_all(directory)?;
    let (temporary, mut file) = loop {
        let number = NEXT_TEMP_FILE.fetch_add(1, Ordering::Relaxed);
        let temporary = directory.join(format!(".musaic-save-{}-{number}.tmp", std::process::id()));
        match std::fs::OpenOptions::new()
            .write(true)
            .create_new(true)
            .open(&temporary)
        {
            Ok(file) => break (TemporaryFile(temporary), file),
            Err(error) if error.kind() == std::io::ErrorKind::AlreadyExists => continue,
            Err(error) => return Err(error.into()),
        }
    };
    file.write_all(bytes)?;
    file.sync_all()?;
    drop(file);
    std::fs::rename(&temporary.0, path)?;
    #[cfg(unix)]
    std::fs::File::open(directory)?.sync_all()?;
    Ok(())
}
