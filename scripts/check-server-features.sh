#!/usr/bin/env bash
set -euo pipefail

metadata="$(cargo metadata --locked --no-deps --format-version 1 --no-default-features --features server)"
python3 -c 'import json, sys
data = json.loads(sys.stdin.read())
package = next(p for p in data["packages"] if p["name"] == "brawler")
assert "client" in package["features"]
assert "server" in package["features"]
' <<< "$metadata"

features="$(cargo tree --locked --no-default-features --features server -e features)"
# The bevy_input crate is part of Bevy's core ECS API in 0.19. The forbidden
# capabilities here are its actual device backends and all client presentation.
for forbidden in bevy_render bevy_winit bevy_window bevy_audio bevy_asset; do
  if grep -q "$forbidden" <<< "$features"; then
    echo "server feature graph unexpectedly contains $forbidden" >&2
    exit 1
  fi
done

if grep -qE 'feature "(keyboard|mouse|gamepad|touch|gestures)"' <<< "$features"; then
  echo "server feature graph unexpectedly contains a device-input backend" >&2
  exit 1
fi

echo "server feature graph excludes client presentation capabilities"
