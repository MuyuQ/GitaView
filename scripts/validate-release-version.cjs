const fs = require("node:fs");
const path = require("node:path");

const root = path.resolve(__dirname, "..");
const packageJson = require(path.join(root, "package.json"));
const tauriConfig = require(path.join(root, "src-tauri", "tauri.conf.json"));
const cargoToml = fs.readFileSync(path.join(root, "src-tauri", "Cargo.toml"), "utf8");
const expectedTag = `v${packageJson.version}`;

if (process.env.GITHUB_REF_NAME !== expectedTag) {
  throw new Error(`Release tag must be ${expectedTag}`);
}
if (tauriConfig.version !== packageJson.version) {
  throw new Error("Tauri config version must match package.json");
}
if (!cargoToml.includes(`version = "${packageJson.version}"`)) {
  throw new Error("Cargo package version must match package.json");
}

console.log(`Validated release version ${packageJson.version}`);
