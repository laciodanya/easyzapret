import { createHash } from "node:crypto";
import { createReadStream, existsSync, mkdirSync, readFileSync, renameSync, rmSync, statSync } from "node:fs";
import { execFile } from "node:child_process";
import { promisify } from "node:util";
import { fileURLToPath } from "node:url";

const run = promisify(execFile);
const directory = new URL("../src-tauri/vpn-components/", import.meta.url);
const catalog = JSON.parse(readFileSync(new URL("../src-tauri/vpn-assets.json", import.meta.url), "utf8"));

async function verified(path, asset) {
  if (!existsSync(path) || statSync(path).size !== asset.size) return false;
  const hash = createHash("sha256");
  for await (const chunk of createReadStream(path)) hash.update(chunk);
  return hash.digest("hex") === asset.sha256;
}

mkdirSync(directory, { recursive: true });
await Promise.all(catalog.map(async (asset) => {
  const path = fileURLToPath(new URL(asset.archive, directory));
  if (!(await verified(path, asset))) {
    const temporaryPath = path + ".partial";
    try {
      await run("curl", [
        "--fail", "--silent", "--show-error", "--location", "--proto", "=https",
        "--proto-redir", "=https", "--connect-timeout", "10", "--max-time", "90",
        "--retry", "2", "--retry-max-time", "150", "--max-filesize", String(asset.size),
        "--output", temporaryPath, asset.url,
      ]);
      if (!(await verified(temporaryPath, asset))) throw new Error(`Incorrect SHA-256 or size for ${asset.archive}`);
      rmSync(path, { force: true });
      renameSync(temporaryPath, path);
    } finally {
      rmSync(temporaryPath, { force: true });
    }
  }
  console.log(`Verified ${asset.archive}`);
}));
