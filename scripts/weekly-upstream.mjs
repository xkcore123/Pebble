#!/usr/bin/env node
import { spawnSync } from "node:child_process";
import { appendFileSync } from "node:fs";
import { pathToFileURL } from "node:url";
import { pebblePatch } from "./pebble-patch.mjs";

// The URL override is for local bare-repository fixtures only. Production CLI
// always fetches QingJ01/Pebble and only pushes this checkout's origin/master.
export function syncWeekly(cwd, {
  upstream = "https://github.com/QingJ01/Pebble.git",
  log = console.log,
} = {}) {
  function git(args, optional = false) {
    const result = spawnSync("git", args, { cwd, encoding: "utf8", windowsHide: true });
    if (result.error) throw result.error;
    if (result.status !== 0 && !optional) {
      throw new Error(result.stderr.trim() || result.stdout.trim() || "Git command failed");
    }
    return { ok: result.status === 0, text: result.stdout.trim() };
  }
  const rev = (ref) => git(["rev-parse", "--verify", ref]).text;
  const ancestor = (a, b) => git(["merge-base", "--is-ancestor", a, b], true).ok;

  if (git(["branch", "--show-current"]).text !== "master") throw new Error("Sync requires master");
  if (git(["status", "--porcelain"]).text || git(["rev-parse", "-q", "--verify", "MERGE_HEAD"], true).ok) {
    throw new Error("Sync requires a clean, unpatched working tree, with no merge in progress");
  }
  git(["fetch", "--no-tags", "origin", "refs/heads/master"]);
  const remote = rev("FETCH_HEAD");
  const before = rev("HEAD");
  if (before !== remote) throw new Error("Local master must exactly match origin/master before automatic sync");
  git(["fetch", "--no-tags", upstream, "refs/heads/master"]);
  const upstreamSha = rev("FETCH_HEAD");

  if (ancestor(upstreamSha, before)) {
    // Also checks the patch for a manually requested recovery build.
    pebblePatch(cwd, "check", log);
    log("No upstream updates; scheduled build will be skipped.");
    return { changed: false, sha: before, upstreamSha };
  }
  if (!git(["merge-base", before, upstreamSha], true).ok) {
    throw new Error("Upstream history is unrelated; refusing automatic integration");
  }
  if (!git(["merge", "--no-edit", upstreamSha], true).ok) {
    throw new Error("Upstream merge failed. Nothing pushed; resolve conflicts manually before retrying.");
  }
  // Validate without applying: source in the maintained branch stays upstream.
  // Failure leaves a local-only merge in this disposable checkout, never origin.
  pebblePatch(cwd, "check", log);
  const sha = rev("HEAD");
  git(["push", "origin", "HEAD:refs/heads/master"]);
  log(`Synced ${upstreamSha}; build exact commit ${sha}`);
  return { changed: true, sha, upstreamSha };
}

if (process.argv[1] && import.meta.url === pathToFileURL(process.argv[1]).href) {
  try {
    if (process.env.GITHUB_ACTIONS !== "true" || process.env.GITHUB_REPOSITORY !== "xkcore123/Pebble") {
      throw new Error("Run this publishing script via Weekly Upstream Sync in xkcore123/Pebble");
    }
    const result = syncWeekly(process.cwd());
    if (process.env.GITHUB_OUTPUT) appendFileSync(process.env.GITHUB_OUTPUT,
      `changed=${result.changed}\nsha=${result.sha}\n`);
    if (process.env.GITHUB_STEP_SUMMARY) appendFileSync(process.env.GITHUB_STEP_SUMMARY,
      `### ${result.changed ? "Upstream synced; patched build follows" : "Already up to date; scheduled build skipped"}\n` +
      `Upstream: \`${result.upstreamSha}\`\n\nBuild source: \`${result.sha}\`\n` +
      "\nA manual rebuild request can run the build even without new commits.\n");
  } catch (error) {
    console.error(error.message);
    process.exitCode = 1;
  }
}
