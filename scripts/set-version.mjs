import { readFileSync, writeFileSync } from "node:fs";

const version = process.argv[2];
if (!/^\d+\.\d+\.\d+(?:-[0-9A-Za-z.-]+)?$/.test(version ?? "")) {
  console.error("Usage: npm run version:set -- 1.2.3");
  process.exit(1);
}

function updateJson(path, update) {
  const value = JSON.parse(readFileSync(path, "utf8"));
  update(value);
  writeFileSync(path, `${JSON.stringify(value, null, 2)}\n`);
}

updateJson("package.json", (value) => { value.version = version; });
updateJson("package-lock.json", (value) => {
  value.version = version;
  value.packages[""].version = version;
});
updateJson("src-tauri/tauri.conf.json", (value) => { value.version = version; });

const cargoTomlPath = "src-tauri/Cargo.toml";
const cargoToml = readFileSync(cargoTomlPath, "utf8").replace(
  /(\[package\][\s\S]*?^version = ")[^"]+("$)/m,
  `$1${version}$2`,
);
writeFileSync(cargoTomlPath, cargoToml);

const cargoLockPath = "src-tauri/Cargo.lock";
const cargoLock = readFileSync(cargoLockPath, "utf8").replace(
  /(\[\[package\]\]\nname = "eavesdrop"\nversion = ")[^"]+("\n)/,
  `$1${version}$2`,
);
writeFileSync(cargoLockPath, cargoLock);

console.log(`Eavesdrop version set to ${version}`);
