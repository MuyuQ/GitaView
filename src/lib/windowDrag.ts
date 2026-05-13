export function shouldStartWindowDrag(target: EventTarget | null): boolean {
  const closest = (target as { closest?: (selector: string) => unknown } | null)?.closest;
  if (typeof closest !== "function") return true;
  return !closest.call(target, "button, input, select, textarea, a, [role='button']");
}

export function shouldStartCollapsedDrag(allowDrag: boolean, button: number): boolean {
  return allowDrag && button === 0;
}

export function shouldStartExpandedDrag(allowDrag: boolean, button: number, target: EventTarget | null): boolean {
  return allowDrag && button === 0 && shouldStartWindowDrag(target);
}

export function shouldPromoteExpandedDrag(
  allowDrag: boolean,
  start: { x: number; y: number } | null,
  current: { x: number; y: number },
  threshold = 4,
): boolean {
  if (!allowDrag || !start) return false;
  const deltaX = Math.abs(current.x - start.x);
  const deltaY = Math.abs(current.y - start.y);
  return deltaX + deltaY >= threshold;
}

export function shouldPromoteCollapsedDrag(
  allowDrag: boolean,
  start: { x: number; y: number } | null,
  current: { x: number; y: number },
  threshold = 4,
): boolean {
  if (!allowDrag || !start) return false;
  const deltaX = Math.abs(current.x - start.x);
  const deltaY = Math.abs(current.y - start.y);
  return deltaX + deltaY >= threshold;
}
