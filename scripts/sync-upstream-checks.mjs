import { test } from "node:test";
import assert from "node:assert/strict";
import { mkdtempSync, writeFileSync, readFileSync, rmSync } from "node:fs";
import { tmpdir } from "node:os";
import { join, resolve, sep } from "node:path";
import { spawnSync } from "node:child_process";
import { syncUpstream } from "./sync-upstream.mjs";

function fixture(t) {
  const root = mkdtempSync(join(tmpdir(), "pebble-sync-test-"));
  t.after(() => {
    const target = resolve(root);
    assert.ok(target.startsWith(resolve(tmpdir()) + sep));
    assert.ok(target.split(sep).at(-1).startsWith("pebble-sync-test-"));
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
  const identity = (cwd) => {
    git(cwd, "config", "user.name", "Fixture");
    git(cwd, "config", "user.email", "fixture@example.invalid");
    git(cwd, "config", "commit.gpgsign", "false");
    git(cwd, "config", "core.autocrlf", "false");
  };
  identity(author);
  const commit = (cwd, file, text) => {
    writeFileSync(join(cwd, file), text);
    git(cwd, "add", file);
    git(cwd, "commit", "-m", `Update ${file}`);
    return git(cwd, "rev-parse", "HEAD");
  };
  const base = commit(author, "app.txt", "base\n");
  git(author, "push", "origin", "master");
  git(author, "push", origin, "master");
  git(root, "clone", "--config", "core.autocrlf=false", origin, local);
  identity(local);
  git(local, "remote", "add", "upstream", upstream);
  git(local, "switch", "-c", "personal");
  commit(local, "fix.txt", "personal fix\n");
  git(local, "push", "-u", "origin", "personal");
  const sync = (apply = true) => syncUpstream(local, { apply, log: () => {} });
  return { git, upstream, origin, author, local, commit, base, sync };
}

test("no-op keeps the upstream mirror and personal patch separate", (t) => {
  const f = fixture(t);
  assert.equal(f.sync().changed, false);
  assert.equal(f.git(f.local, "rev-parse", "master"), f.base);
  assert.notEqual(f.git(f.local, "rev-parse", "personal"), f.base);
});

test("merges upstream without losing fixes or pushing untested changes", (t) => {
  const f = fixture(t);
  const upstream = f.commit(f.author, "new.txt", "upstream update\n");
  f.git(f.author, "push", "origin", "master");
  assert.equal(f.sync(false).changed, false);
  assert.equal(f.git(f.local, "rev-parse", "master"), f.base);
  const result = f.sync();
  assert.ok(result.changed && result.backup);
  assert.equal(f.git(f.local, "rev-parse", "master"), upstream);
  assert.equal(readFileSync(join(f.local, "fix.txt"), "utf8"), "personal fix\n");
  assert.equal(readFileSync(join(f.local, "new.txt"), "utf8"), "upstream update\n");
  assert.equal(f.git(f.origin, "rev-parse", "master"), f.base);
});

test("refuses to overwrite uncommitted work", (t) => {
  const f = fixture(t);
  writeFileSync(join(f.local, "fix.txt"), "unfinished\n");
  assert.throws(() => f.sync(), /must be clean/);
  assert.equal(readFileSync(join(f.local, "fix.txt"), "utf8"), "unfinished\n");
});

test("refuses a master branch containing personal changes", (t) => {
  const f = fixture(t);
  f.git(f.local, "branch", "-f", "master", "personal");
  assert.throws(() => f.sync(), /master has diverged/);
});

test("refuses to discard another checkout's personal commits", (t) => {
  const f = fixture(t);
  f.git(f.author, "fetch", f.origin, "personal");
  f.git(f.author, "switch", "-c", "personal", "FETCH_HEAD");
  f.commit(f.author, "other.txt", "another checkout\n");
  f.git(f.author, "push", f.origin, "personal");
  assert.throws(() => f.sync(), /origin\/personal has changes/);
});

test("refuses to mutate master when it is checked out in another worktree", (t) => {
  const f = fixture(t);
  const checkout = join(f.local, "..", "master-checkout");
  f.git(f.local, "worktree", "add", checkout, "master");
  assert.throws(() => f.sync(), /master is checked out in another worktree/);
});

test("a merge conflict preserves the mirror, remote and a recovery branch", (t) => {
  const f = fixture(t);
  f.commit(f.local, "app.txt", "personal edit\n");
  const oldPersonal = f.git(f.local, "rev-parse", "personal");
  f.commit(f.author, "app.txt", "upstream edit\n");
  f.git(f.author, "push", "origin", "master");
  assert.throws(() => f.sync(), /Merge did not complete/);
  assert.equal(f.git(f.local, "rev-parse", "master"), f.base);
  assert.equal(f.git(f.origin, "rev-parse", "master"), f.base);
  const backup = f.git(f.local, "for-each-ref", "--format=%(objectname)", "refs/heads/backup/");
  assert.equal(backup, oldPersonal);
  f.git(f.local, "merge", "--abort");
  assert.equal(f.git(f.local, "rev-parse", "personal"), oldPersonal);
});
