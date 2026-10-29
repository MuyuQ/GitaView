import { describe, expect, it, vi } from "vitest";
import { createSnapshotReceiver, type StatusSnapshot } from "./statusSnapshots";

const snapshot = (generation: number, error: string | null = null): StatusSnapshot => ({
  generation, completedAt: generation * 1000, repos: [], error,
});

describe("status snapshot delivery", () => {
  it("ignores a delayed IPC response after a newer tray refresh event", () => {
    const apply = vi.fn();
    const receive = createSnapshotReceiver(apply);
    receive(snapshot(2));
    receive(snapshot(1));
    receive(snapshot(2));
    expect(apply).toHaveBeenCalledOnce();
    expect(apply).toHaveBeenCalledWith(snapshot(2));
  });

  it("delivers errors and permits recovery from a later refresh", () => {
    const apply = vi.fn();
    const receive = createSnapshotReceiver(apply);
    receive(snapshot(1, "unreadable settings"));
    receive(snapshot(2));
    receive(snapshot(1, "unreadable settings"));
    expect(apply).toHaveBeenCalledTimes(2);
    expect(apply).toHaveBeenLastCalledWith(snapshot(2));
  });
});
