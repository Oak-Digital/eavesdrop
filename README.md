# Eavesdrop

Eavesdrop is a tray-first meeting recorder for macOS 13+ and Windows 10/11. It records a selected microphone alone or mixes it with computer audio, writes recoverable two-second AAC fragments, and keeps the finalized M4A library encrypted until the user explicitly plays or exports a recording.

## Run locally

Prerequisites: Node.js 20+, Rust 1.88+, CMake, the Tauri 2 platform prerequisites, and full Xcode on macOS or Visual Studio Build Tools with the Windows SDK on Windows.

```sh
npm install
npm run tauri dev
```

The first recording asks for microphone permission. Online recording on macOS also requires Screen Recording permission. The app remains available from the menu bar or notification area after its library window is closed.

## Local transcription

Eavesdrop embeds the open-source [`whisper.cpp`](https://github.com/ggml-org/whisper.cpp) engine. Transcription happens on-device: decrypted audio is decoded and processed in memory and is never uploaded or written to a plaintext working file.

1. Open **Settings → Local transcription**, choose Tiny, Base, or Small, and select **Install**. Base is the recommended starting point.
2. Eavesdrop downloads the model from the [official whisper.cpp model repository](https://huggingface.co/ggerganov/whisper.cpp/tree/main), verifies its published fingerprint, and selects it automatically.
3. Open a finished recording and select **Transcribe**. Eavesdrop saves the timestamped transcript in its local library.

Larger models are generally more accurate but need more memory and take longer to run. An existing whisper.cpp-compatible `.bin` model can still be selected manually.

## Verification

```sh
npm run build
npm test
cd src-tauri
cargo fmt --all -- --check
cargo clippy --all-targets --no-default-features -- -D warnings
cargo test --lib --no-default-features
```

The default build uses a lightweight correlation-based echo suppressor. A bundled WebRTC AEC3 build is available with `cargo build --features aec3`; it requires Meson and a working native C/C++ toolchain.

## Packaging

Build a universal macOS application and DMG on macOS:

```sh
rustup target add aarch64-apple-darwin x86_64-apple-darwin
npm run tauri build -- --target universal-apple-darwin --bundles app,dmg
# Keep the development identity stable across unsigned pilot builds:
./scripts/sign-macos-local.sh src-tauri/target/universal-apple-darwin/release/bundle/macos/Eavesdrop.app
```

Updater artifacts require `TAURI_SIGNING_PRIVATE_KEY` or `TAURI_SIGNING_PRIVATE_KEY_PATH`. For a local app-only build that does not publish an update, use `npm run bundle:mac`; it disables updater artifact generation for that build and applies the stable pilot signature.

Build the Windows x64 MSI from a Windows x64 build machine:

```powershell
rustup target add x86_64-pc-windows-msvc
npm run tauri build -- --target x86_64-pc-windows-msvc --bundles msi
```

Local macOS builds use an ad-hoc signature and are intended only for development. Public macOS builds must use the Developer ID and notarization setup below. Windows signing should be connected to the organization's certificate provider through Tauri's `bundle.windows.signCommand`. Signing identities, notarization credentials, and code-signing certificates are intentionally not stored in this repository.

## Trusted macOS distribution

macOS will warn that an app “cannot be checked for malicious software” when it is distributed with an ad-hoc signature. A public release must instead be signed with an Apple [**Developer ID Application** certificate](https://developer.apple.com/help/account/certificates/create-developer-id-certificates), submitted to [Apple's notary service](https://developer.apple.com/documentation/security/notarizing-macos-software-before-distribution), and stapled with the returned ticket. This requires a paid Apple Developer Program membership.

Create a Developer ID Application certificate in the Apple Developer portal from the Mac that will manage the certificate. Export the certificate and private key from Keychain Access as a password-protected `.p12`. Then create an App Store Connect API key with access to notarization and download its one-time `.p8` file.

Add these repository secrets in **GitHub → Oak-Digital/eavesdrop → Settings → Secrets and variables → Actions**:

| Secret | Value |
| --- | --- |
| `APPLE_CERTIFICATE` | Base64-encoded contents of the exported `.p12` |
| `APPLE_CERTIFICATE_PASSWORD` | Password chosen when exporting the `.p12` |
| `APPLE_SIGNING_IDENTITY` | Full identity, such as `Developer ID Application: Oak Digital ApS (TEAMID)` |
| `APPLE_API_ISSUER` | Issuer ID shown for the App Store Connect API key |
| `APPLE_API_KEY` | API key ID, such as `ABC123DEFG` |
| `APPLE_API_KEY_CONTENT` | Complete contents of `AuthKey_ABC123DEFG.p8` |

On macOS, copy the `.p12` value without writing another unencrypted copy:

```sh
openssl base64 -A -in DeveloperIDApplication.p12 | pbcopy
```

Copy the API private key value with:

```sh
pbcopy < AuthKey_ABC123DEFG.p8
```

When all six secrets are present, the release job imports the certificate into an ephemeral keychain, signs the universal app with the hardened runtime, notarizes the app, staples Apple's ticket, verifies it with Gatekeeper, and repeats notarization and verification for the final DMG. Until they are configured, the workflow publishes an ad-hoc signed internal-pilot build and emits a warning; users must then explicitly allow that build through Gatekeeper.

## Publishing updates

Eavesdrop checks the latest public GitHub Release shortly after launch and can download, verify, install, and restart from Settings. Update archives are authenticated with Tauri's updater signature in addition to the platform installer signature.

The release workflow builds a universal macOS app and a Windows x64 MSI, creates `latest.json`, and publishes the release only after both builds succeed. The macOS build is signed and notarized when the Apple distribution secrets are configured; otherwise it uses the temporary ad-hoc pilot signature. To publish:

```sh
npm run version:set -- 0.2.0
npm test
cd src-tauri && cargo test && cd ..
git add .
git commit -m "Release 0.2.0"
git tag v0.2.0
git push origin main v0.2.0
```

The `TAURI_SIGNING_PRIVATE_KEY` GitHub Actions secret is also required and has already been configured for this repository. Back up the matching private key securely; it is never committed. Anyone installing version 0.1.9 or later can receive subsequent releases through the app. The first Developer ID-signed update may ask existing pilot users to grant microphone and screen-recording permissions again because the application's macOS signing identity is changing from the old ad-hoc pilot identity.

## Storage and privacy

- Metadata is stored in SQLite under the platform application-data directory.
- Each recording has its own AES-256-GCM key, wrapped by a master key held in macOS Keychain or Windows Credential Manager.
- Audio assets and recovery fragments use independently authenticated blocks with fresh nonces.
- Playback and export decrypt in memory; the app does not create durable plaintext working files.
- Whisper transcription also processes audio in memory and never sends it over the network.
- Deleted recordings remain recoverable for seven days and are then purged at startup.

## Source layout

- `src/`: React/TypeScript library, onboarding, settings, and quick recorder surfaces.
- `src-tauri/src/audio/`: normalization, mixing, echo suppression, AAC encoding, and M4A muxing.
- `src-tauri/src/platform/`: ScreenCaptureKit/CoreAudio and WASAPI capture backends.
- `src-tauri/src/crypto.rs`: key management and encrypted block storage.
- `src-tauri/src/storage.rs`: SQLite schema, migrations, and library queries.
- `src-tauri/src/detection.rs`: debounced Zoom, Teams, and Google Meet detection.
- `src-tauri/src/destinations.rs`: local export plus the future project-delivery boundary.
