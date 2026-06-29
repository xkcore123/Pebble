import { readFileSync } from "node:fs";
import { resolve } from "node:path";
import { pathToFileURL } from "node:url";
import { describe, expect, it } from "vitest";

describe("Windows release configuration", () => {
  it("keeps release version metadata in sync", () => {
    const packageJson = JSON.parse(readFileSync(resolve(process.cwd(), "package.json"), "utf8"));
    const tauriConfig = JSON.parse(
      readFileSync(resolve(process.cwd(), "src-tauri", "tauri.conf.json"), "utf8"),
    );
    const cargoToml = readFileSync(resolve(process.cwd(), "src-tauri", "Cargo.toml"), "utf8");
    const changelog = readFileSync(resolve(process.cwd(), "CHANGELOG.md"), "utf8");
    const releaseWorkflow = readFileSync(resolve(process.cwd(), ".github", "workflows", "release.yml"), "utf8");
    const cargoVersion = cargoToml.match(/^version\s*=\s*"([^"]+)"/m)?.[1];

    expect(packageJson.version).toBe("0.1.2");
    expect(tauriConfig.version).toBe(packageJson.version);
    expect(cargoVersion).toBe(packageJson.version);
    expect(changelog).toContain(`## [${packageJson.version}] - `);
    expect(changelog).toContain(`[Unreleased]: https://github.com/QingJ01/Pebble/compare/v${packageJson.version}...HEAD`);
    expect(releaseWorkflow).toContain(`default: v${packageJson.version}`);
  });

  it("defines only the Windows desktop build script", () => {
    const packageJson = JSON.parse(readFileSync(resolve(process.cwd(), "package.json"), "utf8"));

    expect(packageJson.scripts["build:windows"]).toBe("tauri build --bundles nsis");
    expect(packageJson.scripts["build:macos"]).toBeUndefined();
    expect(packageJson.scripts["build:linux"]).toBeUndefined();
  });

  it("routes the generic build command to the Windows NSIS bundle only", async () => {
    const packageJson = JSON.parse(readFileSync(resolve(process.cwd(), "package.json"), "utf8"));
    const buildScriptPath = resolve(process.cwd(), "scripts", "build-tauri.mjs");
    const buildScriptSource = readFileSync(buildScriptPath, "utf8");
    const buildScript = await import(pathToFileURL(buildScriptPath).href);

    expect(packageJson.scripts.build).toBe("node scripts/build-tauri.mjs");
    expect(buildScriptSource).not.toMatch(/^#!/);
    expect(buildScript.bundleTargetsForPlatform("win32")).toBe("nsis");
    expect(() => buildScript.bundleTargetsForPlatform("darwin")).toThrow(/only builds Windows/);
    expect(() => buildScript.bundleTargetsForPlatform("linux")).toThrow(/only builds Windows/);
  });

  it("runs package builds only on Windows in CI", () => {
    const ciWorkflow = readFileSync(resolve(process.cwd(), ".github", "workflows", "ci.yml"), "utf8");

    expect(ciWorkflow).toContain("runs-on: windows-latest");
    expect(ciWorkflow).toContain("pnpm build:windows");
    expect(ciWorkflow).not.toContain("macos-15");
    expect(ciWorkflow).not.toContain("ubuntu-latest");
    expect(ciWorkflow).not.toContain("build:macos");
    expect(ciWorkflow).not.toContain("build:linux");
    expect(ciWorkflow).not.toContain("Upload Linux package artifacts");
  });

  it("uploads only Windows artifacts during tagged releases", () => {
    const releaseWorkflow = readFileSync(
      resolve(process.cwd(), ".github", "workflows", "release.yml"),
      "utf8",
    );

    expect(releaseWorkflow).toContain("Windows Release");
    expect(releaseWorkflow).toContain("runs-on: windows-latest");
    expect(releaseWorkflow).toContain("pnpm build:windows");
    expect(releaseWorkflow).toContain("target/release/bundle/nsis");
    expect(releaseWorkflow).toContain("pebble-windows-${{ env.PEBBLE_VERSION }}");
    expect(releaseWorkflow).toContain("vMAJOR.MINOR.PATCH-patched.YYYYMMDDHHMMSS");
    expect(releaseWorkflow).toContain("--prerelease");
    expect(releaseWorkflow).not.toContain("Linux Package Release");
    expect(releaseWorkflow).not.toContain("macOS Release");
    expect(releaseWorkflow).not.toContain("*.AppImage");
    expect(releaseWorkflow).not.toContain("*.dmg");
  });

  it("syncs upstream weekly and dispatches Windows releases only after updates", () => {
    const autoPatchWorkflow = readFileSync(
      resolve(process.cwd(), ".github", "workflows", "auto-patch.yml"),
      "utf8",
    );

    expect(autoPatchWorkflow).toContain('cron: "17 20 * * 0"');
    expect(autoPatchWorkflow).toContain("actions: write");
    expect(autoPatchWorkflow).toContain("id: sync");
    expect(autoPatchWorkflow).toContain("steps.sync.outputs.upstream_changed == 'true'");
    expect(autoPatchWorkflow).toContain("gh workflow run release.yml");
    expect(autoPatchWorkflow).toContain("patched.");
  });
});
