import { test } from "node:test";
import assert from "node:assert/strict";
import { spawnSync } from "node:child_process";
import { mkdtempSync, mkdirSync, readFileSync, writeFileSync, rmSync } from "node:fs";
import { createHash } from "node:crypto";
import { join, resolve, sep } from "node:path";
import { tmpdir } from "node:os";
import { syncWeekly } from "./weekly-upstream.mjs";

function fixture(t) {
  const root = mkdtempSync(join(tmpdir(), "pebble-weekly-test-"));
  t.after(() => {
    const target = resolve(root);
    assert.ok(target.startsWith(resolve(tmpdir()) + sep));
    assert.ok(target.split(sep).at(-1).startsWith("pebble-weekly-test-"));
    rmSync(target, { recursive: true, force: true });
  });
  function git(cwd, ...args) {
    const result = spawnSync("git", args, { cwd, encoding: "utf8", windowsHide: true });
    assert.equal(result.status, 0, result.stderr);
    return result.stdout.trim();
  }
  const upstream = join(root, "upstream.git");
  const origin = join(root, "origin.git");
  const author = join(root, "author");
  const local = join(root, "local");
  git(root, "init", "--bare", "--initial-branch=master", upstream);
  git(root, "init", "--bare", "--initial-branch=master", origin);
  git(root, "clone", "--config", "core.autocrlf=false", upstream, author);
  function identity(cwd) {
    git(cwd, "config", "user.name", "Fixture");
    git(cwd, "config", "user.email", "fixture@example.invalid");
    git(cwd, "config", "commit.gpgsign", "false");
  }
  identity(author);
  function commit(cwd, path, text) {
    writeFileSync(join(cwd, path), text);
    git(cwd, "add", path);
    git(cwd, "commit", "-m", `Update ${path}`);
    return git(cwd, "rev-parse", "HEAD");
  }
  commit(author, "app.txt", "upstream\n");
  git(author, "push", "origin", "master");
  git(author, "push", origin, "master");
  git(root, "clone", "--config", "core.autocrlf=false", origin, local);
  identity(local);
  writeFileSync(join(local, "app.txt"), "fixed\n");
  const patch = git(local, "diff", "--full-index") + "\n";
  writeFileSync(join(local, "app.txt"), "upstream\n");
  mkdirSync(join(local, "patches"));
  writeFileSync(join(local, "patches", "pebble-fixes.patch"), patch);
  writeFileSync(join(local, "patches", "manifest.json"), JSON.stringify({
    patch: "pebble-fixes.patch", sha256: createHash("sha256").update(patch).digest("hex"), files: ["app.txt"],
  }));
  git(local, "add", "patches");
  git(local, "commit", "-m", "Independent patch");
  git(local, "push", "origin", "master");
  const before = git(origin, "rev-parse", "master");
  return { root, local, author, upstream, origin, before, git, commit,
    sync: () => syncWeekly(local, { upstream, log: () => {} }) };
}

test("no updates: emits false, no merge commit and no rebuild", (t) => {
  const f = fixture(t);
  const result = f.sync();
  assert.equal(result.changed, false);
  assert.equal(result.sha, f.before);
  assert.equal(f.git(f.origin, "rev-parse", "master"), f.before);
});

test("new upstream: push clean source, preserve patch, return exact build SHA", (t) => {
  const f = fixture(t);
  const upstreamSha = f.commit(f.author, "new.txt", "new upstream\n");
  f.git(f.author, "push", "origin", "master");
  const result = f.sync();
  assert.equal(result.changed, true);
  assert.equal(result.upstreamSha, upstreamSha);
  assert.equal(result.sha, f.git(f.origin, "rev-parse", "master"));
  assert.equal(f.git(f.origin, "show", "master:app.txt"), "upstream");
  assert.equal(f.git(f.origin, "show", "master:new.txt"), "new upstream");
  assert.equal(f.git(f.local, "status", "--porcelain"), "");
  assert.equal(f.git(f.origin, "for-each-ref", "--format=%(refname)", "refs/heads"), "refs/heads/master");
  assert.equal(f.sync().changed, false, "rerun must not rebuild an already-integrated update");
});

test("outdated patch: fail before changing origin", (t) => {
  const f = fixture(t);
  f.commit(f.author, "app.txt", "upstream rewrote this function\n");
  f.git(f.author, "push", "origin", "master");
  assert.throws(() => f.sync(), /Patch does not apply/);
  assert.equal(f.git(f.origin, "rev-parse", "master"), f.before);
});

test("merge conflict: fail before changing origin", (t) => {
  const f = fixture(t);
  f.commit(f.local, "config.txt", "fork\n");
  f.git(f.local, "push", "origin", "master");
  const before = f.git(f.origin, "rev-parse", "master");
  f.commit(f.author, "config.txt", "upstream\n");
  f.git(f.author, "push", "origin", "master");
  assert.throws(() => f.sync(), /merge failed/);
  assert.equal(f.git(f.origin, "rev-parse", "master"), before);
});

test("dirty checkout: never overwrite work", (t) => {
  const f = fixture(t);
  writeFileSync(join(f.local, "app.txt"), "unfinished\n");
  assert.throws(() => f.sync(), /clean, unpatched/);
  assert.equal(readFileSync(join(f.local, "app.txt"), "utf8"), "unfinished\n");
});

test("remote advanced: reject stale checkout without force pushing", (t) => {
  const f = fixture(t);
  f.git(f.author, "fetch", f.origin, "master");
  f.git(f.author, "merge", "--ff-only", "FETCH_HEAD");
  const remote = f.commit(f.author, "other.txt", "concurrent edit\n");
  f.git(f.author, "push", f.origin, "master");
  assert.throws(() => f.sync(), /exactly match origin/);
  assert.equal(f.git(f.origin, "rev-parse", "master"), remote);
});
