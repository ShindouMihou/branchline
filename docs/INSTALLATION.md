# Installing Branchline on macOS

The current preview supports Apple Silicon Macs with macOS 12 or later.

```sh
curl -fsSL https://raw.githubusercontent.com/ShindouMihou/branchline/main/scripts/install.sh | bash
```

Read [the script](../scripts/install.sh) before running it if you prefer. It requires the standard tools shipped with macOS, not Node.js, Rust, Python, or GitHub CLI.

## What the installer does

1. Checks macOS, the host architecture, and the minimum OS version. Apple Silicon under Rosetta is recognized through the hardware capability check.
2. Resolves the latest published release, or uses the tag you specify.
3. Downloads the macOS app archive and its SHA-256 checksum over HTTPS.
4. Checks the checksum, expected app bundle identity, and code signature before replacing any existing installation.
5. Copies the app into a staging directory on the destination volume, then moves it into place. A failed replacement restores the previous copy.

The default destination is `~/Applications/Branchline.app`. No administrator privileges are requested. The script does not launch the app, remove quarantine attributes, disable Gatekeeper, or change your connection settings and request history.

The checksum protects against incomplete or corrupted downloads; it comes from the same GitHub release as the app. This preview is ad-hoc signed, not signed with a verified Developer ID or notarized by Apple. macOS may restrict opening it under your security policy. A one-line installer does not make an unnotarized app notarized.

## Choose a version or directory

```sh
curl -fsSL https://raw.githubusercontent.com/ShindouMihou/branchline/main/scripts/install.sh | \
  BRANCHLINE_VERSION=v0.4.0 BRANCHLINE_INSTALL_DIR="$HOME/Applications" bash
```

The destination must be an absolute, writable directory. Quit the Branchline GUI before updating. MCP-only adapter processes are not treated as the GUI app.

## Manual installation

Download `Branchline-macOS-arm64.zip` and `Branchline-macOS-arm64.zip.sha256` from the same [release](https://github.com/ShindouMihou/branchline/releases).

```sh
shasum -a 256 -c Branchline-macOS-arm64.zip.sha256
ditto -x -k Branchline-macOS-arm64.zip .
codesign --verify --deep --strict Branchline.app
```

Move the extracted app into your Applications folder and open it when ready.

## Uninstall

Quit Branchline and move its app bundle to Trash. Removing the app does not remove its separately stored settings, connections, or activity. Request receipts can be cleared through the app's Activity view before uninstalling.
