import { expect, it } from "vitest";
import { createRefreshQueue } from "./refreshQueue";

it("does not lose a request arriving in the completion microtask", async () => {
  let calls = 0;
  const completion = Promise.resolve();
  const request = createRefreshQueue(async () => { calls++; await completion; });
  const first = request();
  const second = completion.then(() => request());
  await Promise.all([first, second]);
  expect(calls).toBe(2);
});

it("coalesces requests during a refresh and runs again after completion", async () => {
  const releases: Array<() => void> = [];
  let calls = 0;
  const request = createRefreshQueue(async () => {
    calls++;
    await new Promise<void>((resolve) => releases.push(resolve));
  });
  const done = request();
  request();
  request();
  expect(calls).toBe(1);
  releases.shift()!();
  await Promise.resolve();
  await Promise.resolve();
  expect(calls).toBe(2);
  releases.shift()!();
  await done;
  expect(calls).toBe(2);
});

it("runs a pending refresh even if the previous refresh fails", async () => {
  let reject!: (error: Error) => void;
  let calls = 0;
  const request = createRefreshQueue(async () => {
    if (++calls === 1) await new Promise<void>((_, fail) => { reject = fail; });
  });
  const done = request();
  request();
  reject(new Error("read failed"));
  await done;
  expect(calls).toBe(2);
  await request();
  expect(calls).toBe(3);
});
