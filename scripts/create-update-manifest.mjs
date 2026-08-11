import { readFileSync, writeFileSync } from "node:fs";
import { join } from "node:path";

const [version, repository, outputDirectory = "release"] = process.argv.slice(2);
if (!/^\d+\.\d+\.\d+(?:-[0-9A-Za-z.-]+)?$/.test(version ?? "") || !repository?.includes("/")) {
  console.error("Usage: node scripts/create-update-manifest.mjs 1.2.3 owner/repository [output-directory]");
  process.exit(1);
}

const macName = `Eavesdrop_${version}_universal.app.tar.gz`;
const windowsName = `Eavesdrop_${version}_x64_en-US.msi`;
const releaseBase = `https://github.com/${repository}/releases/download/v${version}`;
const signature = (name) => readFileSync(join(outputDirectory, `${name}.sig`), "utf8").trim();
const platform = (name) => ({ url: `${releaseBase}/${name}`, signature: signature(name) });

const manifest = {
  version,
  notes: `Eavesdrop ${version}`,
  pub_date: new Date().toISOString(),
  platforms: {
    "darwin-aarch64": platform(macName),
    "darwin-x86_64": platform(macName),
    "windows-x86_64": platform(windowsName),
  },
};

writeFileSync(join(outputDirectory, "latest.json"), `${JSON.stringify(manifest, null, 2)}\n`);
