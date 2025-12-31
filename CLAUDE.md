# cdMenu - Development Notes

## Project Overview
A macOS/Windows menubar app for monitoring Bitbucket Cloud pipeline statuses, built with Tauri v2.

## Current Status
- v1.0.0 released: https://github.com/wmhegarty/cdmenu/releases/tag/v1.0.0
- Builds for macOS (ARM64 + Intel) and Windows (x64)
- **Issue**: macOS builds are not code-signed, causing Gatekeeper to block the app

### Temporary Workaround for Users
After downloading the DMG, users must run:
```bash
xattr -cr /Applications/cdMenu.app
```

---

## TODO: Code Signing Implementation

### macOS Code Signing

#### Requirements
1. **Apple Developer Account** ($99/year) - https://developer.apple.com/programs/
2. **Certificates needed**:
   - "Developer ID Application" certificate (for signing the app)
   - "Developer ID Installer" certificate (for signing the DMG/pkg)
3. **Notarization** - Required for apps distributed outside the App Store on macOS 10.15+

#### Setup Steps

1. **Create certificates in Apple Developer Portal**:
   - Go to Certificates, Identifiers & Profiles
   - Create "Developer ID Application" certificate
   - Create "Developer ID Installer" certificate
   - Download and install in Keychain

2. **Export certificates for CI**:
   ```bash
   # Export as .p12 files from Keychain Access
   # You'll need the certificate + private key
   ```

3. **GitHub Secrets to add**:
   ```
   APPLE_CERTIFICATE          # Base64 encoded .p12 file
   APPLE_CERTIFICATE_PASSWORD # Password for the .p12
   APPLE_ID                   # Your Apple ID email
   APPLE_PASSWORD             # App-specific password (not your Apple ID password)
   APPLE_TEAM_ID              # Your Team ID from Apple Developer Portal
   ```

4. **Generate App-Specific Password**:
   - Go to https://appleid.apple.com/
   - Sign in → Security → App-Specific Passwords
   - Generate one for "cdMenu CI"

5. **Update tauri.conf.json** - Add signing config:
   ```json
   {
     "bundle": {
       "macOS": {
         "signingIdentity": "-",
         "entitlements": null
       }
     }
   }
   ```

6. **Update GitHub Actions workflow**:
   ```yaml
   - name: Build Tauri app
     uses: tauri-apps/tauri-action@v0
     env:
       GITHUB_TOKEN: ${{ secrets.GITHUB_TOKEN }}
       APPLE_CERTIFICATE: ${{ secrets.APPLE_CERTIFICATE }}
       APPLE_CERTIFICATE_PASSWORD: ${{ secrets.APPLE_CERTIFICATE_PASSWORD }}
       APPLE_SIGNING_IDENTITY: "Developer ID Application: Your Name (TEAM_ID)"
       APPLE_ID: ${{ secrets.APPLE_ID }}
       APPLE_PASSWORD: ${{ secrets.APPLE_PASSWORD }}
       APPLE_TEAM_ID: ${{ secrets.APPLE_TEAM_ID }}
     with:
       args: --target ${{ matrix.target }}
   ```

#### Tauri Code Signing Docs
- https://v2.tauri.app/distribute/sign/macos/

---

### Windows Code Signing

#### Requirements
1. **Code Signing Certificate** - Options:
   - **EV Certificate** (~$400/year) - Immediate SmartScreen trust
   - **Standard Certificate** (~$100/year) - Builds reputation over time
   - Providers: DigiCert, Sectigo, GlobalSign

2. **For CI, you need**:
   - Certificate in .pfx format
   - Certificate password

#### GitHub Secrets to add:
```
WINDOWS_CERTIFICATE          # Base64 encoded .pfx file
WINDOWS_CERTIFICATE_PASSWORD # Password for the .pfx
```

#### Update GitHub Actions:
```yaml
- name: Build Tauri app (Windows)
  if: matrix.platform == 'windows-latest'
  uses: tauri-apps/tauri-action@v0
  env:
    GITHUB_TOKEN: ${{ secrets.GITHUB_TOKEN }}
    TAURI_SIGNING_PRIVATE_KEY: ${{ secrets.WINDOWS_CERTIFICATE }}
    TAURI_SIGNING_PRIVATE_KEY_PASSWORD: ${{ secrets.WINDOWS_CERTIFICATE_PASSWORD }}
```

#### Tauri Windows Signing Docs
- https://v2.tauri.app/distribute/sign/windows/

---

## Quick Reference

### Local Development
```bash
# Run in dev mode
cargo tauri dev

# Build release
cargo tauri build

# Build DMG only
cargo tauri build --bundles dmg
```

### Release Process
```bash
# Update version in:
# - package.json
# - src-tauri/Cargo.toml
# - src-tauri/tauri.conf.json

# Commit, tag, and push
git add -A && git commit -m "Release vX.X.X"
git tag vX.X.X
git push && git push --tags

# GitHub Actions will build and create release
```

### Key Files
- `src-tauri/src/tray.rs` - System tray menu
- `src-tauri/src/polling.rs` - Background pipeline checking + notifications
- `src-tauri/src/bitbucket/client.rs` - Bitbucket API client
- `src-tauri/src/config.rs` - App state and configuration
- `.github/workflows/build.yml` - CI/CD workflow

### Bitbucket API
- Requires: Pipelines:Read + Repositories:Read permissions
- App passwords: https://bitbucket.org/account/settings/app-passwords/
