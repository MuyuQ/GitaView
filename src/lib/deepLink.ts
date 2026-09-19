import { listen } from "@tauri-apps/api/event";
import { hasTauriRuntime } from "./runtime";

/**
 * Deep link 事件契约。
 *
 * 事件名与 Rust `lib.rs` 的 `DEEP_LINK_OPEN_REPO_EVENT` 对齐：
 * `gitaview://open/repo/<id>` 触发后，后端广播该事件并携带仓库 id，
 * 前端展开视图并选中对应仓库。
 */
export const openRepoEventName = "gitaview://open-repo";

export type UnsubscribeFn = () => void;

/** 订阅"打开指定仓库"深链请求；非 Tauri 运行时（浏览器预览）为 no-op */
export async function subscribeToOpenRepoRequests(
  handler: (repoId: string) => void,
): Promise<UnsubscribeFn> {
  if (!hasTauriRuntime()) {
    return () => {};
  }
  const unlisten = await listen<string>(openRepoEventName, (event) => handler(event.payload));
  return unlisten;
}
