//! Neutral envelope for all shared authored gameplay content.

use bevy::prelude::Resource;
use serde::{Deserialize, Serialize};

pub const GAMEPLAY_CONTENT_ENVELOPE_VERSION: u16 = 17;

#[derive(Resource, Serialize, Deserialize, Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub struct GameplayContentFingerprint(pub u64);

pub fn gameplay_content_fingerprint(
    weapons: &crate::combat::WeaponCatalog,
    maps: &crate::map::MapContentCatalog,
    builds: &crate::builds::BuildCatalog,
) -> Result<GameplayContentFingerprint, String> {
    let weapon_material = weapons.canonical_fingerprint_material()?;
    let map_material = maps.canonical_fingerprint_material()?;
    let build_material = builds.canonical_fingerprint_material()?;
    let part_material =
        crate::weapon_parts::WeaponPartCatalog::embedded()?.canonical_fingerprint_material()?;
    let bytes = postcard::to_allocvec(&(
        GAMEPLAY_CONTENT_ENVELOPE_VERSION,
        weapon_material,
        map_material,
        build_material,
        part_material,
    ))
    .map_err(|error| format!("gameplay content envelope serialization failed: {error}"))?;
    Ok(GameplayContentFingerprint(fnv1a64(&bytes)))
}

#[must_use]
pub fn fnv1a64(bytes: &[u8]) -> u64 {
    fnv1a64_seeded(0xcbf2_9ce4_8422_2325, bytes)
}

/// Continue one FNV-1a hash with more material instead of starting a new digest.
#[must_use]
pub fn fnv1a64_seeded(seed: u64, bytes: &[u8]) -> u64 {
    bytes.iter().fold(seed, |hash, byte| {
        (hash ^ u64::from(*byte)).wrapping_mul(0x0100_0000_01b3)
    })
}
