export type ActionResultKind = "success" | "error";

export interface ActionResult {
  kind: ActionResultKind;
  text: string;
}

/// 统一操作结果的成功/失败语义：失败必须有独立的 kind，
/// 供 UI 用红色 + role="alert" 呈现，而不是与成功共用样式。
export function formatActionResult(
  action: string,
  outcome: { ok: true; message?: string } | { ok: false; error: unknown },
): ActionResult {
  if (outcome.ok) {
    return { kind: "success", text: outcome.message ?? `${action} 已完成` };
  }
  return { kind: "error", text: `${action} 失败：${String(outcome.error)}` };
}

export function actionResultClassName(kind: ActionResultKind): string {
  return kind === "error" ? "action-result action-result-error" : "action-result";
}

export function actionResultRole(kind: ActionResultKind): "alert" | "status" {
  return kind === "error" ? "alert" : "status";
}
