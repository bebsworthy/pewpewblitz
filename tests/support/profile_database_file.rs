//! A test-owned `SQLite` profile database path, fresh before use and removed afterwards.
//!
//! Test paths are keyed by process ID, and macOS recycles process IDs. A file left by an earlier
//! run (an older schema, or another test's accounts and maps) would otherwise be reopened: an old
//! schema panics the lobby at startup, and old rows change publishing and queue outcomes
//! (BRL-1chc, BRL-shwv). Shared by the network harness and the routed-process tests.

pub(crate) struct ProfileDatabaseFile(std::path::PathBuf);

impl ProfileDatabaseFile {
    /// Removes any existing database and its `-wal`/`-shm` files, panicking with the path if one
    /// exists but cannot be removed.
    pub(crate) fn fresh(path: std::path::PathBuf) -> Self {
        let file = Self(path);
        for path in file.files() {
            match std::fs::remove_file(&path) {
                Ok(()) => {}
                Err(error) if error.kind() == std::io::ErrorKind::NotFound => {}
                Err(error) => panic!("remove stale profile database {}: {error}", path.display()),
            }
        }
        file
    }

    pub(crate) fn path(&self) -> String {
        self.0.to_string_lossy().into_owned()
    }

    /// The database and its WAL-mode side files.
    fn files(&self) -> impl Iterator<Item = std::path::PathBuf> + '_ {
        ["", "-wal", "-shm"].into_iter().map(|suffix| {
            let mut path = self.0.clone().into_os_string();
            path.push(suffix);
            std::path::PathBuf::from(path)
        })
    }
}

impl Drop for ProfileDatabaseFile {
    fn drop(&mut self) {
        // Owners drop this after the process or app holding the connection has closed it.
        // Never panic here: this also runs while a failed test unwinds.
        for path in self.files() {
            let _ = std::fs::remove_file(path);
        }
    }
}
