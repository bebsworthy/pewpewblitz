#!/usr/bin/env bash
# Verifies that the player client build (`-p brawler-client` without features) excludes
# automation and evidence-tooling code (BRL-0165, package layout BRL-0170):
#   1. `brawler-client` owns the `automation` feature, unreachable from its default, and the shared
#      `brawler` crate has no automation feature;
#   2. automation-owned module declarations stay behind `#[cfg(feature = "automation")]`;
#   3. the built player binary contains none of the automation marker strings, including the
#      automation-only configuration validation, the legacy direct-UDP client combat evidence,
#      client process closeout/window sampling that live in the shared crate (BRL-3p70), and the
#      `--shell-capture` flag and its flow driver (BRL-pw9k).
# Set BRAWLER_CLIENT_FEATURES_POSITIVE_CONTROL=1 to also build the automation client and assert
# that it contains every marker, proving the markers are real.
set -euo pipefail

project_dir=$(CDPATH= cd -- "$(dirname -- "$0")/.." && pwd)
cd "$project_dir"

metadata="$(cargo metadata --locked --no-deps --format-version 1)"
python3 -c 'import json, sys
data = json.loads(sys.stdin.read())
shared = next(p for p in data["packages"] if p["name"] == "brawler")
assert "automation" not in shared["features"], "the shared brawler crate has an automation feature"
package = next(p for p in data["packages"] if p["name"] == "brawler-client")
features = package["features"]
assert "automation" in features, "brawler-client has no automation feature"

def closure(name, seen):
    for entry in features.get(name, []):
        if entry in features and entry not in seen:
            seen.add(entry)
            closure(entry, seen)
    return seen

for root in ("default",):
    reached = closure(root, {root})
    assert "automation" not in reached, f"the {root} feature enables automation"

# Developer-only review tools are automation builds, never player binaries.
for target in package["targets"]:
    if target["name"] in ("brawler-vfx-lab", "brawler-ui-gallery"):
        required = target.get("required-features", [])
        assert "automation" in required, "%s does not require automation" % target["name"]
' <<< "$metadata"

# Each automation-owned module or seam must be declared directly under the feature gate.
gated_declarations=(
    "packages/brawler-client/src/lib.rs:mod automation;"
    "packages/brawler-client/src/lib.rs:mod vfx_lab;"
    "packages/brawler-client/src/flow.rs:mod automation;"
    "packages/brawler-client/src/queue/mod.rs:mod automation;"
    "packages/brawler-client/src/session/mod.rs:mod automation;"
    "packages/brawler-client/src/ui/mod.rs:pub(crate) mod gallery;"
    "packages/brawler-client/src/presentation_3d/mod.rs:mod diagnostics;"
    "packages/brawler-client/src/bin/client.rs:mod automation;"
)
for entry in "${gated_declarations[@]}"; do
    file=${entry%%:*}
    declaration=${entry#*:}
    if ! awk -v declaration="$declaration" '
        $0 == declaration { found = 1; if (gated) ok = 1 }
        { gated = ($0 ~ /^#\[cfg\(feature = "automation"\)\]$/) || (gated && $0 ~ /^#\[path = /) }
        END { exit (found && ok) ? 0 : 1 }
    ' "$file"; then
        printf 'player client: `%s` in %s is not directly gated by the automation feature\n' \
            "$declaration" "$file" >&2
        exit 1
    fi
done

# Literal strings owned by automation-only code reachable from brawler-client (verified present
# in a `-p brawler-client --features automation` build). Each must still exist in the source so a
# rename cannot silently turn this check into a no-op.
markers=(
    "[--combat-demo | --controller-demo]"
    "brawler product queue smoke"
    "headless movement automation enabled"
    "windowed synthetic controller demo enabled"
    "Automation Brawler"
    "frame_p95_ms"
    "capturing scheduled frame + client state"
    "--product-practice-smoke requires product match automation"
    "timing handoff-connected"
    "headless client is extending its run until combat evidence completes"
    "BRAWLER_NETWORK_COMBAT_CLIENT_READY_FILE"
    "BRAWLER_DIAGNOSTICS_CLOSEOUT_FILE"
    "--shell-capture"
    "shell capture route reached a step"
)
for marker in "${markers[@]}"; do
    if ! grep -rqF -- "$marker" packages/brawler-client/src src; then
        printf 'player client: marker %s no longer exists in packages/brawler-client/src or src; update this check\n' "$marker" >&2
        exit 1
    fi
done

cargo build --locked -p brawler-client --bin brawler-client
binary="${CARGO_TARGET_DIR:-target}/debug/brawler-client"
binary_strings="$(strings -a "$binary")"
for marker in "${markers[@]}"; do
    if grep -qF -- "$marker" <<< "$binary_strings"; then
        printf 'player client binary unexpectedly contains automation marker %s\n' "$marker" >&2
        exit 1
    fi
done

if [[ "${BRAWLER_CLIENT_FEATURES_POSITIVE_CONTROL:-0}" == 1 ]]; then
    cargo build --locked -p brawler-client --features automation --bin brawler-client
    binary_strings="$(strings -a "$binary")"
    for marker in "${markers[@]}"; do
        if ! grep -qF -- "$marker" <<< "$binary_strings"; then
            printf 'automation client binary lacks marker %s; it no longer proves exclusion\n' "$marker" >&2
            exit 1
        fi
    done
    echo "automation client build contains every marker (positive control)"
fi

echo "player client build excludes automation and evidence tooling"
