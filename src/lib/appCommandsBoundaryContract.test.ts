import { describe, expect, it } from "vitest";
import { readProjectFile, readProjectFileCompact } from "./sourceContract";

describe("app command boundary contract", () => {
  it("keeps platform opening and repository policy out of app_commands", () => {
    const commands = readProjectFile("src-tauri/src/app_commands.rs");
    const compactCommands = readProjectFileCompact("src-tauri/src/app_commands.rs");

    expect(commands).not.toContain("target_os");
    expect(commands).not.toContain("Command::new");
    expect(commands).not.toContain("enum RepoGitOperation");
    expect(commands).not.toContain("fn validate_repo_git_operation");
    expect(commands).not.toContain("fn repo_id_from_path");
    expect(commands).not.toContain("fn find_repo");
    expect(compactCommands).toContain(
      "use crate::repo_operation::{validate_repo_git_operation, RepoGitOperation};",
    );
    expect(compactCommands).toContain(
      "use crate::repo_registry::{find_repo, repo_id_from_path};",
    );
    expect(compactCommands).toContain(
      "use crate::system_open::{open_directory, open_http_url};",
    );
  });
});
