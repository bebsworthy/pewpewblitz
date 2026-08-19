//! Versioned bounded persistence for preferred names, favorites, and successful joins.

use super::parse_server_address;
use atomic_write_file::AtomicWriteFile;
use bevy::prelude::Resource;
use directories::ProjectDirs;
use serde::{Deserialize, Serialize};
use std::{
    fs,
    io::{Read as _, Write as _},
    path::{Path, PathBuf},
};

const CONNECTIONS_SCHEMA_VERSION: u16 = 1;
const MAX_CONNECTIONS_BYTES: u64 = 64 * 1024;
pub const MAX_SAVED_SERVERS: usize = 16;

#[derive(Resource, Clone, Debug, PartialEq, Eq)]
pub struct ClientConnectionsPath(pub PathBuf);

impl Default for ClientConnectionsPath {
    fn default() -> Self {
        let path = ProjectDirs::from("com", "Brawler", "Brawler").map_or_else(
            || PathBuf::from("connections.ron"),
            |dirs| dirs.config_dir().join("connections.ron"),
        );
        Self(path)
    }
}

#[derive(Resource, Serialize, Deserialize, Clone, Debug, Default, PartialEq, Eq)]
#[serde(deny_unknown_fields)]
pub struct ConnectionsFileV1 {
    pub schema_version: u16,
    pub preferred_display_name: Option<String>,
    pub favorites: Vec<SavedServerV1>,
    pub recents: Vec<RecentServerV1>,
}

#[derive(Serialize, Deserialize, Clone, Debug, PartialEq, Eq)]
#[serde(deny_unknown_fields)]
pub struct SavedServerV1 {
    pub name: String,
    pub address: String,
}

#[derive(Serialize, Deserialize, Clone, Debug, PartialEq, Eq)]
#[serde(deny_unknown_fields)]
pub struct RecentServerV1 {
    pub server_name: String,
    pub address: String,
}

impl ConnectionsFileV1 {
    #[must_use]
    pub fn empty() -> Self {
        Self {
            schema_version: CONNECTIONS_SCHEMA_VERSION,
            ..Self::default()
        }
    }

    pub fn validate(mut self) -> Result<Self, String> {
        if self.schema_version != CONNECTIONS_SCHEMA_VERSION {
            return Err("unsupported connections schema".to_string());
        }
        if self.favorites.len() > MAX_SAVED_SERVERS || self.recents.len() > MAX_SAVED_SERVERS {
            return Err("connections list exceeds its bound".to_string());
        }
        if let Some(name) = &self.preferred_display_name {
            self.preferred_display_name = Some(
                crate::lobby::normalize_proposed_display_name(name)
                    .map_err(|error| format!("invalid preferred display name: {error}"))?,
            );
        }
        let mut favorite_addresses = std::collections::BTreeSet::new();
        for favorite in &mut self.favorites {
            favorite.name = validate_server_name(&favorite.name)?;
            favorite.address = canonical_address(&favorite.address)?;
            if !favorite_addresses.insert(favorite.address.clone()) {
                return Err("duplicate favorite address".to_string());
            }
        }
        let mut recent_addresses = std::collections::BTreeSet::new();
        for recent in &mut self.recents {
            recent.server_name = validate_server_name(&recent.server_name)?;
            recent.address = canonical_address(&recent.address)?;
            if !recent_addresses.insert(recent.address.clone()) {
                return Err("duplicate recent address".to_string());
            }
        }
        Ok(self)
    }

    pub fn record_recent(&mut self, server_name: &str, address: &str) -> Result<(), String> {
        let server_name = validate_server_name(server_name)?;
        let address = canonical_address(address)?;
        self.recents.retain(|recent| recent.address != address);
        self.recents.insert(
            0,
            RecentServerV1 {
                server_name,
                address,
            },
        );
        self.recents.truncate(MAX_SAVED_SERVERS);
        Ok(())
    }

    pub fn add_favorite(&mut self, name: &str, address: &str) -> Result<(), String> {
        let name = validate_server_name(name)?;
        let address = canonical_address(address)?;
        if let Some(existing) = self
            .favorites
            .iter_mut()
            .find(|favorite| favorite.address == address)
        {
            existing.name = name;
            return Ok(());
        }
        if self.favorites.len() >= MAX_SAVED_SERVERS {
            return Err("favorite list is full".to_string());
        }
        self.favorites.push(SavedServerV1 { name, address });
        Ok(())
    }

    pub fn remove_favorite(&mut self, address: &str) -> bool {
        let Ok(address) = canonical_address(address) else {
            return false;
        };
        let before = self.favorites.len();
        self.favorites
            .retain(|favorite| favorite.address != address);
        self.favorites.len() != before
    }
}

fn validate_server_name(value: &str) -> Result<String, String> {
    crate::lobby::validate_presentation_name(value)
        .map_err(|error| format!("invalid saved server name: {error}"))
}

fn canonical_address(value: &str) -> Result<String, String> {
    parse_server_address(value)
        .map(|address| address.canonical().to_string())
        .map_err(|error| format!("invalid saved address: {error:?}"))
}

pub fn load_connections(path: &Path) -> Result<Option<ConnectionsFileV1>, String> {
    let file = match fs::File::open(path) {
        Ok(file) => file,
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => return Ok(None),
        Err(error) => return Err(format!("could not read connections: {error}")),
    };
    let mut bytes = Vec::new();
    file.take(MAX_CONNECTIONS_BYTES + 1)
        .read_to_end(&mut bytes)
        .map_err(|error| format!("could not read connections: {error}"))?;
    if bytes.len() > usize::try_from(MAX_CONNECTIONS_BYTES).unwrap_or(usize::MAX) {
        return Err("connections file is too large".to_string());
    }
    let source = std::str::from_utf8(&bytes)
        .map_err(|error| format!("connections file is not UTF-8: {error}"))?;
    let file: ConnectionsFileV1 =
        ron::from_str(source).map_err(|error| format!("connections file was rejected: {error}"))?;
    file.validate().map(Some)
}

pub fn save_connections(path: &Path, state: &ConnectionsFileV1) -> Result<(), String> {
    let state = state.clone().validate()?;
    let parent = path
        .parent()
        .ok_or_else(|| "connections path has no parent directory".to_string())?;
    fs::create_dir_all(parent)
        .map_err(|error| format!("could not create connections directory: {error}"))?;
    let serialized = ron::ser::to_string_pretty(&state, ron::ser::PrettyConfig::default())
        .map_err(|error| format!("could not encode connections: {error}"))?;
    let mut file = AtomicWriteFile::open(path)
        .map_err(|error| format!("could not open atomic connections file: {error}"))?;
    file.write_all(serialized.as_bytes())
        .map_err(|error| format!("could not write connections: {error}"))?;
    file.commit()
        .map_err(|error| format!("could not replace connections file: {error}"))
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::time::{SystemTime, UNIX_EPOCH};

    fn test_path(label: &str) -> PathBuf {
        let nonce = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap()
            .as_nanos();
        std::env::temp_dir().join(format!(
            "brawler-m03-{label}-{}-{nonce}",
            std::process::id()
        ))
    }

    #[test]
    fn valid_connections_round_trip_and_mru_is_bounded() {
        let path = test_path("round-trip").join("connections.ron");
        let mut state = ConnectionsFileV1::empty();
        state.preferred_display_name = Some("Player One".to_string());
        state.add_favorite("Local", "LOCALHOST").unwrap();
        for index in 0..20 {
            state
                .record_recent(&format!("Server {index}"), &format!("host{index}.test"))
                .unwrap();
        }
        assert_eq!(state.recents.len(), MAX_SAVED_SERVERS);
        save_connections(&path, &state).unwrap();
        assert_eq!(load_connections(&path).unwrap(), Some(state));
        fs::remove_dir_all(path.parent().unwrap()).unwrap();
    }

    #[test]
    fn favorite_identity_is_canonical_and_removal_is_offline() {
        let mut state = ConnectionsFileV1::empty();
        state.add_favorite("First", "LOCALHOST").unwrap();
        state.add_favorite("Renamed", "localhost:5000").unwrap();
        assert_eq!(state.favorites.len(), 1);
        assert_eq!(state.favorites[0].name, "Renamed");
        assert!(state.remove_favorite("localhost"));
        assert!(state.favorites.is_empty());
    }

    #[test]
    fn malformed_unsupported_invalid_and_oversized_files_fail_as_one_unit() {
        let dir = test_path("rejected");
        fs::create_dir_all(&dir).unwrap();
        let path = dir.join("connections.ron");
        for source in [
            "not ron".to_string(),
            "(schema_version:2,preferred_display_name:None,favorites:[],recents:[])".to_string(),
            "(schema_version:1,preferred_display_name:None,favorites:[(name:\"x\",address:\"bad host\")],recents:[])".to_string(),
        ] {
            fs::write(&path, &source).unwrap();
            assert!(load_connections(&path).is_err());
            assert_eq!(fs::read_to_string(&path).unwrap(), source);
        }
        fs::write(
            &path,
            vec![b'x'; usize::try_from(MAX_CONNECTIONS_BYTES + 1).unwrap()],
        )
        .unwrap();
        assert!(load_connections(&path).is_err());
        fs::remove_dir_all(dir).unwrap();
    }
}
