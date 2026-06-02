# Collapsed Summary Clarity Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Replace ambiguous single-character collapsed-widget status labels with a stable action-oriented summary that stays aligned across the native window, stable capsule, and transition capsule.

**Architecture:** Keep the complete four-bucket repository summary model unchanged, then add a focused presentation helper that filters the synced bucket and owns the three approved visible labels. Both collapsed React surfaces consume that helper so stable and animated states cannot drift. Treat the Tauri initial width, runtime width, and CSS widths as one `312px` cross-platform contract.

**Tech Stack:** React 19, TypeScript, plain CSS, Vitest, Tauri 2 JSON configuration

---

### Task 1: Add the action-oriented presentation model

**Files:**
- Create: `src/lib/collapsedSummary.ts`
- Create: `src/lib/collapsedSummary.test.ts`

- [ ] **Step 1: Write the failing test**

Create `src/lib/collapsedSummary.test.ts`:

```ts
import { describe, expect, it } from "vitest";
import { summarizeActionableCollapsed } from "./collapsedSummary";
import type { RepoStatus } from "../types";

const repo = (name: string, relation: RepoStatus["relation"]): RepoStatus => ({
  id: name,
  name,
  path: `E:/repos/${name}`,
  group: "全部分组",
  branch: "main",
  relation,
  changeLabel: "-",
  hint: "",
  hasRemote: relation !== "no_remote",
  remoteUrl: relation === "no_remote" ? null : "https://example.com/repo",
});

describe("summarizeActionableCollapsed", () => {
  it("omits synced repositories and keeps action labels in stable order", () => {
    expect(summarizeActionableCollapsed([
      repo("clean", "synced"),
      repo("up", "local_ahead"),
      repo("down", "remote_ahead"),
      repo("split", "diverged"),
      repo("broken", "error"),
      repo("local", "no_remote"),
    ])).toEqual([
      { bucket: "syncable", label: "待同步", count: 2 },
      { bucket: "needs_attention", label: "需关注", count: 2 },
      { bucket: "no_remote", label: "无远端", count: 1 },
    ]);
  });

  it("keeps zero-count action buckets visible", () => {
    expect(summarizeActionableCollapsed([repo("clean", "synced")])).toEqual([
      { bucket: "syncable", label: "待同步", count: 0 },
      { bucket: "needs_attention", label: "需关注", count: 0 },
      { bucket: "no_remote", label: "无远端", count: 0 },
    ]);
  });
});
```

- [ ] **Step 2: Run the focused test and verify RED**

Run: `npm test -- src/lib/collapsedSummary.test.ts`

Expected: FAIL because `./collapsedSummary` does not exist.

- [ ] **Step 3: Add the minimal shared helper**

Create `src/lib/collapsedSummary.ts`:

```ts
import { summarizeCollapsed } from "./statusModel";
import type { RepoStatus } from "../types";

const actionableBuckets = ["syncable", "needs_attention", "no_remote"] as const;
type ActionableCollapsedBucket = typeof actionableBuckets[number];

const actionableLabel: Record<ActionableCollapsedBucket, string> = {
  syncable: "待同步",
  needs_attention: "需关注",
  no_remote: "无远端",
};

export function summarizeActionableCollapsed(repos: RepoStatus[]) {
  const summary = summarizeCollapsed(repos);
  return actionableBuckets.map((bucket) => ({
    bucket,
    label: actionableLabel[bucket],
    count: summary.find((item) => item.bucket === bucket)?.count ?? 0,
  }));
}
```

- [ ] **Step 4: Run the focused test and verify GREEN**

Run: `npm test -- src/lib/collapsedSummary.test.ts`

Expected: PASS with `2` tests.

### Task 2: Render the shared summary in both collapsed surfaces

**Files:**
- Modify: `src/components/WidgetCollapsed.tsx`
- Modify: `src/components/TransitionSurface.tsx`
- Modify: `src/lib/reviewRemediationContract.test.ts`

- [ ] **Step 1: Strengthen the source contract before production edits**

Replace the collapsed-summary assertions in `src/lib/reviewRemediationContract.test.ts` with:

```ts
    const transition = readProjectFile("src/components/TransitionSurface.tsx");

    expect(collapsed).toContain("summarizeActionableCollapsed");
    expect(collapsed).toContain('className="repo-word">仓库</span>');
    expect(collapsed).toContain("{item.label}");
    expect(collapsed).not.toContain("summaryShortLabel");
    expect(transition).toContain("summarizeActionableCollapsed");
    expect(transition).toContain('className="repo-word">仓库</span>');
    expect(transition).toContain("{item.label}");
```

Keep the existing hover, color, and Vite icon assertions.

- [ ] **Step 2: Run the contract test and verify RED**

Run: `npm test -- src/lib/reviewRemediationContract.test.ts`

Expected: FAIL because the stable and transition surfaces do not yet consume the actionable summary helper.

- [ ] **Step 3: Update the stable collapsed component**

In `src/components/WidgetCollapsed.tsx`:

- Replace the `summarizeCollapsed` import with `summarizeActionableCollapsed`.
- Replace the summary call with `summarizeActionableCollapsed(repos)`.
- Remove the `summaryLabel` and `summaryShortLabel` maps.
- Insert `<span className="repo-word">仓库</span>` before the total.
- Render `<span>{item.label}</span>` before `<span>{item.count}</span>`.

- [ ] **Step 4: Update the transition capsule**

In `src/components/TransitionSurface.tsx`:

- Replace the `summarizeCollapsed` import with `summarizeActionableCollapsed`.
- Replace the summary call with `summarizeActionableCollapsed(repos)`.
- Insert `<span className="repo-word">仓库</span>` before the total.
- Render `<span>{item.label}</span>` before `<span>{item.count}</span>`.

- [ ] **Step 5: Run focused tests and verify GREEN**

Run: `npm test -- src/lib/collapsedSummary.test.ts src/lib/reviewRemediationContract.test.ts`

Expected: PASS with the shared summary helper tests and source contract tests.

### Task 3: Align the cross-platform collapsed width

**Files:**
- Modify: `src/App.tsx`
- Modify: `src/styles/widget.css`
- Modify: `src-tauri/tauri.conf.json`
- Create: `src/lib/collapsedWidthContract.test.ts`

- [ ] **Step 1: Write the failing width contract**

Create `src/lib/collapsedWidthContract.test.ts`:

```ts
import { describe, expect, it } from "vitest";
import { readProjectFile, readProjectJson } from "./sourceContract";

describe("collapsed width contract", () => {
  it("aligns native and CSS collapsed surfaces at 312px", () => {
    const app = readProjectFile("src/App.tsx");
    const styles = readProjectFile("src/styles/widget.css");
    const tauri = readProjectJson<{ app: { windows: Array<{ width: number }> } }>("src-tauri/tauri.conf.json");

    expect(tauri.app.windows[0].width).toBe(312);
    expect(app).toContain("collapsed: { width: 312, height: 40 }");
    expect(styles).toMatch(/\.collapsed-widget\s*{[^}]*width:\s*312px;/);
    expect(styles).toMatch(/\.transition-capsule\s*{[^}]*width:\s*312px;/);
  });
});
```

- [ ] **Step 2: Run the width contract and verify RED**

Run: `npm test -- src/lib/collapsedWidthContract.test.ts`

Expected: FAIL because all four width sources still use `218`.

- [ ] **Step 3: Update all width sources**

- Change `src/App.tsx` runtime collapsed width from `218` to `312`.
- Change `src/styles/widget.css` stable `.collapsed-widget` width from `218px` to `312px`.
- Change `src/styles/widget.css` `.transition-capsule` width from `218px` to `312px`.
- Change `src-tauri/tauri.conf.json` initial main-window width from `218` to `312`.
- Add `.repo-word` styling in `src/styles/widget.css` using the muted color, `11px`, and the existing Chinese fallback font family.

- [ ] **Step 4: Run the width contract and verify GREEN**

Run: `npm test -- src/lib/collapsedWidthContract.test.ts`

Expected: PASS with `1` test.

### Task 4: Verify, inspect, and publish

**Files:**
- Modify: `docs/superpowers/plans/2026-06-02-collapsed-summary-clarity.md`

- [ ] **Step 1: Run the complete frontend suite**

Run: `npm test`

Expected: PASS with all Vitest files and tests.

- [ ] **Step 2: Build the production frontend**

Run: `npm run build`

Expected: PASS with TypeScript compilation and Vite production bundle output.

- [ ] **Step 3: Check patch hygiene**

Run: `git diff --check`

Expected: exit code `0` with no whitespace errors.

- [ ] **Step 4: Review the changed files**

Run: `git status --short && git diff --stat && git diff -- src src-tauri`

Expected: only the approved collapsed-summary implementation, test files, plan file, and matching width configuration are changed.

- [ ] **Step 5: Commit and push**

```bash
git add docs/superpowers/plans/2026-06-02-collapsed-summary-clarity.md src src-tauri/tauri.conf.json
git commit -m "feat: clarify collapsed repository summary"
git push -u origin codex/collapsed-summary-clarity
```
