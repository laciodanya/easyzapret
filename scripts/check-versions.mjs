#!/usr/bin/env node
import { readFileSync } from "node:fs";
import { spawnSync } from "node:child_process";
import { fileURLToPath } from "node:url";
import { dirname, join } from "node:path";

const root = join(dirname(fileURLToPath(import.meta.url)), "..");
const packageVersion = JSON.parse(readFileSync(join(root, "package.json"), "utf8")).version;
const tauriVersion = JSON.parse(
  readFileSync(join(root, "src-tauri/tauri.conf.json"), "utf8"),
).version;
const cargoToml = readFileSync(join(root, "src-tauri/Cargo.toml"), "utf8");
const cargoMatch = cargoToml.match(/^version\s*=\s*"([^"]+)"/m);
const cargoVersion = cargoMatch?.[1];

const meta = spawnSync(
  "cargo",
  ["metadata", "--manifest-path", "src-tauri/Cargo.toml", "--no-deps", "--format-version", "1"],
  { cwd: root, encoding: "utf8" },
);
if (meta.status !== 0) {
  console.error(meta.stderr || meta.stdout);
  process.exit(1);
}
const lockVersion = JSON.parse(meta.stdout).packages[0].version;

const versions = {
  package: packageVersion,
  tauri: tauriVersion,
  cargoToml: cargoVersion,
  cargoMetadata: lockVersion,
};

const unique = new Set(Object.values(versions));
if (unique.size !== 1 || [...unique][0] == null) {
  console.error("Version mismatch:", versions);
  process.exit(1);
}

console.log(`Versions OK: ${packageVersion}`);
