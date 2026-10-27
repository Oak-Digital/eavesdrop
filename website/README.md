# Eavesdrop website

A standalone React + Vite landing page for Eavesdrop. It has its own dependencies and build, so deploying it does not require Rust, Tauri, or any changes to the desktop application.

## Local development

Use Node.js 22.12+ (or a compatible newer LTS).

From the repository root:

```sh
cd website
npm ci
npm run dev
```

## Publish to Vercel

1. Import the `Oak-Digital/eavesdrop` Git repository into a new Vercel project.
2. Set **Root Directory** to **`website`**. This matters: the repository root builds the desktop app.
3. Select the **Vite** framework preset. The included `vercel.json` sets the install command to `npm ci`, build command to `npm run build`, and output directory to `dist`.
4. Deploy. No environment variables, server functions, or credentials are required.

Alternatively, from this directory, run `npx vercel` for a preview deployment and `npx vercel --prod` when ready to publish.

To verify the production build locally:

```sh
npm run build
npm run preview
```

This is a single-page, static site with section anchors; it does not need catch-all route rewrites. The website is prepared for deployment but is not automatically published by this implementation.

## Downloads

The page requests the latest public stable release from the GitHub API and selects the actual `.dmg` and `.msi` assets. It never guesses versioned filenames or offers updater archives as installers. Missing platforms link to the release page instead.

A six-second timeout and a verified release snapshot keep downloads available when GitHub is unavailable or rate-limited. In that case the page says “Available release” rather than claiming it is the latest. `src/release-snapshot.json` currently contains verified v0.1.22 URLs; refresh it when retiring older releases. `src/releases.mjs` holds the repository and API configuration.

```sh
npm test
```

Tests cover installer selection, missing platforms, malformed release responses, draft/prerelease rejection, and foreign or mismatched release URLs.

## Content and loading

### Social sharing

`index.html` includes canonical, Open Graph, and Twitter large-image card metadata in the initial HTML so crawlers do not need JavaScript. The public URL is `https://eavesdrop.oak.dk/`; update the canonical, `og:url`, and both image URLs together if the domain changes.

`public/og-image.jpg` is the 1200 × 630 share image. Its editable source is `scripts/share-image.html`, using the bundled Geist fonts, original ASCII waveform, and circular Oak logo. To refresh it, run `npm run dev`, open `/scripts/share-image.html`, let the fonts load, and capture the top-left 1200 × 630 pixels at 1× scale as JPEG. The source page is not an entry point in the production build. If the image dimensions or format change, update the image metadata in `index.html` too.

### Recorder hero

The hero is a custom isometric tape recorder built with the [Hairline creation skill](https://github.com/lucasmarkes/hairline/tree/main/skills/hairline-create). Moving across it scrubs the reels and transfers tape between them. Arrow keys, Home, and End also scrub; the idle tour moves forward, rewinds, and returns to rest. A Pause button stops the tour. Reduced-motion preferences disable autoplay, and Hairline's shared loop sleeps offscreen.

- Edit `scripts/hairline/recorder.js` for geometry, interaction, and tour stops; `src/RecorderHero.tsx` owns the React lifecycle and playback control.
- `npm run hero:build` validates the figure and generates the ESM adapter plus a standalone `scripts/hairline/hairline-recorder.html` preview. It runs automatically before development and production builds. Generated files are ignored by Git.
- `scripts/hairline/vendor/` preserves the upstream kernel, bench, builder, and validator unchanged from Hairline commit `37d2e78dcb333ab30c3338e28ed4a690364ea014`, under the included MIT license. There is no runtime CDN dependency.
- The theme uses Hairline's public CSS variables in `src/styles.css`. The illustration does not capture or play audio.

For Hairline's eight-view visual check, run its `skills/hairline-create/look.mjs` from `scripts/hairline/` with `recorder.js --answer 300,175 --edge 65,175 --edge 335,175`. It produces a sheet and motion frames to inspect; the static validator alone cannot judge appearance.

### Page files

- `src/App.tsx`: presentation, privacy, download lookup, and FAQs.
- `src/Demo.tsx`: keyboard-accessible recording/summary/transcript preview using fictional example content. It does not capture or play audio.
- `src/styles.css`: responsive paper-and-ink design, using locally bundled Geist fonts.
- `index.html`: title, search/social descriptions, favicon, and a no-JavaScript download fallback.
- `ascii-reveal` supplies `AsciiSkeleton` during the real release request and lazy preview load, and `AsciiReveal` when content arrives. There are no artificial loading delays. The package and site respect reduced-motion preferences.

The loading treatments are inspired by [ascii-reveal](https://quanhoangindex.github.io/ascii-reveal/); the recorder is custom Hairline geometry, and the privacy illustration is native text. Product claims are based on the app source and repository documentation, including the distinction between local processing and optional cloud summaries.
