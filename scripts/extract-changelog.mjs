#!/usr/bin/env node
import { readFileSync, writeFileSync } from "node:fs";

const version = (process.argv[2] || "").replace(/^v/, "");
if (!/^\d+\.\d+\.\d+$/.test(version)) {
  console.error("Usage: node scripts/extract-changelog.mjs <version>");
  process.exit(1);
}

const text = readFileSync("CHANGELOG.md", "utf8");
const match = text.match(new RegExp(`(^## ${version}\\s*[\\r\\n]+)([\\s\\S]*?)(?=^## |(?![\\s\\S]))`, "m"));
if (!match) {
  console.error(`No CHANGELOG section for ${version}`);
  process.exit(1);
}

const body = match[2].trim();
const headings = [...text.matchAll(/^## (\d+\.\d+\.\d+)\s*$/gm)].map((m) => m[1]);
const idx = headings.indexOf(version);
const prev = idx >= 0 && idx + 1 < headings.length ? headings[idx + 1] : null;

let notes = `## What's Changed\n\n${body}\n`;
if (prev) {
  notes += `\n**Full Changelog**: https://github.com/laciodanya/easyzapret/compare/v${prev}...v${version}\n`;
}

const out = process.argv[3] || "release-notes.md";
writeFileSync(out, notes);
console.log(`Wrote ${out}`);
