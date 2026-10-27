# In-app updates

Branchline checks for updates at startup and at most once every six hours while running. Checks are global across repository windows and do not download or install automatically. Users can disable automatic checks directly in Appearance → App updates, or check manually using Branchline → Check for Updates…. Checks open a prompt only when an update is available. An up-to-date result or check error uses a nonmodal status message.

When an update is available, a small App update button opens its version and release notes. Install & restart downloads the signed archive, verifies it, replaces the current application using Tauri's updater, and restarts. All Branchline windows close. Installation is refused while a Git operation or an approved AI command is active. Commit drafts remain in local storage; pending approvals remain in the connector's journal.

Checks use this public endpoint without repository details or credentials:

```text
https://github.com/ShindouMihou/branchline/releases/latest/download/latest.json
```

Network failures stay quiet during background checks. Manual checks show an error and offer a retry. A missing manifest does not count as “up to date.” The signed archive must come from this repository's HTTPS release-download path. Tauri verifies its signature against the embedded public key and requires the signed version to match the announced version; altered archives or inflated version metadata are rejected.

## First installation with updater support

Versions through 0.4.2 do not contain this updater. Existing users must install one updater-enabled release from the usual download or installer. After that, future releases can be installed inside the app. Version 0.4.3 is the first release that includes the updater and its signed update feed.

The update signature is separate from macOS code signing. Preview builds remain ad-hoc signed and are not notarized. Neither the updater nor the installer bypasses Gatekeeper.

## Preparing a release

1. Update the version in package.json, package-lock.json, src-tauri/Cargo.toml, src-tauri/Cargo.lock, and src-tauri/tauri.conf.json. Add matching notes at docs/releases/vVERSION.md.
2. Run the frontend, Rust, installer, and release-manifest checks.
3. On an Apple Silicon Mac, run:

   ```sh
   npm run release:prepare
   ```

   The helper builds the app with updater artifacts, verifies the app signature, version, identity, and architecture, and writes five files into .local/releases/vVERSION. Existing release output is never overwritten. Use `-- --output .local/updater-validation` for a separate local validation build.
4. Publish all five files with the matching GitHub release:

   - Branchline-macOS-arm64.zip
   - Branchline-macOS-arm64.zip.sha256
   - Branchline.app.tar.gz
   - Branchline.app.tar.gz.sig
   - latest.json

5. Mark the release as latest only after the correct commit/tag and all files are ready. Download the published files to verify their checksums and manifest. Publishing latest.json without its signed archive leaves in-app installation unavailable.

Do not re-sign or modify Branchline.app after creating the updater archive. Tauri applies the configured macOS signing identity before packaging; the signed tarball and the regular ZIP come from the same app bundle.

## Signing key

The initial private updater key is stored outside the repository at `~/.tauri/branchline-updater.key`, with owner-only permissions. Its adjacent `.pub` file must match the public key embedded in src-tauri/tauri.conf.json. The release helper checks this before building. Back up the private key securely: losing it prevents publishing updates trusted by existing installations. Never commit it, attach it to a release, or paste it into a command or approval request.

To use another secure key-file location, set TAURI_SIGNING_PRIVATE_KEY to its file path and keep the matching .pub alongside it. An encrypted key can use TAURI_SIGNING_PRIVATE_KEY_PASSWORD from a protected environment. Replacing the public key is a migration requiring a release trusted by the old key; generating a new key alone does not update existing users' trust.

For notarized production releases, configure a Developer ID signing identity and notarization credentials separately from the updater key.
