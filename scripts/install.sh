#!/bin/bash
set -euo pipefail

usage() {
  cat <<'HELP'
Install Branchline for Apple Silicon macOS 12 or later.

  curl -fsSL https://raw.githubusercontent.com/ShindouMihou/branchline/main/scripts/install.sh | bash

Optional environment variables:
  BRANCHLINE_VERSION       Release tag, such as v0.4.0 (default: latest)
  BRANCHLINE_INSTALL_DIR   Destination directory (default: ~/Applications)

The installer verifies the release checksum and app signature. It does not
request sudo, remove quarantine attributes, or open the installed app.
HELP
}

fail() { printf 'Branchline: %s\n' "$*" >&2; exit 1; }

cleanup() {
  local status=$?
  if [[ -n "$backup" && -e "$backup" && ! -e "$target" ]]; then
    mv "$backup" "$target" || printf 'Restore the previous app from %s\n' "$backup" >&2
  fi
  [[ -z "$scratch" ]] || rm -rf "$scratch"
  if [[ -n "$staging" && -d "$staging" && ! -e "$backup" ]]; then rm -rf "$staging"; fi
  exit "$status"
}

if [[ ${1:-} == '--help' || ${1:-} == '-h' ]]; then usage; exit 0; fi
[[ $# -eq 0 ]] || { usage >&2; exit 1; }
[[ $(uname -s) == Darwin ]] || fail 'This installer supports macOS only.'
[[ $(id -u) != 0 ]] || fail 'Run as your normal user. sudo is not required.'
architecture=$(uname -m)
if [[ "$architecture" == x86_64 && $(sysctl -n hw.optional.arm64 2>/dev/null || true) == 1 ]]; then architecture=arm64; fi
[[ "$architecture" == arm64 ]] || fail 'The current release supports Apple Silicon Macs only.'
os_version=$(sw_vers -productVersion)
os_major=${os_version%%.*}
[[ "$os_major" =~ ^[0-9]+$ && "$os_major" -ge 12 ]] || fail 'macOS 12 or later is required.'
for tool in curl plutil shasum ditto codesign mktemp; do command -v "$tool" >/dev/null || fail "Required tool is missing: $tool"; done
if pgrep -f '/Branchline\.app/Contents/MacOS/branchline([[:space:]]+--mcp-background)?$' >/dev/null; then
  fail 'Quit the Branchline app, then run this installer again.'
fi

scratch=''
staging=''
backup=''
target=''
trap cleanup EXIT
trap 'exit 130' INT
trap 'exit 143' TERM HUP
scratch=$(mktemp -d "${TMPDIR:-/tmp}/branchline-download.XXXXXX")
fetch() { curl --proto '=https' --tlsv1.2 --fail --location --silent --show-error "$@"; }
version=${BRANCHLINE_VERSION:-latest}
if [[ "$version" == latest ]]; then
  fetch https://api.github.com/repos/ShindouMihou/branchline/releases/latest -o "$scratch/release.json"
  version=$(plutil -extract tag_name raw -o - "$scratch/release.json") || fail 'Cannot read the latest release. Try an explicit BRANCHLINE_VERSION.'
fi
[[ "$version" =~ ^v[0-9]+\.[0-9]+\.[0-9]+([-+][0-9A-Za-z.-]+)?$ ]] || fail 'The release tag is invalid.'
asset=Branchline-macOS-arm64.zip
base="https://github.com/ShindouMihou/branchline/releases/download/$version"
printf 'Downloading Branchline %s…\n' "$version"
fetch "$base/$asset" -o "$scratch/$asset"
fetch "$base/$asset.sha256" -o "$scratch/checksum"
read -r expected ignored < "$scratch/checksum" || fail 'Cannot read the release checksum.'
[[ "$expected" =~ ^[0-9a-fA-F]{64}$ ]] || fail 'The release checksum has an invalid format.'
actual=$(shasum -a 256 "$scratch/$asset")
actual=${actual%% *}
[[ "$actual" == "$expected" ]] || fail 'Checksum mismatch. No app was installed.'
ditto -x -k "$scratch/$asset" "$scratch/unpacked"
source_app="$scratch/unpacked/Branchline.app"
[[ -d "$source_app" && ! -L "$source_app" ]] || fail 'The release does not contain a regular Branchline.app bundle.'
identifier=$(plutil -extract CFBundleIdentifier raw -o - "$source_app/Contents/Info.plist") || fail 'Cannot read the app identity.'
[[ "$identifier" == dev.branchline.git ]] || fail 'The downloaded app has an unexpected bundle identity.'
codesign --verify --deep --strict "$source_app" || fail 'The app signature did not verify. No app was installed.'
install_dir=${BRANCHLINE_INSTALL_DIR:-"$HOME/Applications"}
[[ "$install_dir" == /* ]] || fail 'BRANCHLINE_INSTALL_DIR must be an absolute directory.'
mkdir -p "$install_dir"
[[ -w "$install_dir" ]] || fail "The destination is not writable: $install_dir"
target="$install_dir/Branchline.app"
[[ ! -L "$target" ]] || fail 'The existing destination is a symlink. Choose a different install directory.'
[[ ! -e "$target" || -d "$target" ]] || fail 'The existing destination is not an app directory.'
staging=$(mktemp -d "$install_dir/.branchline-install.XXXXXX")
ditto "$source_app" "$staging/Branchline.app"
if [[ -e "$target" ]]; then
  backup="$staging/previous.app"
  mv "$target" "$backup"
fi
if ! mv "$staging/Branchline.app" "$target"; then fail 'Could not install the app; restoring the previous copy.'; fi
if [[ -n "$backup" ]]; then rm -rf "$backup"; backup=''; fi
printf '\nInstalled Branchline %s at:\n  %s\n\n' "$version" "$target"
printf 'Open it in Finder when you are ready. This preview build is ad-hoc signed\nand is not notarized; normal macOS security policies still apply.\n'
