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
