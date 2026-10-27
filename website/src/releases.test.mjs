import test from "node:test";
import assert from "node:assert/strict";
import { parseRelease, parseReleaseHistory, REPOSITORY } from "./releases.mjs";
const asset = (name) => ({
  name,
  browser_download_url: `${REPOSITORY}/releases/download/v1.2.3/${name}`,
});
const release = (assets) => ({ tag_name: "v1.2.3", assets });
test("selects installers and ignores signatures and updater archives", () => {
  const result = parseRelease(
    release([
      asset("app.app.tar.gz"),
      asset("app.dmg.sig"),
      asset("app.dmg"),
      asset("app.msi"),
    ]),
  );
  assert.equal(result.mac, asset("app.dmg").browser_download_url);
  assert.equal(result.windows, asset("app.msi").browser_download_url);
});
test("missing platform stays unavailable rather than manufacturing a URL", () => {
  assert.equal(parseRelease(release([asset("app.dmg")])).windows, null);
});
test("rejects external and wrong-release installer URLs", () => {
  assert.equal(
    parseRelease(
      release([
        {
          name: "app.dmg",
          browser_download_url: "https://example.com/app.dmg",
        },
      ]),
    ).mac,
    null,
  );
  assert.equal(
    parseRelease(
      release([
        {
          ...asset("app.msi"),
          browser_download_url: asset("app.msi").browser_download_url.replace(
            "v1.2.3",
            "v1.2.2",
          ),
        },
      ]),
    ).windows,
    null,
  );
});
test("rejects malformed, draft, and prerelease responses", () => {
  for (const input of [
    null,
    {},
    { message: "rate limited" },
    { ...release([]), draft: true },
    { ...release([]), prerelease: true },
  ]) {
    assert.throws(() => parseRelease(input));
  }
});

const historyEntry = (overrides = {}) => ({
  tag_name: "v1.2.3",
  published_at: "2026-10-27T10:45:31Z",
  body: "A fix.\n\n- Details with **Markdown**.",
  ...overrides,
});

test("history keeps stable releases in publication order and preserves Markdown", () => {
  const result = parseReleaseHistory([
    historyEntry({ tag_name: "v1.2.2", published_at: "2026-10-26T12:00:00Z" }),
    historyEntry(),
    historyEntry({ draft: true }),
    historyEntry({ prerelease: true }),
    null,
    {},
    historyEntry({ tag_name: " " }),
    historyEntry({ published_at: "invalid" }),
    historyEntry({ published_at: null }),
  ]);
  assert.deepEqual(result.map((r) => r.version), ["v1.2.3", "v1.2.2"]);
  assert.equal(result[0].body, historyEntry().body);
  assert.equal(result[0].publishedAt, "2026-10-27T10:45:31.000Z");
});

test("history permits missing notes and builds its own repository links", () => {
  const [result] = parseReleaseHistory([historyEntry({
    body: null,
    tag_name: "release/1.2.3",
    html_url: "https://example.com/untrusted",
  })]);
  assert.equal(result.body, "");
  assert.equal(result.notes, `${REPOSITORY}/releases/tag/release%2F1.2.3`);
});

test("unavailable or empty history triggers the saved fallback", () => {
  for (const value of [null, {}, { message: "rate limited" }, [], [historyEntry({ draft: true })]]) {
    assert.throws(() => parseReleaseHistory(value));
  }
});
