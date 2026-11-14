use super::*;
use crate::REPOSITORY_ROOT;

/// Size budget per weapon for the mobile APK (BRL-5k0g); the builder enforces the same ceiling.
const WEAPON_GLB_BYTE_BUDGET: usize = 48_000;

fn catalog() -> FighterWeaponCatalog {
    FighterWeaponCatalog::embedded().expect("embedded fighter weapon catalog")
}

fn glb_json(asset_path: &str) -> (serde_json::Value, usize) {
    let path = std::path::Path::new(REPOSITORY_ROOT)
        .join("assets")
        .join(asset_path);
    let bytes = std::fs::read(&path).unwrap_or_else(|_| panic!("{asset_path} is shipped"));
    assert_eq!(&bytes[0..4], b"glTF");
    let json_length = u32::from_le_bytes(bytes[12..16].try_into().unwrap()) as usize;
    assert_eq!(&bytes[16..20], b"JSON");
    let json = serde_json::from_slice(&bytes[20..20 + json_length]).unwrap();
    (json, bytes.len())
}

fn node<'a>(json: &'a serde_json::Value, name: &str) -> Option<&'a serde_json::Value> {
    json["nodes"]
        .as_array()
        .unwrap()
        .iter()
        .find(|node| node["name"] == name)
}

#[test]
fn embedded_catalog_gives_every_family_its_own_original_model() {
    let catalog = catalog();
    let models = WeaponFamily::ALL
        .map(|family| catalog.model(family).model.clone())
        .into_iter()
        .collect::<std::collections::BTreeSet<_>>();
    assert_eq!(models.len(), 4, "each family reads as its own weapon");
    assert!(
        models
            .iter()
            .all(|model| model.starts_with(WEAPON_MODEL_ROOT))
    );
}

#[test]
fn shipped_weapon_glbs_match_the_attachment_contract() {
    let catalog = catalog();
    for family in WeaponFamily::ALL {
        let model = &catalog.model(family).model;
        let (json, size) = glb_json(model);
        assert!(size <= WEAPON_GLB_BYTE_BUDGET, "{model} is {size} bytes");
        let meshes = json["meshes"].as_array().unwrap();
        assert_eq!(meshes.len(), 1, "{model}: one mesh, so one outline hull");
        let primitives = meshes[0]["primitives"].as_array().unwrap();
        assert_eq!(primitives.len(), 1);
        assert!(primitives[0]["attributes"].get("COLOR_0").is_some());
        assert_eq!(json["materials"].as_array().unwrap().len(), 1);
        for forbidden in [
            "cameras",
            "lights",
            "skins",
            "animations",
            "textures",
            "images",
        ] {
            assert!(json.get(forbidden).is_none(), "{model} exports {forbidden}");
        }
        let root = node(&json, "weapon_root").expect("weapon_root attaches to the socket");
        assert!(
            root.get("translation").is_none() && root.get("rotation").is_none(),
            "{model}: weapon_root is the identity attachment origin"
        );
        for anchor in ["grip_primary", "muzzle", "projectile_spawn"] {
            assert!(node(&json, anchor).is_some(), "{model} lacks {anchor}");
        }
        // Riff's two-handed clips bake the support hand onto this anchor.
        let two_handed = family != WeaponFamily::OneHanded;
        assert_eq!(
            node(&json, "grip_secondary").is_some(),
            two_handed,
            "{model}: grip_secondary marks a two-handed family"
        );
    }
}

#[test]
fn preview_framing_centres_each_model() {
    let catalog = catalog();
    for family in WeaponFamily::ALL {
        let transform = catalog.preview_transform(family);
        let model = catalog.model(family);
        assert!(
            (transform.transform_point(model.preview_center)).length() < 1e-5,
            "{family:?} preview centre lands on the camera target"
        );
    }
}

#[test]
fn catalog_rejects_missing_duplicate_and_out_of_tree_models() {
    let row = |family: &str, model: &str| {
        format!(
            "(family: {family}, model: \"{model}\", preview_scale: 1.0, preview_center: (0.0, 0.0, 0.0))"
        )
    };
    let source = |rows: &[String]| format!("(schema_version: 1, weapons: [{}])", rows.join(", "));
    let good = |family| row(family, "brawler/models/original/weapons/wpn_test.glb");
    let all = ["OneHanded", "Heavy", "Throwable", "Melee"]
        .map(good)
        .to_vec();
    assert!(FighterWeaponCatalog::from_ron(&source(&all)).is_ok());
    assert!(FighterWeaponCatalog::from_ron(&source(&all[..3])).is_err());
    let mut duplicate = all.clone();
    duplicate[3] = good("Heavy");
    assert!(FighterWeaponCatalog::from_ron(&source(&duplicate)).is_err());
    let mut third_party = all.clone();
    third_party[0] = row(
        "OneHanded",
        "brawler/models/kenney/blaster-kit/blaster-a.glb",
    );
    assert!(FighterWeaponCatalog::from_ron(&source(&third_party)).is_err());
    let mut escaping = all;
    escaping[0] = row("OneHanded", "brawler/models/original/weapons/../x.glb");
    assert!(FighterWeaponCatalog::from_ron(&source(&escaping)).is_err());
}

#[test]
fn held_weapon_swaps_only_when_the_family_changes() {
    let mut world = World::new();
    let socket = world.spawn_empty().id();
    let mut held = HeldWeapon::new(socket);
    let mut commands = world.commands();
    assert_eq!(
        held.begin_swap(&mut commands, WeaponFamily::Heavy),
        Some(socket)
    );
    let sprayer = commands.spawn_empty().id();
    held.attached(WeaponFamily::Heavy, sprayer);
    assert_eq!(held.begin_swap(&mut commands, WeaponFamily::Heavy), None);
    assert_eq!(
        held.begin_swap(&mut commands, WeaponFamily::Melee),
        Some(socket)
    );
    world.flush();
    assert!(
        world.get_entity(sprayer).is_err(),
        "the stale family model is despawned"
    );
}
