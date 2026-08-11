# Eavesdrop

Eavesdrop is a tray-first meeting recorder for macOS 13+ and Windows 10/11. It records a selected microphone alone or mixes it with computer audio, writes recoverable two-second AAC fragments, and keeps the finalized M4A library encrypted until the user explicitly plays or exports a recording.

## Run locally

Prerequisites: Node.js 20+, Rust 1.85+, the Tauri 2 platform prerequisites, and full Xcode on macOS or Visual Studio Build Tools with the Windows SDK on Windows.

```sh
npm install
npm run tauri dev
```

The first recording asks for microphone permission. Online recording on macOS also requires Screen Recording permission. The app remains available from the menu bar or notification area after its library window is closed.

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

Tauri signs the macOS bundle when an Apple Developer signing identity is available. Windows signing should be connected to the organization's certificate provider through Tauri's `bundle.windows.signCommand`. Signing identities, notarization credentials, and code-signing certificates are intentionally not stored in this repository.

## Publishing updates

Eavesdrop checks the latest public GitHub Release shortly after launch and can download, verify, install, and restart from Settings. Update archives are authenticated with Tauri's updater signature in addition to the platform installer signature.

The release workflow builds a universal macOS app and Windows x64 MSI, creates `latest.json`, and publishes the release only after both builds succeed. The macOS archive is re-signed with Eavesdrop's stable designated requirement before the updater signature is created, which preserves the app identity and privacy permissions between pilot builds. To publish:

```sh
npm run version:set -- 0.2.0
npm test
cd src-tauri && cargo test && cd ..
git add .
git commit -m "Release 0.2.0"
git tag v0.2.0
git push origin main v0.2.0
```

The `TAURI_SIGNING_PRIVATE_KEY` GitHub Actions secret is required and has already been configured for this repository. Back up the matching private key securely; it is never committed. Anyone installing version 0.1.9 or later can receive subsequent releases through the app.

The workflow currently uses Tauri's documented ad-hoc macOS identity for internal distribution. Before external distribution, configure a Developer ID Application certificate and notarization secrets, plus a Windows code-signing certificate, so the operating systems can verify the publisher as well as the updater payload.

## Storage and privacy

- Metadata is stored in SQLite under the platform application-data directory.
- Each recording has its own AES-256-GCM key, wrapped by a master key held in macOS Keychain or Windows Credential Manager.
- Audio assets and recovery fragments use independently authenticated blocks with fresh nonces.
- Playback and export decrypt in memory; the app does not create durable plaintext working files.
- Deleted recordings remain recoverable for seven days and are then purged at startup.

## Source layout

- `src/`: React/TypeScript library, onboarding, settings, and quick recorder surfaces.
- `src-tauri/src/audio/`: normalization, mixing, echo suppression, AAC encoding, and M4A muxing.
- `src-tauri/src/platform/`: ScreenCaptureKit/CoreAudio and WASAPI capture backends.
- `src-tauri/src/crypto.rs`: key management and encrypted block storage.
- `src-tauri/src/storage.rs`: SQLite schema, migrations, and library queries.
- `src-tauri/src/detection.rs`: debounced Zoom, Teams, and Google Meet detection.
- `src-tauri/src/destinations.rs`: local export plus the future project-delivery boundary.
