import { readFileSync } from "node:fs";
import { resolve } from "node:path";
import { describe, expect, it } from "vitest";

describe("Linux AppImage WebKit runtime workaround", () => {
  it("configures WebKitGTK compositing before Tauri initializes", () => {
    const mainSource = readFileSync(resolve(process.cwd(), "src-tauri", "src", "main.rs"), "utf8");
    const configureIndex = mainSource.indexOf("configure_linux_appimage_webkit_runtime();");
    const runIndex = mainSource.indexOf("pebble_lib::run();");

    expect(mainSource).toContain("APPIMAGE");
    expect(mainSource).toContain("WEBKIT_DISABLE_COMPOSITING_MODE");
    expect(configureIndex).toBeGreaterThanOrEqual(0);
    expect(runIndex).toBeGreaterThan(configureIndex);
  });
});
