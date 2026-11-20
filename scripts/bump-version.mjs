#!/usr/bin/env node
/**
 * Bumps the app version in package.json, package-lock.json, Cargo.toml,
 * and tauri.conf.json so release CI does not fail on mismatches.
 *
 * Usage: node scripts/bump-version.mjs 0.6.3
 */
import { readFileSync, writeFileSync } from "node:fs";
import { spawnSync } from "node:child_process";
import { fileURLToPath } from "node:url";
import { dirname, join } from "node:path";

const root = join(dirname(fileURLToPath(import.meta.url)), "..");
const version = process.argv[2];

if (!version || !/^\d+\.\d+\.\d+$/.test(version)) {
  console.error("Usage: node scripts/bump-version.mjs <semver>");
  process.exit(1);
}

function replaceInFile(rel, pattern, replacement) {
  const path = join(root, rel);
  const before = readFileSync(path, "utf8");
  const after = before.replace(pattern, replacement);
  if (before === after) {
    throw new Error(`No version match in ${rel}`);
  }
  writeFileSync(path, after);
}

replaceInFile("package.json", /("version"\s*:\s*")[^"]+(")/, `$1${version}$2`);
replaceInFile(
  "src-tauri/tauri.conf.json",
  /("version"\s*:\s*")[^"]+(")/,
  `$1${version}$2`,
);
replaceInFile("src-tauri/Cargo.toml", /^(version\s*=\s*")[^"]+(")/m, `$1${version}$2`);

const lock = JSON.parse(readFileSync(join(root, "package-lock.json"), "utf8"));
lock.version = version;
if (lock.packages?.[""]) lock.packages[""].version = version;
writeFileSync(join(root, "package-lock.json"), `${JSON.stringify(lock, null, 2)}\n`);

const cargoUpdate = spawnSync(
  "cargo",
  ["generate-lockfile", "--manifest-path", "src-tauri/Cargo.toml"],
  { cwd: root, encoding: "utf8" },
);
if (cargoUpdate.status !== 0) {
  console.error(cargoUpdate.stderr || cargoUpdate.stdout);
  process.exit(cargoUpdate.status ?? 1);
}

console.log(`Bumped version to ${version}`);
