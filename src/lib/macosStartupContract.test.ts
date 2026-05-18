import { describe, expect, it } from "vitest";
import { readProjectFile, readProjectJson } from "./sourceContract";

describe("macOS transparent widget startup contract", () => {
  it("enables macOSPrivateApi when the main window is transparent", () => {
    const config = readProjectJson<{
      app: {
        macOSPrivateApi?: boolean;
        windows: Array<{ transparent?: boolean }>;
      };
    }>("src-tauri/tauri.conf.json");
    const cargoToml = readProjectFile("src-tauri/Cargo.toml");
    const hasTransparentWindow = config.app.windows.some((windowConfig: { transparent?: boolean }) => windowConfig.transparent);

    expect(hasTransparentWindow).toBe(true);
    expect(config.app.macOSPrivateApi).toBe(true);
    expect(cargoToml).toContain("macos-private-api");
  });
});
