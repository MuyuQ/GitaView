/** Keep one active refresh and one coalesced follow-up request. */
export function createRefreshQueue(refresh: () => Promise<void>) {
  let active: Promise<void> | null = null;
  let pending = false;
  return (): Promise<void> => {
    pending = true;
    if (active) return active;
    active = (async () => {
      do {
        pending = false;
        try {
          await refresh();
        } catch {
          // The caller reports the failure; it must not discard a pending refresh.
          await Promise.resolve();
        }
      } while (pending);
      active = null;
    })();
    return active;
  };
}
