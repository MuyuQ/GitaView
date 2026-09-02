export type SettingsMessage = { kind: "ok" | "error"; text: string } | null;

/// 设置页提示消息：成功与失败必须可区分（失败红色 + role="alert"）
export function okMessage(text: string): SettingsMessage {
  return { kind: "ok", text };
}

export function errorMessage(prefix: string, err: unknown): SettingsMessage {
  return { kind: "error", text: `${prefix}：${String(err)}` };
}

export function errorMessageText(text: string): SettingsMessage {
  return { kind: "error", text };
}
