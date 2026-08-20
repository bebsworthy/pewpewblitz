#!/usr/bin/env bash
set -euo pipefail

project_dir=$(CDPATH= cd -- "$(dirname -- "$0")/.." && pwd)
cd "$project_dir"

world_sources=(src/client/presentation_3d src/combat/client src/map/client.rs src/terrain/client)
if rg -n '\b(Sprite|Text2d|Mesh2d|ColorMaterial)\b' "${world_sources[@]}"; then
    printf '%s\n' 'V3 gameplay-world presentation contains a retired 2D render type' >&2
    exit 1
fi
if rg -n 'VisualPlacementKind::Sprite|facility_tileset|team_(blue|red)\.png' src assets/manifest.ron Cargo.toml; then
    printf '%s\n' 'V3 source or manifest contains a retired renderer contract or asset' >&2
    exit 1
fi
if awk '
    /^bevy-client = \[/ { in_client = 1 }
    in_client && /"bevy\/bevy_sprite"/ { found = 1 }
    in_client && /^]/ { in_client = 0 }
    END { exit found ? 0 : 1 }
' Cargo.toml; then
    printf '%s\n' 'V3 client directly enables the retired world-sprite feature' >&2
    exit 1
fi

printf '%s\n' 'V3 gameplay-world source excludes retired 2D renderer contracts'
