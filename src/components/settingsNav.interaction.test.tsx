import { cleanup, render, screen } from "@testing-library/react";
import userEvent from "@testing-library/user-event";
import { afterEach, describe, expect, it, vi } from "vitest";
import { SettingsShell } from "./settings/SettingsShell";

vi.mock("../../lib/commands", () => ({
  getSettings: vi.fn().mockResolvedValue({
    version: 1,
    repos: [],
    groups: [{ name: "全部分组", repoIds: [] }],
    defaultGroup: "全部分组",
    refresh: { lightweightRefreshEnabled: true, intervalMinutes: 5 },
    safety: { confirmPull: true, confirmPush: true },
    appearance: { allowWidgetDrag: true },
  }),
  scanRepositories: vi.fn().mockResolvedValue([]),
  addRepository: vi.fn(),
  removeRepository: vi.fn(),
  addGroup: vi.fn(),
  removeGroup: vi.fn(),
  saveSettings: vi.fn(),
}));

afterEach(() => {
  cleanup();
});

describe("settings shell navigation", () => {
  it("lands on repository settings and switches to the general section", async () => {
    const user = userEvent.setup();
    render(<SettingsShell onClose={() => undefined} />);

    // 默认落在仓库设置：仓库管理卡片可见
    expect(screen.getByText("仓库管理")).toBeTruthy();
    expect(screen.queryByText("刷新设置")).toBeNull();

    await user.click(screen.getByRole("button", { name: "常规设置" }));

    expect(screen.getByText("刷新设置")).toBeTruthy();
    expect(screen.getByText("安全操作")).toBeTruthy();
    expect(screen.getByText("外观")).toBeTruthy();
    expect(screen.queryByText("仓库管理")).toBeNull();
  });

  it("marks the active navigation item with aria-current", async () => {
    const user = userEvent.setup();
    render(<SettingsShell onClose={() => undefined} />);

    const repositoryNav = screen.getByRole("button", { name: "仓库设置" });
    expect(repositoryNav.getAttribute("aria-current")).toBe("page");

    await user.click(screen.getByRole("button", { name: "常规设置" }));
    const generalNav = screen.getByRole("button", { name: "常规设置" });
    expect(generalNav.getAttribute("aria-current")).toBe("page");
    expect(repositoryNav.getAttribute("aria-current")).toBeNull();
  });

  it("invokes onClose from the footer close button", async () => {
    const user = userEvent.setup();
    const onClose = vi.fn();
    render(<SettingsShell onClose={onClose} />);

    await user.click(screen.getByRole("button", { name: "关闭" }));
    expect(onClose).toHaveBeenCalledTimes(1);
  });
});
