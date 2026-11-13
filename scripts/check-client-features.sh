#!/usr/bin/env bash
# Verifies that the player client build (`--no-default-features --features client`, also the
# package default) excludes automation and evidence-tooling code (BRL-0165):
#   1. the `automation` feature exists and is unreachable from `default` and `client`;
#   2. automation-owned module declarations stay behind `#[cfg(feature = "automation")]`;
#   3. the built player binary contains none of the automation marker strings.
set -euo pipefail

project_dir=$(CDPATH= cd -- "$(dirname -- "$0")/.." && pwd)
cd "$project_dir"

metadata="$(cargo metadata --locked --no-deps --format-version 1)"
python3 -c 'import json, sys
data = json.loads(sys.stdin.read())
package = next(p for p in data["packages"] if p["name"] == "brawler")
features = package["features"]
assert "automation" in features, "brawler has no automation feature"

def closure(name, seen):
    for entry in features.get(name, []):
        if entry in features and entry not in seen:
            seen.add(entry)
            closure(entry, seen)
    return seen

for root in ("default", "client"):
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
    "src/client/mod.rs:mod automation;"
    "src/client/mod.rs:mod vfx_lab;"
    "src/client/queue/mod.rs:mod automation;"
    "src/client/session/mod.rs:mod automation;"
    "src/client/ui/mod.rs:pub(crate) mod gallery;"
    "src/client/presentation_3d/mod.rs:mod diagnostics;"
    "src/bin/client.rs:mod automation;"
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
# in a `client,automation` build). Each must still exist in the source so a rename cannot
# silently turn this check into a no-op.
markers=(
    "[--combat-demo | --controller-demo]"
    "brawler product queue smoke"
    "headless movement automation enabled"
    "windowed synthetic controller demo enabled"
    "Automation Brawler"
    "frame_p95_ms"
    "capturing scheduled frame + client state"
)
for marker in "${markers[@]}"; do
    if ! grep -rqF -- "$marker" src; then
        printf 'player client: marker %s no longer exists in src; update this check\n' "$marker" >&2
        exit 1
    fi
done

cargo build --locked --bin brawler-client --no-default-features --features client
binary="target/debug/brawler-client"
binary_strings="$(strings -a "$binary")"
for marker in "${markers[@]}"; do
    if grep -qF -- "$marker" <<< "$binary_strings"; then
        printf 'player client binary unexpectedly contains automation marker %s\n' "$marker" >&2
        exit 1
    fi
done

echo "player client build excludes automation and evidence tooling"
