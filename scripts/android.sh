#!/usr/bin/env bash
# Builds the Android player APK (BRL-0159) and optionally installs and launches it over adb.
#   scripts/android.sh build   [HOST:PORT]
#   scripts/android.sh install [HOST:PORT]
# HOST:PORT is an optional, editable server prefill baked into the APK (for example the Mac's LAN
# address running `BRAWLER_ROUTED_BIND=0.0.0.0:5000 just server`).
set -euo pipefail

project_dir=$(CDPATH= cd -- "$(dirname -- "$0")/.." && pwd)
mode=${1:-}
server=${2:-}
ndk_version=29.0.14206865
android_dir="$project_dir/packages/brawler-android/android"
apk="$android_dir/app/build/outputs/apk/release/app-release.apk"
package=com.pewpewblitz.client

case "$mode" in
    build | install) ;;
    *)
        printf '%s\n' 'usage: scripts/android.sh build|install [HOST:PORT]' >&2
        exit 2
        ;;
esac

export ANDROID_HOME=${ANDROID_HOME:-$HOME/Library/Android/sdk}
export ANDROID_NDK_HOME=${ANDROID_NDK_HOME:-$ANDROID_HOME/ndk/$ndk_version}
# A stale JAVA_HOME (for example a removed Android Studio JBR) falls back to Homebrew's JDK 21.
if [[ ! -x "${JAVA_HOME:-}/bin/java" && -x /opt/homebrew/opt/openjdk@21/bin/java ]]; then
    export JAVA_HOME=/opt/homebrew/opt/openjdk@21
fi

missing=()
[[ -d "$ANDROID_NDK_HOME" ]] || missing+=("Android NDK $ndk_version at $ANDROID_NDK_HOME")
[[ -d "$ANDROID_HOME/platforms/android-37.0" || -d "$ANDROID_HOME/platforms/android-37" ]] ||
    missing+=("SDK platform android-37.0")
command -v cargo-ndk >/dev/null || missing+=("cargo-ndk (cargo install cargo-ndk --locked)")
command -v gradle >/dev/null || missing+=("gradle (brew install gradle)")
[[ -x "${JAVA_HOME:-}/bin/java" ]] || missing+=("a JDK 17+ via JAVA_HOME (brew install openjdk@21)")
if ((${#missing[@]})); then
    printf 'brawler android: missing prerequisite: %s\n' "${missing[@]}" >&2
    printf '%s\n' 'brawler android: see the Android section of README.md' >&2
    exit 1
fi

cd "$project_dir"
BRAWLER_ANDROID_SERVER="$server" cargo ndk \
    --target arm64-v8a \
    --platform 31 \
    --output-dir "$android_dir/app/src/main/jniLibs" \
    build --locked -p brawler-android --profile android
gradle --project-dir "$android_dir" --console=plain assembleRelease
printf 'brawler android: APK %s (%s)\n' "$apk" "$(du -h "$apk" | cut -f1)"

if [[ "$mode" == install ]]; then
    adb=$ANDROID_HOME/platform-tools/adb
    "$adb" install -r "$apk"
    "$adb" shell am start -n "$package/.MainActivity"
fi
