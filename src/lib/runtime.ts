type RuntimeProbe = {
  window?: {
    __TAURI_INTERNALS__?: unknown;
  };
};

export function hasTauriRuntime(globalObject: RuntimeProbe = globalThis as RuntimeProbe): boolean {
  return Boolean(globalObject.window?.__TAURI_INTERNALS__);
}
