# macOS Code Signing & Notarization

This guide covers how to sign and notarize HR Command Center for macOS distribution outside the App Store.

## Prerequisites

- Apple Developer Program membership ($99/year) -- enroll at https://developer.apple.com
- macOS with Xcode Command Line Tools installed
- Tauri CLI (`cargo install tauri-cli`)

## 1. Create a Developer ID Certificate

1. Open **Keychain Access** on your Mac
2. Go to **Keychain Access > Certificate Assistant > Request a Certificate from a Certificate Authority**
3. Enter your email, select "Saved to disk", and save the CSR file
4. Go to https://developer.apple.com/account/resources/certificates/list
5. Click **+** to create a new certificate
6. Select **Developer ID Application** (NOT "Apple Development" or "Apple Distribution")
7. Upload the CSR file from step 3
8. Download the certificate and double-click to install it in your Keychain

## 2. Find Your Signing Identity

Run this command to list installed code signing certificates:

```bash
security find-identity -v -p codesigning
```

Look for a line like:

```
"Developer ID Application: Your Name (TEAMID)"
```

This string is your signing identity.

## 3. Find Your Team ID

Your Team ID appears in parentheses at the end of your signing identity (e.g., `TEAMID` above).

You can also find it at https://developer.apple.com/account > Membership Details > Team ID.

## 4. Create an App-Specific Password

Apple requires an app-specific password for notarization (NOT your regular Apple ID password).

1. Go to https://appleid.apple.com
2. Sign in and navigate to **Sign-In and Security > App-Specific Passwords**
3. Click **Generate an app-specific password**
4. Label it "HR Command Center Notarization"
5. Copy and save the generated password securely

## 5. Set Environment Variables

Set these environment variables before building. NEVER hardcode these values in config files.

```bash
# Code signing identity (from step 2)
export APPLE_SIGNING_IDENTITY="Developer ID Application: Your Name (TEAMID)"

# Notarization credentials
export APPLE_ID="your-apple-id@example.com"
export APPLE_PASSWORD="xxxx-xxxx-xxxx-xxxx"
export APPLE_TEAM_ID="YOURTEAMID"
```

You can add these to a `.env` file (already in `.gitignore`) or your shell profile.

## 6. Build Signed + Notarized App

With environment variables set:

```bash
cargo tauri build
```

Tauri will automatically:
1. Sign the app with your Developer ID certificate
2. Submit the app to Apple's notary service
3. Staple the notarization ticket to the app

The signed `.dmg` will be in `src-tauri/target/release/bundle/dmg/`.

## 7. Verify Signing

After building, verify the app is properly signed:

```bash
codesign --verify --deep --strict --verbose=2 "src-tauri/target/release/bundle/macos/HR Command Center.app"
```

Verify notarization:

```bash
spctl --assess --type exec --verbose=2 "src-tauri/target/release/bundle/macos/HR Command Center.app"
```

## CI/CD (GitHub Actions)

For automated builds, store these as GitHub repository secrets:

| Secret | Value |
|--------|-------|
| `APPLE_CERTIFICATE` | Base64-encoded .p12 certificate (`base64 -i certificate.p12`) |
| `APPLE_CERTIFICATE_PASSWORD` | Password used when exporting .p12 |
| `APPLE_SIGNING_IDENTITY` | Certificate name from `security find-identity` |
| `APPLE_ID` | Apple account email |
| `APPLE_PASSWORD` | App-specific password |
| `APPLE_TEAM_ID` | Team ID |
| `KEYCHAIN_PASSWORD` | Arbitrary password for temporary CI keychain |

See the GitHub Actions workflow in `.github/workflows/` for the build configuration.

## Entitlements

The app's entitlements are defined in `src-tauri/Entitlements.plist`:

| Entitlement | Purpose |
|-------------|---------|
| `com.apple.security.app-sandbox` | Required for notarized apps |
| `com.apple.security.network.client` | Outbound network access (Claude API) |
| `com.apple.security.keychain-access-groups` | Keychain access (API key storage) |
| `com.apple.security.files.user-selected.read-write` | File picker access (CSV/Excel imports) |

## Troubleshooting

### "App is damaged and can't be opened"
The app is not signed or notarized. Make sure all env vars are set and rebuild.

### "Developer cannot be verified"
Notarization failed or was not stapled. Check `APPLE_ID`, `APPLE_PASSWORD`, and `APPLE_TEAM_ID`.

### Keychain access denied at runtime
Ensure the `com.apple.security.keychain-access-groups` entitlement includes your Team ID prefix.

### Build succeeds but app crashes on launch
Check Console.app for sandbox violations. You may need additional entitlements.
