import { beforeEach, describe, expect, it } from "vitest";
import {
  BACKGROUND_IMAGE_STORAGE_KEY,
  useUIStore,
  type BackgroundImageFit,
} from "../../src/stores/ui.store";

describe("UIStore background image settings", () => {
  beforeEach(() => {
    localStorage.clear();
    useUIStore.setState({
      backgroundImage: null,
    });
  });

  it("persists imported background image settings", () => {
    useUIStore.getState().setBackgroundImage({
      path: "C:\\Users\\me\\AppData\\Roaming\\Pebble\\backgrounds\\background.png",
      filename: "background.png",
    });

    const state = useUIStore.getState();
    expect(state.backgroundImage).toMatchObject({
      path: "C:\\Users\\me\\AppData\\Roaming\\Pebble\\backgrounds\\background.png",
      filename: "background.png",
      fit: "cover",
      opacity: 0.35,
    });
    expect(JSON.parse(localStorage.getItem(BACKGROUND_IMAGE_STORAGE_KEY) ?? "{}")).toMatchObject({
      filename: "background.png",
      fit: "cover",
      opacity: 0.35,
    });
  });

  it("updates fit and opacity without losing the stored image path", () => {
    useUIStore.getState().setBackgroundImage({
      path: "/tmp/pebble/backgrounds/wallpaper.webp",
      filename: "wallpaper.webp",
    });

    useUIStore.getState().setBackgroundImageFit("repeat" as BackgroundImageFit);
    useUIStore.getState().setBackgroundImageOpacity(0.4);

    expect(useUIStore.getState().backgroundImage).toMatchObject({
      path: "/tmp/pebble/backgrounds/wallpaper.webp",
      filename: "wallpaper.webp",
      fit: "repeat",
      opacity: 0.4,
    });
  });

  it("allows full-strength background opacity and clamps invalid extremes", () => {
    useUIStore.getState().setBackgroundImage({
      path: "/tmp/pebble/backgrounds/wallpaper.webp",
      filename: "wallpaper.webp",
    });

    useUIStore.getState().setBackgroundImageOpacity(1);
    expect(useUIStore.getState().backgroundImage?.opacity).toBe(1);

    useUIStore.getState().setBackgroundImageOpacity(4);
    expect(useUIStore.getState().backgroundImage?.opacity).toBe(1);

    useUIStore.getState().setBackgroundImageOpacity(0);
    expect(useUIStore.getState().backgroundImage?.opacity).toBe(0.05);
  });

  it("clears the background image preference", () => {
    useUIStore.getState().setBackgroundImage({
      path: "/tmp/pebble/backgrounds/wallpaper.jpg",
      filename: "wallpaper.jpg",
    });

    useUIStore.getState().clearBackgroundImage();

    expect(useUIStore.getState().backgroundImage).toBeNull();
    expect(localStorage.getItem(BACKGROUND_IMAGE_STORAGE_KEY)).toBeNull();
  });
});
