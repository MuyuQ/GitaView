import { describe, expect, it } from "vitest";
import { readProjectFile, readProjectFileCompact } from "./sourceContract";

describe("git backend boundary contract", () => {
  it("keeps remote URL normalization outside git commands", () => {
    const lib = readProjectFileCompact("src-tauri/src/lib.rs");
    const commands = readProjectFile("src-tauri/src/git/commands.rs");
    const remote = readProjectFile("src-tauri/src/git/remote.rs");
    const appCommands = readProjectFileCompact("src-tauri/src/app_commands.rs");

    expect(lib).toContain("pub mod remote;");
    expect(commands).not.toContain("pub fn normalize_remote_url");
    expect(commands).toContain("use crate::git::remote::normalize_remote_url;");
    expect(remote).toContain("pub fn normalize_remote_url");
    expect(appCommands).toContain("use crate::git::remote::normalize_remote_url;");
  });

  it("keeps status text formatting outside git commands", () => {
    const lib = readProjectFileCompact("src-tauri/src/lib.rs");
    const commands = readProjectFile("src-tauri/src/git/commands.rs");
    const statusText = readProjectFile("src-tauri/src/git/status_text.rs");
    const repoStatus = readProjectFileCompact("src-tauri/src/repo_status.rs");

    expect(lib).toContain("pub mod status_text;");
    expect(commands).not.toContain("pub fn change_label");
    expect(commands).not.toContain("pub fn relation_hint");
    expect(commands).not.toContain("pub fn state_hint");
    expect(statusText).toContain("pub fn change_label");
    expect(statusText).toContain("pub fn state_hint");
    expect(repoStatus).toContain(
      "use crate::git::status_text::{change_label, state_hint};",
    );
  });
});
