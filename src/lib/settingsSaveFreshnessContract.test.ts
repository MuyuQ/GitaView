import { describe, expect, it } from "vitest";
import { readProjectFile } from "./sourceContract";

describe("settings save freshness contract", () => {
  const savingComponents = [
    "src/components/settings/RepositorySettings.tsx",
    "src/components/settings/GroupSettings.tsx",
    "src/components/settings/RefreshSettings.tsx",
    "src/components/settings/AppearanceSettings.tsx",
  ];

  it("serializes persistence against freshly loaded settings in the shared queue", () => {
    const queue = readProjectFile("src/lib/settingsMutations.ts");

    expect(queue).toContain("persistSettings(patch(await loadSettings()))");
    expect(queue).toMatch(/pending\.then/);
  });

  it.each(savingComponents)("routes $path through the serialized update queue", (path) => {
    const source = readProjectFile(path);

    expect(source).toContain("queueSettingsUpdate(");
    expect(source).not.toMatch(/[^.]saveSettings\(/);
  });

  it.each(savingComponents)("builds the patch from the latest-settings argument in $path", (path) => {
    const source = readProjectFile(path);

    expect(source).toContain("(currentSettings)");
  });
});
