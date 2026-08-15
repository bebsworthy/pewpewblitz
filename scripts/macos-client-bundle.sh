#!/usr/bin/env bash
set -euo pipefail

if [[ "$(uname -s)" != "Darwin" ]]; then
    printf 'brawler macOS bundle: this helper requires macOS\n' >&2
    exit 2
fi

script_dir="$(cd -- "$(dirname -- "${BASH_SOURCE[0]}")" && pwd)"
repo_root="$(cd -- "$script_dir/.." && pwd)"
target_dir="${CARGO_TARGET_DIR:-$repo_root/target}"
if [[ "$target_dir" != /* ]]; then
    target_dir="$repo_root/$target_dir"
fi

cargo build --locked --manifest-path "$repo_root/Cargo.toml" --target-dir "$target_dir" \
    --no-default-features --features client --bin brawler-client

bundle_root="$(mktemp -d "${TMPDIR:-/tmp}/brawler-client-bundle.XXXXXX")"
bundle_path="$bundle_root/BrawlerVisualClient.app"
contents_path="$bundle_path/Contents"
executable_path="$contents_path/MacOS/BrawlerVisualClient"
plist_path="$contents_path/Info.plist"

mkdir -p "$contents_path/MacOS"
cp "$target_dir/debug/brawler-client" "$executable_path"
chmod 755 "$executable_path"
plutil -create xml1 "$plist_path"
plutil -insert CFBundleDevelopmentRegion -string en "$plist_path"
plutil -insert CFBundleDisplayName -string 'Brawler Visual Client' "$plist_path"
plutil -insert CFBundleExecutable -string BrawlerVisualClient "$plist_path"
plutil -insert CFBundleIdentifier -string dev.brawler.visual-client "$plist_path"
plutil -insert CFBundleInfoDictionaryVersion -string 6.0 "$plist_path"
plutil -insert CFBundleName -string BrawlerVisualClient "$plist_path"
plutil -insert CFBundlePackageType -string APPL "$plist_path"
plutil -insert CFBundleShortVersionString -string 0.1.0 "$plist_path"
plutil -insert NSHighResolutionCapable -bool true "$plist_path"

printf '%s\n' "$bundle_path"
