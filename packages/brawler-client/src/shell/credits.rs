//! Credits overlay text (BRL-wpr8). The third-party list mirrors the "Shipped packs" table of
//! `assets/licenses/THIRD_PARTY.md`, and the original-work list covers every
//! `pewpew-blitz-original-*.txt` note beside it; the tests below fail when either side drifts.

/// One shipped third-party pack, keyed by its exact `THIRD_PARTY.md` pack name.
struct ThirdPartyCredit {
    /// Pack name as written in the index table, shown verbatim.
    pack: &'static str,
    used_for: &'static str,
    author: &'static str,
    /// SPDX identifier as written in the index table.
    license: &'static str,
}

const THIRD_PARTY: [ThirdPartyCredit; 6] = [
    ThirdPartyCredit {
        pack: "Mini Characters",
        used_for: "zombie NPC",
        author: "Kenney",
        license: "CC0-1.0",
    },
    ThirdPartyCredit {
        pack: "Sci-Fi Sounds",
        used_for: "fire and defeat sounds",
        author: "Kenney",
        license: "CC0-1.0",
    },
    ThirdPartyCredit {
        pack: "Impact Sounds",
        used_for: "impact sound",
        author: "Kenney",
        license: "CC0-1.0",
    },
    ThirdPartyCredit {
        pack: "Interface Sounds",
        used_for: "ready and error sounds",
        author: "Kenney",
        license: "CC0-1.0",
    },
    ThirdPartyCredit {
        pack: "Lilita One",
        used_for: "display font",
        author: "Juan Montoreano",
        license: "OFL-1.1",
    },
    ThirdPartyCredit {
        pack: "Fira Mono (subset)",
        used_for: "Bevy default font",
        author: "Mozilla Foundation / Telefonica",
        license: "OFL-1.1",
    },
];

/// Original project work, each with the repository path that records its provenance: a
/// `pewpew-blitz-original-*.txt` note, or the `asset_src/` builder directory where no note exists.
const ORIGINAL: [(&str, &str); 7] = [
    ("Riff fighter", "asset_src/characters/riff"),
    (
        "fighter weapons",
        "assets/licenses/pewpew-blitz-original-weapons.txt",
    ),
    (
        "environment kits",
        "assets/licenses/pewpew-blitz-original-environment.txt",
    ),
    (
        "floor decals",
        "assets/licenses/pewpew-blitz-original-decals.txt",
    ),
    (
        "icons and mode pennants",
        "assets/licenses/pewpew-blitz-original-icons.txt",
    ),
    ("branding and menu art", "asset_src/player-shell"),
    (
        "all other sound effects and music",
        "assets/licenses/pewpew-blitz-original-audio.txt",
    ),
];

fn license_label(spdx: &str) -> &'static str {
    match spdx {
        "CC0-1.0" => "CC0 1.0",
        "OFL-1.1" => "SIL OFL 1.1",
        other => panic!("Credits has no display label for license {other}"),
    }
}

pub(super) fn credits_text() -> String {
    use std::fmt::Write as _;

    let mut text =
        String::from("PewPew Blitz 0.1.0\n\nBuilt with Bevy 0.19 (MIT OR Apache-2.0).\n\n");
    text.push_str("THIRD-PARTY ASSETS\n");
    for credit in &THIRD_PARTY {
        let _ = writeln!(
            text,
            "{}, {}: {}, {}.",
            credit.pack,
            credit.used_for,
            credit.author,
            license_label(credit.license)
        );
    }
    text.push_str("\nORIGINAL PEWPEW BLITZ WORK\n");
    let (last, rest) = ORIGINAL.split_last().expect("original work is credited");
    let rest: Vec<_> = rest.iter().map(|(label, _)| *label).collect();
    // The first label starts the sentence, so it is written capitalized.
    let _ = write!(text, "{}, and {}.", rest.join(", "), last.0);
    text.push_str("\n\nFull license texts ship in assets/licenses/.");
    text
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::collections::BTreeSet;
    use std::path::Path;

    fn root() -> &'static Path {
        Path::new(crate::REPOSITORY_ROOT)
    }

    /// `(pack, author, license, license file)` rows of the index's "Shipped packs" table.
    fn shipped_packs() -> Vec<[String; 4]> {
        let index = std::fs::read_to_string(root().join("assets/licenses/THIRD_PARTY.md"))
            .expect("THIRD_PARTY.md");
        let table = index
            .split("## Shipped packs")
            .nth(1)
            .and_then(|rest| rest.split("\n## ").next())
            .expect("a Shipped packs section");
        let rows: Vec<[String; 4]> = table
            .lines()
            .filter(|line| line.starts_with('|'))
            .skip(2) // header and separator
            .map(|line| {
                let cells: Vec<&str> = line.trim_matches('|').split('|').map(str::trim).collect();
                assert_eq!(cells.len(), 6, "unexpected Shipped packs row: {line}");
                [cells[0], cells[2], cells[3], cells[5].trim_matches('`')].map(str::to_owned)
            })
            .collect();
        assert!(!rows.is_empty(), "the Shipped packs table is empty");
        rows
    }

    #[test]
    fn third_party_credits_match_the_license_index() {
        let index: BTreeSet<[String; 3]> = shipped_packs()
            .into_iter()
            .map(|[pack, author, license, _]| [pack, author, license])
            .collect();
        let credits: BTreeSet<[String; 3]> = THIRD_PARTY
            .iter()
            .map(|c| [c.pack, c.author, c.license].map(str::to_owned))
            .collect();
        assert_eq!(
            credits, index,
            "Credits and THIRD_PARTY.md shipped packs differ"
        );
        assert_eq!(credits.len(), THIRD_PARTY.len(), "a pack is credited twice");
    }

    #[test]
    fn every_license_file_is_credited() {
        let licenses = root().join("assets/licenses");
        let shipped: BTreeSet<String> =
            shipped_packs().into_iter().map(|[.., file]| file).collect();
        for file in &shipped {
            assert!(licenses.join(file).is_file(), "missing license text {file}");
        }
        let original: BTreeSet<&str> = ORIGINAL.iter().map(|(_, path)| *path).collect();
        for (label, path) in &ORIGINAL {
            assert!(
                root().join(path).exists(),
                "{label}: missing provenance {path}"
            );
        }
        for entry in std::fs::read_dir(&licenses).expect("assets/licenses") {
            let name = entry.expect("license entry").file_name();
            let name = name.to_str().expect("UTF-8 file name");
            if name == "THIRD_PARTY.md" {
                continue;
            }
            let credited = shipped.contains(name)
                || original.contains(format!("assets/licenses/{name}").as_str());
            assert!(credited, "assets/licenses/{name} is not credited");
        }
    }

    #[test]
    fn credits_text_names_only_shipped_work() {
        let text = credits_text();
        for credit in &THIRD_PARTY {
            assert!(text.contains(credit.pack), "{}", credit.pack);
        }
        for (label, _) in &ORIGINAL {
            assert!(
                text.to_lowercase().contains(&label.to_lowercase()),
                "{label}"
            );
        }
        for retired in ["Blaster Kit", "Murphy", "HaywardMorihara", "KayKit"] {
            assert!(!text.contains(retired), "Credits still names {retired}");
        }
    }
}
