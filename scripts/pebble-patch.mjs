#!/usr/bin/env node
import { spawnSync } from "node:child_process";
import { createHash } from "node:crypto";
import { readFileSync } from "node:fs";
import { join } from "node:path";
import { pathToFileURL } from "node:url";

export function pebblePatch(cwd, mode = "check", log = console.log) {
  if (!["check", "apply", "reverse"].includes(mode)) throw new Error("Unknown patch mode");
  function git(args, optional = false) {
    const result = spawnSync("git", args, { cwd, encoding: "utf8", windowsHide: true });
    if (result.error) throw result.error;
    if (result.status !== 0 && !optional) throw new Error(result.stderr.trim() || "Git command failed");
    return { ok: result.status === 0, text: result.stdout.trim() };
  }
  const manifest = JSON.parse(readFileSync(join(cwd, "patches", "manifest.json"), "utf8"));
  if (manifest.patch !== "pebble-fixes.patch") throw new Error("Unexpected patch path");
  const patch = join(cwd, "patches", manifest.patch);
  const hash = createHash("sha256").update(readFileSync(patch)).digest("hex");
  if (hash !== manifest.sha256) throw new Error("Patch checksum mismatch; review the patch and manifest together");
  const flags = mode === "reverse" ? ["--reverse"] : [];
  if (mode !== "reverse") {
    const tracked = git(["status", "--porcelain", "--untracked-files=no"]).text;
    if (tracked) throw new Error("Tracked files are modified (or patch is already applied). Commit/stash work first.");
  }
  if (!git(["apply", ...flags, "--check", patch], true).ok) {
    throw new Error("Patch does not apply cleanly. No files changed. Review upstream changes; never skip failed hunks.");
  }
  if (mode !== "check") git(["apply", ...flags, patch]);
  log(mode === "check" ? "Patch verified; source unchanged." :
    mode === "apply" ? "Fixes applied to working tree only. Do not commit the expanded source changes." :
      "Patch reversed. Inspect git status before syncing or committing.");
  return { sha256: hash, files: manifest.files.length };
}

if (process.argv[1] && import.meta.url === pathToFileURL(process.argv[1]).href) {
  try {
    const args = process.argv.slice(2);
    if (args.length > 1 || (args[0] && !["--check", "--apply", "--reverse"].includes(args[0]))) {
      throw new Error("Usage: node scripts/pebble-patch.mjs [--check | --apply | --reverse]");
    }
    pebblePatch(process.cwd(), (args[0] || "--check").slice(2));
  } catch (error) {
    console.error(error.message);
    process.exitCode = 1;
  }
}
