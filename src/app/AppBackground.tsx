import { backgroundImageUrl } from "@/lib/backgroundImage";
import type { BackgroundImageSettings } from "@/stores/ui.store";

interface AppBackgroundProps {
  image: BackgroundImageSettings | null;
}

function cssUrl(value: string): string {
  return `url("${value.replace(/"/g, '\\"')}")`;
}

export default function AppBackground({ image }: AppBackgroundProps) {
  if (!image) return null;

  const repeat = image.fit === "repeat";
  const url = backgroundImageUrl(image.path);

  return (
    <div
      aria-hidden="true"
      className="app-background-layer"
      data-testid="app-background"
      style={{
        backgroundImage: cssUrl(url),
        backgroundPosition: "center",
        backgroundRepeat: repeat ? "repeat" : "no-repeat",
        backgroundSize: repeat ? "auto" : image.fit,
        opacity: image.opacity,
      }}
    />
  );
}
