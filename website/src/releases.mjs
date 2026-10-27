export const REPOSITORY = "https://github.com/Oak-Digital/eavesdrop";
export const RELEASES_URL = `${REPOSITORY}/releases/latest`;
export const RELEASE_API =
  "https://api.github.com/repos/Oak-Digital/eavesdrop/releases/latest";
export const RELEASE_HISTORY_API =
  "https://api.github.com/repos/Oak-Digital/eavesdrop/releases?per_page=10";

/** Keep published stable releases, newest first, without trusting supplied URLs. */
export function parseReleaseHistory(value) {
  if (!Array.isArray(value)) throw new Error("Release history unavailable");
  const releases = value
    .filter(
      (release) =>
        release &&
        typeof release.tag_name === "string" &&
        release.tag_name.trim() &&
        !release.draft &&
        !release.prerelease &&
        typeof release.published_at === "string" &&
        Number.isFinite(Date.parse(release.published_at)),
    )
    .map((release) => ({
      version: release.tag_name,
      publishedAt: new Date(release.published_at).toISOString(),
      body: typeof release.body === "string" ? release.body.trim() : "",
      notes: `${REPOSITORY}/releases/tag/${encodeURIComponent(release.tag_name)}`,
    }))
    .sort((a, b) => Date.parse(b.publishedAt) - Date.parse(a.publishedAt));
  if (!releases.length) throw new Error("No published releases available");
  return releases;
}

/** Only offer actual public installers from this repository, never updater archives. */
export function parseRelease(value) {
  if (
    !value ||
    typeof value.tag_name !== "string" ||
    value.draft ||
    value.prerelease ||
    !Array.isArray(value.assets)
  ) {
    throw new Error("No stable release available");
  }
  const prefix = `${REPOSITORY}/releases/download/${encodeURIComponent(value.tag_name)}/`;
  const installer = (suffix) =>
    value.assets.find(
      (asset) =>
        typeof asset.name === "string" &&
        asset.name.toLowerCase().endsWith(suffix) &&
        typeof asset.browser_download_url === "string" &&
        asset.browser_download_url.startsWith(prefix),
    )?.browser_download_url ?? null;
  return {
    version: value.tag_name,
    mac: installer(".dmg"),
    windows: installer(".msi"),
    notes: `${REPOSITORY}/releases/tag/${encodeURIComponent(value.tag_name)}`,
  };
}
