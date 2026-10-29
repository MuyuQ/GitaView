export type ActionResultKind = "success" | "error";

export interface ActionResult {
  kind: ActionResultKind;
  text: string;
}

export async function executeRepoAction(
  action: string,
  execute: () => Promise<string | void>,
  onRefresh: () => void,
): Promise<ActionResult> {
  try {
    const message = await execute();
    return formatActionResult(action, { ok: true, message: typeof message === "string" ? message : undefined });
  } catch (error) {
    return formatActionResult(action, { ok: false, error });
  } finally {
    // Git can change refs or the working tree before reporting a failure.
    if (action === "Fetch" || action === "Pull" || action === "Push") onRefresh();
  }
}

/// 统一操作结果的成功/失败语义：失败必须有独立的 kind，
/// 供 UI 用红色 + role="alert" 呈现，而不是与成功共用样式。
export function formatActionResult(
  action: string,
  outcome: { ok: true; message?: string } | { ok: false; error: unknown },
): ActionResult {
  if (outcome.ok) {
    // 空字符串消息会渲染成空 span，回退到默认成功文案
    const message = outcome.message?.trim() ? outcome.message : `${action} 已完成`;
    return { kind: "success", text: message };
  }
  return { kind: "error", text: `${action} 失败：${String(outcome.error)}` };
}

export function actionResultClassName(kind: ActionResultKind): string {
  return kind === "error" ? "action-result action-result-error" : "action-result";
}

export function actionResultRole(kind: ActionResultKind): "alert" | "status" {
  return kind === "error" ? "alert" : "status";
}
