import { fireEvent, render, screen, waitFor } from "@testing-library/react";
import { beforeEach, describe, expect, it, vi } from "vitest";
import AboutTab from "../../../src/features/settings/AboutTab";
import { invoke } from "@tauri-apps/api/core";

vi.mock("react-i18next", () => ({
  useTranslation: () => ({
    t: (_key: string, fallback?: string) => fallback ?? _key,
  }),
}));

vi.mock("@tauri-apps/api/app", () => ({
  getVersion: vi.fn().mockResolvedValue("1.2.3"),
}));

vi.mock("@tauri-apps/api/core", () => ({
  invoke: vi.fn(),
}));

const mockInvoke = vi.mocked(invoke);

describe("AboutTab diagnostics", () => {
  beforeEach(() => {
    mockInvoke.mockReset();
    mockInvoke.mockImplementation((command) => {
      if (command === "read_app_log") {
        return Promise.resolve({
          path: "C:\\Users\\me\\AppData\\Roaming\\Pebble\\logs\\pebble.log",
          content: "first line\nlatest line",
          truncated: false,
        });
      }
      return Promise.resolve(null);
    });
  });

  it("opens the diagnostic log after five quick app icon clicks", async () => {
    render(<AboutTab />);

    const iconButton = screen.getByRole("button", { name: "Open diagnostic log" });

    for (let i = 0; i < 4; i += 1) {
      fireEvent.click(iconButton);
    }

    expect(mockInvoke).not.toHaveBeenCalledWith("read_app_log", expect.anything());

    fireEvent.click(iconButton);

    await waitFor(() => {
      expect(mockInvoke).toHaveBeenCalledWith("read_app_log", { maxBytes: 65536 });
    });
    expect(screen.getByRole("dialog", { name: "Diagnostic log" })).toBeTruthy();
    expect(screen.getByText(/latest line/)).toBeTruthy();
    expect(screen.getByText(/pebble\.log$/)).toBeTruthy();
  });

  it("keeps the diagnostic log open when clicking the backdrop", async () => {
    render(<AboutTab />);

    const iconButton = screen.getByRole("button", { name: "Open diagnostic log" });
    for (let i = 0; i < 5; i += 1) {
      fireEvent.click(iconButton);
    }

    const dialog = await screen.findByRole("dialog", { name: "Diagnostic log" });
    fireEvent.mouseDown(dialog);
    fireEvent.click(dialog);

    expect(screen.queryByRole("dialog", { name: "Diagnostic log" })).not.toBeNull();
  });
});
