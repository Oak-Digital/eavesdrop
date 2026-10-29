# Eavesdrop

Eavesdrop is a tray-first meeting recorder for macOS 13+ and Windows 10/11. It records a selected microphone alone or mixes it with computer audio, writes recoverable two-second AAC fragments, and keeps the finalized M4A library encrypted at rest.

## Run locally

Prerequisites: Node.js 20+, Rust 1.88+, CMake, the Tauri 2 platform prerequisites, and full Xcode on macOS or Visual Studio Build Tools with the Windows SDK on Windows.

```sh
npm install
npm run tauri dev
```

The first recording asks for microphone permission. Online recording on macOS also requires Screen Recording permission. The app remains available from the menu bar or notification area after its library window is closed.

## Design system

The interface draws from [ascii-reveal](https://quanhoangindex.github.io/ascii-reveal/): neutral paper/ink surfaces, fine frame lines, monochrome actions, and a static ASCII signal motif. Geist is used for interface text; Geist Mono is reserved for identity, timestamps, and metadata. Both fonts are bundled locally for offline use.

`src/styles.css` owns the shared color, typography, spacing, and radius tokens for the library, recorder, and onboarding. Use semantic tokens rather than literal component colors. Buttons and select triggers share the fully rounded `--radius-button` token. Light is the default appearance. Dark and system appearance remain available in Settings; existing preferences are preserved. Color is reserved for recording, warnings, errors, and permission status. All animation respects reduced-motion preferences. Dropdowns use the shared `src/Select.tsx` component, styled with these same tokens over Radix select primitives. Use its options and `onValueChange` API for new pickers; it handles keyboard navigation, typeahead, focus return, and viewport-aware popup positioning.

## macOS app icon

The Dock and Command-Tab icon uses a glass version of the five-bar waveform, with a frosted pearl tile and translucent orange bars with warm amber reflections. Its transparent master is `assets/icon-macos-glass.png`; regenerate the bundled `src-tauri/icons/icon.icns` on macOS with `npm run icon:mac`. The icon set includes standard and Retina representations from 16 to 1024 pixels. Rebuild and relaunch the app to see changes to the bundled icon. The flat vector mark in `assets/icon.svg` remains available for interface use.

On macOS, the library sidebar uses the native Sidebar visual-effect material with a subtle theme tint. The material follows window focus and the app’s Light, Dark, or System appearance. The main content stays opaque, and Reduce Transparency uses an opaque sidebar. Windows and browser previews retain the standard solid sidebar.

## OakOS connection

In **Integrations → OakOS**, select **Connect OakOS**, sign in in your default browser,
and approve project access and recording uploads. Return to Eavesdrop when authorization
finishes. No API key or client secret is required. You can cancel sign-in, retry after a
five-minute timeout, or select **Reconnect** if access expires or is revoked.

Access and rotating refresh credentials stay in the operating system credential store,
not in settings, recording exports, logs, or the web UI. Eavesdrop refreshes access before
it expires. **Disconnect** revokes the OAuth connection in OakOS before removing local
credentials; if the network is unavailable, retry disconnecting when online. Access can
also be revoked under OakOS **Settings → API → Connected apps**. Existing personal-token
connections continue working; a successful **Reconnect** replaces the locally stored token
with OAuth credentials. It does not revoke the old personal token in OakOS, since other
integrations may still use it.

The desktop app uses authorization code + S256 PKCE, random state, and issuer validation.
It listens only on `127.0.0.1:43821` during sign-in. If that port is occupied, close the other
sign-in attempt and retry. The browser development preview simulates connection state;
actual authentication and uploads run in the native app.

### OakOS deployment prerequisite

Deploy the coordinated `recording-integration` server support before releasing this client:

1. Apply OakOS migration `0072` for OAuth recording rate limits and idempotency receipts.
2. Set `OAKOS_MCP_ENABLED=true` and `OAKOS_NATIVE_RECORDINGS_ENABLED=true` with the canonical
   origin `https://os.oak.dk`.
3. Register the public desktop client once from the OakOS checkout using the deployment's
   database and origin configuration:
   `NODE_OPTIONS=--conditions=react-server npx tsx scripts/register-recording-client.ts eavesdrop-desktop`.
   The script does not overwrite existing registrations.
4. Verify sign-in, project selection, a synthetic recording upload, refresh, disconnect,
   and reconnect against the deployed service before distributing the desktop update.

The registered client is `eavesdrop-desktop` with the exact redirect
`http://127.0.0.1:43821/oauth/oakos`, resource `https://os.oak.dk/api/v1`, and scopes
`projects:read recordings:write offline_access`. It has no recording-read, Task, or MCP
permissions. OAuth uses the existing `/api/auth/oauth2/authorize` and
`/api/auth/oauth2/token` endpoints. Connection validation/revocation uses
`GET` / `DELETE /api/v1/integrations/eavesdrop/connection`.

## Automatic processing

**Settings → Transcription → Auto transcribe** starts transcription after a new recording is saved. **Settings → Summaries → Auto summarize** starts a summary after a successful transcription, including manual transcription. Both default to off, persist across restarts, and use the selected default processing source, model, and summary template. Enabling them does not process older recordings.

Automatic work runs in the native app even with the library hidden and waits for other transcription or summary work to finish. Disabling a toggle skips waiting work; a request already running finishes normally. Errors appear in the library and can be retried manually. Pending work is not resumed after quitting the app. With Cloud selected, the enabled toggle sends audio or transcripts to the selected provider automatically and provider charges may apply.

## Cloud transcription

Settings → Transcription lets you choose Local or Cloud as the default. Cloud uses OpenAI's `gpt-transcribe` with your API key, shared with OpenAI summaries. Each recording also has its own processing selector, and cloud actions explicitly state that audio is uploaded and provider charges may apply. Local Whisper remains the default after upgrading.

Cloud audio stays in memory: recordings under 24 MB are uploaded as M4A, while larger recordings are decoded and split into 16 kHz mono WAV chunks of at most ten minutes, preferring quiet split points. No plaintext temporary audio files are written. Requests are not automatically retried, and a failed request or chunk leaves the previous transcript intact. The resulting text and detected languages are saved with provider/model provenance. Cloud transcripts are displayed without fabricated timestamps.

API reference: [OpenAI file transcription](https://developers.openai.com/api/docs/guides/speech-to-text).

## Local transcription

Eavesdrop embeds the open-source [`whisper.cpp`](https://github.com/ggml-org/whisper.cpp) engine. Transcription happens on-device: decrypted audio is decoded and processed in memory and is never uploaded or written to a plaintext working file.

1. Open **Settings → Transcription**, choose Tiny, Base, or Small under **Local transcription**, and select **Install**. Base is the recommended starting point.
2. Eavesdrop downloads the model from the [official whisper.cpp model repository](https://huggingface.co/ggerganov/whisper.cpp/tree/main), verifies its published fingerprint, and selects it automatically.
3. Open a finished recording and select **Transcribe**. Eavesdrop saves the timestamped transcript in its local library.

Larger models are generally more accurate but need more memory and take longer to run. An existing whisper.cpp-compatible `.bin` model can still be selected manually.

## Local and Cloud summary models

**Settings → Summary models** groups downloadable models under **Local** and API models under **Cloud**. Local is the initial default and processes transcripts on your computer. Install a bundled model or choose an existing GGUF file. Selecting a Local model makes Local the default again.

For Cloud, choose OpenAI or Anthropic, pick a model (or enter a custom model ID), save your API key, then select **Use Cloud model**. Keys are stored in the operating system credential store, separately from settings and library exports. You can replace or remove each provider's key. Saving a key does not verify provider access; authentication, billing, and model access are checked on generation. Only models supported by the provider's text-generation endpoint can be used.

Every recording has a **Local / Cloud** processing picker. Cloud clearly names the provider and sends the transcript text, summary instructions, and language to that provider when you generate a summary manually or enable Auto summarize; provider charges and data policies apply. Transcription has its own Local / Cloud setting, and audio is not sent to summary providers. There is no automatic fallback from Local to Cloud. Failed, refused, or truncated Cloud responses leave an existing summary unchanged. OpenAI requests use `store: false`; this does not override the provider's other retention policies.

Cloud integrations use the [OpenAI Responses API](https://developers.openai.com/api/reference/resources/responses/methods/create) and [Anthropic Messages API](https://platform.claude.com/docs/en/api/messages/create). The browser development preview simulates Cloud calls and key presence without retaining API keys or making provider requests; native builds perform the actual calls.

## Summary templates

Choose a template in a recording's **Summary** section before selecting **Summarize** or **Summarize again**. **Discussion summary** is the initial default: a detailed thematic record with conclusions, supporting discussion, agreed actions, proposed follow-up, and unresolved questions, based on the Plaud-style example. It follows the meeting's language and targets 2,000–3,000 words for a substantive hour-long meeting, adjusting to the available content.

**Concise summary** keeps the short overview/key points/decisions/actions format. **Action items** focuses on commitments, owners, deadlines, and open decisions. **Custom prompt** uses the saved instructions in Settings with the structured concise layout. Existing custom prompts and older summaries are preserved.

Change the default under **Settings → Summary models → Default summary template** and select **Save template**. A recording's picker overrides that choice for that generation only; saved summaries remember which template produced them. Changing a template does not rewrite a summary until you generate it again.

The shared preset catalog is `src/summaryTemplates.json`, compiled into both the UI and the local summary engine. Discussion summaries retain Markdown and have larger note, output, and context budgets than concise summaries. They can take longer and use more memory; output quality still depends on the selected model. All templates also work with Cloud models. Build the sidecar before Rust app checks or packaging: `npm run summarizer:build -- aarch64-apple-darwin` (substitute the target platform).

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
npm run tauri build -- --target universal-apple-darwin --bundles app
# Keep the development identity stable across unsigned pilot builds:
./scripts/sign-macos-local.sh src-tauri/target/universal-apple-darwin/release/bundle/macos/Eavesdrop.app
./scripts/build-macos-dmg.sh src-tauri/target/universal-apple-darwin/release/bundle/macos/Eavesdrop.app release/Eavesdrop_universal.dmg
```

The DMG opens in icon view with Eavesdrop on the left, an Applications shortcut on the right, and a drag-to-install arrow and instructions between them. After copying, eject the disk image and launch Eavesdrop from Applications. Both signed and ad-hoc releases use the same DMG packaging script; signing and notarization of the release DMG happen after packaging.

DMG packaging requires Python 3.9 or later and network access to install the pinned packages in `scripts/dmg-requirements.txt` into a temporary virtual environment. Finder layout is written directly, so packaging also works on CI without Finder automation. To regenerate the installation background, run `swift scripts/build-dmg-background.swift assets/dmg-background.tiff`; its layout coordinates are shared with `scripts/dmg-settings.py`.

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
- Local Whisper transcription processes audio in memory and never sends it over the network. Optional cloud transcription sends audio to OpenAI when requested manually or through the opt-in Auto transcribe setting; its API key is shared with OpenAI summaries and kept in the system credential store.
- Deleting one or more recordings requires confirmation and permanently removes their local audio, recovery files, transcripts, summaries, highlights, and delivery history. Previously recoverable items from Recently Deleted return to the library on upgrade so they can be explicitly deleted.

## Source layout

- `src/`: React/TypeScript library, onboarding, settings, and quick recorder surfaces.
- `src-tauri/src/audio/`: normalization, mixing, echo suppression, AAC encoding, and M4A muxing.
- `src-tauri/src/platform/`: ScreenCaptureKit/CoreAudio and WASAPI capture backends.
- `src-tauri/src/crypto.rs`: key management and encrypted block storage.
- `src-tauri/src/storage.rs`: SQLite schema, migrations, and library queries.
- `src-tauri/src/detection.rs`: debounced Zoom, Teams, and Google Meet detection.
- `src-tauri/src/destinations.rs`: local export plus the future project-delivery boundary.
