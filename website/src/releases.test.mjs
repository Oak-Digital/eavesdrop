import test from "node:test";
import assert from "node:assert/strict";
import { parseRelease, REPOSITORY } from "./releases.mjs";
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
