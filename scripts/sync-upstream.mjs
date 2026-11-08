#!/usr/bin/env node
// Keep the upstream mirror separate from the personal patch/build branch.
// No reset, force-push, tag push, automatic conflict resolution or remote writes.
import { spawnSync } from "node:child_process";
import { pathToFileURL } from "node:url";

export function syncUpstream(cwd, { apply = false, log = console.log } = {}) {
  function git(args, { optional = false } = {}) {
    const result = spawnSync("git", args, { cwd, encoding: "utf8", windowsHide: true });
    if (result.error) throw result.error;
    if (result.status !== 0 && !optional) {
      throw new Error(result.stderr.trim() || result.stdout.trim() || `git ${args.join(" ")} failed`);
    }
    return { ok: result.status === 0, text: result.stdout.trim() };
  }
  const rev = (ref) => git(["rev-parse", "--verify", ref]).text;
  const ancestor = (base, head) => git(["merge-base", "--is-ancestor", base, head], { optional: true }).ok;

  if (git(["branch", "--show-current"]).text !== "personal") {
    throw new Error("Switch to personal before syncing. Never merge personal into master.");
  }
  if (git(["status", "--porcelain"]).text || git(["rev-parse", "-q", "--verify", "MERGE_HEAD"], { optional: true }).ok) {
    throw new Error("Working tree/index must be clean, with no merge in progress. Commit or resolve your work first.");
  }
  if (git(["worktree", "list", "--porcelain"]).text.split("\n").includes("branch refs/heads/master")) {
    throw new Error("master is checked out in another worktree; leave that checkout before updating the mirror.");
  }

  // Explicit refs avoid importing release tags or other upstream branches.
  git(["fetch", "--no-tags", "upstream", "refs/heads/master:refs/remotes/upstream/master"]);
  git(["fetch", "--no-tags", "origin", "refs/heads/master:refs/remotes/origin/master",
    "refs/heads/personal:refs/remotes/origin/personal"]);
  const upstream = rev("upstream/master");
  const oldMaster = rev("master");
  const oldPersonal = rev("HEAD");
  if (!ancestor("origin/master", upstream) || !ancestor(oldMaster, upstream)) {
    throw new Error("master has diverged from upstream. Refusing to overwrite it; inspect the extra commits.");
  }
  if (!ancestor("origin/personal", oldPersonal)) {
    throw new Error("origin/personal has changes not present locally. Integrate them before syncing upstream.");
  }
  log(`Upstream: ${upstream}\nMirror:   ${oldMaster}\nPersonal: ${oldPersonal}`);
  if (!apply) {
    log("Check only. Use --apply to merge locally. This script never pushes.");
    return { changed: false, upstream };
  }

  let backup;
  if (!ancestor(upstream, oldPersonal)) {
    backup = `backup/personal-before-sync-${Date.now()}`;
    git(["branch", backup, oldPersonal]);
    log(`Saved ${backup}`);
    const merge = git(["merge", "--no-edit", upstream], { optional: true });
    if (!merge.ok) {
      throw new Error(`Merge did not complete. master and origin are unchanged.\nResolve conflicts and commit, or run git merge --abort.\nRecovery branch: ${backup}`);
    }
  }
  // Compare-and-swap after successful merge only. No history rewriting.
  if (oldMaster !== upstream) git(["update-ref", "refs/heads/master", upstream, oldMaster]);
  log("Local sync complete. Review and test, then: git push --atomic origin master personal");
  return { changed: oldMaster !== upstream || oldPersonal !== rev("HEAD"), upstream, backup };
}

if (process.argv[1] && import.meta.url === pathToFileURL(process.argv[1]).href) {
  try {
    const args = process.argv.slice(2);
    if (args.some((arg) => arg !== "--apply" && arg !== "--check")) {
      throw new Error("Usage: node scripts/sync-upstream.mjs [--check | --apply]");
    }
    if (args.includes("--check") && args.includes("--apply")) throw new Error("Choose --check or --apply, not both.");
    syncUpstream(process.cwd(), { apply: args.includes("--apply") });
  } catch (error) {
    console.error(error.message);
    process.exitCode = 1;
  }
}
