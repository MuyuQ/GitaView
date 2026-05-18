import { describe, expect, it } from "vitest";
import { readProjectFile } from "./sourceContract";

describe("repo table column resize listener lifecycle", () => {
  it("keeps document mouse listeners inside a React effect with cleanup", () => {
    const source = readProjectFile("src/components/RepoTable.tsx");

    expect(source).toContain("useEffect");
    expect(source).not.toContain("if (draggingColumn !== prevDraggingColumn.current)");
    expect(source).toMatch(/useEffect\(\(\) => \{[\s\S]*document\.addEventListener\("mousemove", handleMouseMove\);[\s\S]*return \(\) => \{[\s\S]*document\.removeEventListener\("mousemove", handleMouseMove\);[\s\S]*\};[\s\S]*\}, \[draggingColumn, handleMouseMove, handleMouseUp\]\);/);
  });
});
