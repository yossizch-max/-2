// Regenerates SOURCE-MANIFEST.json from the actual git-tracked file tree: path, size
// and sha256 for every file except the manifest itself. This exists because the
// original manifest was a one-time hand snapshot from package creation that was never
// regenerated as the source changed - an external audit caught it describing a version
// of the source that no longer existed (wrong file count, 37 hash/size mismatches,
// missing/extra files). Run this and commit the result whenever the tracked file set
// changes meaningfully, rather than editing SOURCE-MANIFEST.json by hand.
import { execFileSync } from "node:child_process";
import fs from "node:fs";
import path from "node:path";
import { fileURLToPath } from "node:url";
import { createHash } from "node:crypto";

const root = path.resolve(path.dirname(fileURLToPath(import.meta.url)), "..");
const manifestPath = path.join(root, "SOURCE-MANIFEST.json");

const tracked = execFileSync("git", ["ls-files"], { cwd: root, encoding: "utf8" })
  .split("\n").filter(Boolean).sort();

const commit = execFileSync("git", ["rev-parse", "HEAD"], { cwd: root, encoding: "utf8" }).trim();

const files = tracked
  .filter(p => p !== "SOURCE-MANIFEST.json")
  .map(p => {
    const abs = path.join(root, p);
    const buf = fs.readFileSync(abs);
    return { path: p, size: buf.length, sha256: createHash("sha256").update(buf).digest("hex") };
  });

const manifest = {
  package: "TAHRIR_RECONSTRUCTED_CANONICAL_SOURCE_alpha16_1",
  regenerated_at: new Date().toISOString().slice(0, 10),
  regenerated_against_commit: commit,
  regenerated_by: "scripts/generate-source-manifest.mjs",
  reconstruction: true,
  manifest_excludes_itself: true,
  file_count: files.length,
  files,
};

fs.writeFileSync(manifestPath, JSON.stringify(manifest, null, 2) + "\n");
console.log(`Wrote ${manifestPath}: ${files.length} files at commit ${commit}`);
