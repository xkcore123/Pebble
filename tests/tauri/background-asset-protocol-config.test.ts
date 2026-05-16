import { readFileSync } from "node:fs";
import { resolve } from "node:path";
import { describe, expect, it } from "vitest";

describe("background image asset protocol config", () => {
  it("enables the asset protocol for imported background images", () => {
    const config = JSON.parse(
      readFileSync(resolve(process.cwd(), "src-tauri", "tauri.conf.json"), "utf8"),
    );

    expect(config.app.security.assetProtocol).toMatchObject({
      enable: true,
      scope: ["$APPDATA/backgrounds/**/*"],
    });
    expect(config.app.security.csp).toContain("asset:");
    expect(config.app.security.csp).toContain("http://asset.localhost");
  });

  it("enables the matching tauri protocol-asset cargo feature", () => {
    const cargo = readFileSync(resolve(process.cwd(), "Cargo.toml"), "utf8");

    expect(cargo).toMatch(/tauri\s*=\s*\{[^}]*features\s*=\s*\[[^\]]*"protocol-asset"/s);
  });
});
