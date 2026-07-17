import { describe, expect, it } from "vitest";
import { readProjectFile } from "./sourceContract";

describe("settings save freshness contract", () => {
  const cases = [
    {
      path: "src/components/settings/RefreshSettings.tsx",
      field: "refresh:",
    },
    {
      path: "src/components/settings/SafetySettings.tsx",
      field: "safety:",
    },
    {
      path: "src/components/settings/AppearanceSettings.tsx",
      field: "appearance:",
    },
  ];

  it.each(cases)("reloads latest settings before saving $path", ({ path, field }) => {
    const source = readProjectFile(path);
    const saveBody = source.match(/async function handleSave\(\) \{[\s\S]*?\n  \}/)?.[0] ?? "";

    expect(saveBody).toContain("const latestSettings = await getSettings();");
    expect(saveBody).toContain("...latestSettings,");
    expect(saveBody).toContain(field);
  });
});
