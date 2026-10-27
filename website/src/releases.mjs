export const REPOSITORY = "https://github.com/Oak-Digital/eavesdrop";
export const RELEASES_URL = `${REPOSITORY}/releases/latest`;
export const RELEASE_API =
  "https://api.github.com/repos/Oak-Digital/eavesdrop/releases/latest";

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
