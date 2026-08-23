#!/usr/bin/env bash
set -euo pipefail

project_dir=$(CDPATH= cd -- "$(dirname -- "$0")/.." && pwd)
cd "$project_dir"

retired_paths=(
    content/v1
    content/v4
    content/v7
    content/v8
    src/terrain
    assets/catalogs/environment_visuals.ron
)
for path in "${retired_paths[@]}"; do
    if [[ -e "$path" ]]; then
        printf 'retired map-system path remains: %s\n' "$path" >&2
        exit 1
    fi
done

if rg -n \
    'TerrainChannel|LegacyMapTestOverride|compatibility_runtime_map|GridMapRoot|ResolvedGridMap|MapObjectDefinitionId|RegionProfileId|V8-MIGRATION' \
    src tests; then
    printf '%s\n' 'retired map or terrain API remains in current source' >&2
    exit 1
fi

if rg -n \
    'content/v(1|4|7|8)|assets/catalogs/environment_visuals\.ron' \
    src tests build.rs README.md AGENTS.md assets/manifest.ron; then
    printf '%s\n' 'current production inputs still name a retired content location' >&2
    exit 1
fi

printf '%s\n' 'V8 canonical map source excludes retired map and terrain contracts'
