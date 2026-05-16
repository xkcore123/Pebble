import { render, screen } from "@testing-library/react";
import { describe, expect, it, vi } from "vitest";
import AppBackground from "../../src/app/AppBackground";

vi.mock("../../src/lib/backgroundImage", () => ({
  backgroundImageUrl: vi.fn((path: string) => `asset://${path}`),
}));

describe("AppBackground", () => {
  it("renders nothing when no background image is configured", () => {
    const { container } = render(<AppBackground image={null} />);

    expect(container.firstChild).toBeNull();
  });

  it("renders a configured image as a non-interactive background layer", () => {
    render(
      <AppBackground
        image={{
          path: "/tmp/pebble/backgrounds/background.webp",
          filename: "background.webp",
          fit: "contain",
          opacity: 0.35,
          updatedAt: 123,
        }}
      />,
    );

    const layer = screen.getByTestId("app-background");
    expect(layer.getAttribute("aria-hidden")).toBe("true");
    expect(layer.style.backgroundImage).toBe('url("asset:///tmp/pebble/backgrounds/background.webp")');
    expect(layer.style.backgroundSize).toBe("contain");
    expect(layer.style.backgroundRepeat).toBe("no-repeat");
    expect(layer.style.opacity).toBe("0.35");
  });
});
