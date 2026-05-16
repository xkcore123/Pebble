import { fireEvent, render, screen, waitFor } from "@testing-library/react";
import { beforeEach, describe, expect, it, vi } from "vitest";
import AppearanceTab from "../../../src/features/settings/AppearanceTab";
import { useUIStore } from "../../../src/stores/ui.store";
import { deleteBackgroundImage, importBackgroundImage } from "../../../src/lib/backgroundImage";

vi.mock("react-i18next", () => ({
  initReactI18next: {
    type: "3rdParty",
    init: vi.fn(),
  },
  useTranslation: () => ({
    t: (_key: string, fallback?: string) => fallback ?? _key,
  }),
}));

vi.mock("../../../src/lib/backgroundImage", () => ({
  backgroundImageUrl: vi.fn((path: string) => `asset://${path}`),
  deleteBackgroundImage: vi.fn().mockResolvedValue(undefined),
  importBackgroundImage: vi.fn(),
}));

const importBackgroundImageMock = vi.mocked(importBackgroundImage);
const deleteBackgroundImageMock = vi.mocked(deleteBackgroundImage);

describe("AppearanceTab background image", () => {
  beforeEach(() => {
    localStorage.clear();
    importBackgroundImageMock.mockReset();
    deleteBackgroundImageMock.mockReset();
    deleteBackgroundImageMock.mockResolvedValue(undefined);
    useUIStore.setState({
      theme: "light",
      language: "en",
      backgroundImage: null,
    });
  });

  it("imports a selected image through the backend and stores the returned path", async () => {
    importBackgroundImageMock.mockResolvedValue({
      path: "C:\\Users\\me\\AppData\\Roaming\\Pebble\\backgrounds\\background.png",
      filename: "background.png",
      size: 12,
    });
    const file = new File(["png"], "wallpaper.png", { type: "image/png" });

    render(<AppearanceTab />);

    fireEvent.change(screen.getByLabelText("Choose background image"), {
      target: { files: [file] },
    });

    await waitFor(() => expect(importBackgroundImageMock).toHaveBeenCalledWith(file));
    expect(useUIStore.getState().backgroundImage).toMatchObject({
      path: "C:\\Users\\me\\AppData\\Roaming\\Pebble\\backgrounds\\background.png",
      filename: "background.png",
    });
    expect(screen.getByText("background.png")).toBeTruthy();
  });

  it("removes the imported image and clears the local preference", async () => {
    useUIStore.getState().setBackgroundImage({
      path: "/tmp/pebble/backgrounds/background.jpg",
      filename: "background.jpg",
    });

    render(<AppearanceTab />);

    fireEvent.click(screen.getByRole("button", { name: "Remove background image" }));

    await waitFor(() => {
      expect(deleteBackgroundImageMock).toHaveBeenCalledWith("/tmp/pebble/backgrounds/background.jpg");
    });
    expect(useUIStore.getState().backgroundImage).toBeNull();
  });

  it("lets the image opacity slider reach full strength", () => {
    useUIStore.getState().setBackgroundImage({
      path: "/tmp/pebble/backgrounds/background.jpg",
      filename: "background.jpg",
    });

    render(<AppearanceTab />);

    expect(screen.getByLabelText("Image opacity").getAttribute("max")).toBe("1");
  });
});
