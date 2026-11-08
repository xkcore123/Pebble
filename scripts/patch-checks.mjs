import { test } from "node:test";
import assert from "node:assert/strict";
import { spawnSync } from "node:child_process";
import { mkdtempSync, mkdirSync, writeFileSync, readFileSync, rmSync } from "node:fs";
import { createHash } from "node:crypto";
import { join, resolve, sep } from "node:path";
import { tmpdir } from "node:os";
import { pebblePatch } from "./pebble-patch.mjs";

function fixture(t) {
  const cwd = mkdtempSync(join(tmpdir(), "pebble-patch-test-"));
  t.after(() => {
    const target = resolve(cwd);
    assert.ok(target.startsWith(resolve(tmpdir()) + sep));
    assert.ok(target.split(sep).at(-1).startsWith("pebble-patch-test-"));
    rmSync(target, { recursive: true, force: true });
  });
  function git(...args) {
    const r = spawnSync("git", args, { cwd, encoding: "utf8", windowsHide: true });
    assert.equal(r.status, 0, r.stderr);
    return r.stdout;
  }
  git("init", "--initial-branch=master");
  git("config", "user.name", "Fixture");
  git("config", "user.email", "fixture@example.invalid");
  git("config", "core.autocrlf", "false");
  git("config", "commit.gpgsign", "false");
  writeFileSync(join(cwd, "app.txt"), "upstream\n");
  git("add", "app.txt");
  git("commit", "-m", "Upstream fixture");
  writeFileSync(join(cwd, "app.txt"), "fixed\n");
  const patch = git("diff", "--binary", "--full-index");
  writeFileSync(join(cwd, "app.txt"), "upstream\n");
  mkdirSync(join(cwd, "patches"));
  writeFileSync(join(cwd, "patches", "pebble-fixes.patch"), patch);
  writeFileSync(join(cwd, "patches", "manifest.json"), JSON.stringify({
    patch: "pebble-fixes.patch", sha256: createHash("sha256").update(patch).digest("hex"), files: ["app.txt"],
  }));
  git("add", "patches");
  git("commit", "-m", "Store independent patch");
  return { cwd, git, run: (mode) => pebblePatch(cwd, mode, () => {}),
    content: () => readFileSync(join(cwd, "app.txt"), "utf8") };
}

test("check does not modify source; apply and reverse reproduce original", (t) => {
  const f = fixture(t);
  f.run("check");
  assert.equal(f.content(), "upstream\n");
  f.run("apply");
  assert.equal(f.content(), "fixed\n");
  assert.equal(f.git("show", "HEAD:app.txt"), "upstream\n");
  f.run("reverse");
  assert.equal(f.content(), "upstream\n");
  assert.equal(f.git("status", "--porcelain"), "");
});

test("duplicate application fails without changing source", (t) => {
  const f = fixture(t);
  f.run("apply");
  assert.throws(() => f.run("apply"), /already applied/);
  assert.equal(f.content(), "fixed\n");
});

test("upstream conflicts fail rather than silently dropping a fix", (t) => {
  const f = fixture(t);
  writeFileSync(join(f.cwd, "app.txt"), "changed upstream\n");
  f.git("add", "app.txt");
  f.git("commit", "-m", "Changed upstream");
  assert.throws(() => f.run("apply"), /does not apply cleanly/);
  assert.equal(f.content(), "changed upstream\n");
});

test("checksum catches a changed patch", (t) => {
  const f = fixture(t);
  writeFileSync(join(f.cwd, "patches", "pebble-fixes.patch"), "corrupt\n");
  assert.throws(() => f.run("apply"), /checksum mismatch/);
  assert.equal(f.content(), "upstream\n");
});

test("uncommitted edits are protected", (t) => {
  const f = fixture(t);
  writeFileSync(join(f.cwd, "app.txt"), "unfinished\n");
  assert.throws(() => f.run("apply"), /Tracked files are modified/);
  assert.equal(f.content(), "unfinished\n");
});
