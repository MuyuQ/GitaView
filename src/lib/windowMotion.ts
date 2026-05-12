export interface WindowSizeValue {
  width: number;
  height: number;
}

export interface WindowPositionValue {
  x: number;
  y: number;
}

export interface WindowRectValue extends WindowPositionValue, WindowSizeValue {}

const edgeAnchorTolerance = 2;

function clamp(value: number, min: number, max: number) {
  if (max < min) return min;
  return Math.min(Math.max(value, min), max);
}

export function interpolateWindowSize(from: WindowSizeValue, to: WindowSizeValue, progress: number): WindowSizeValue {
  const eased = 1 - Math.pow(1 - Math.min(Math.max(progress, 0), 1), 3);
  return {
    width: Math.round(from.width + (to.width - from.width) * eased),
    height: Math.round(from.height + (to.height - from.height) * eased),
  };
}

export function resolveAnchoredWindowPosition(
  current: WindowRectValue,
  targetSize: WindowSizeValue,
  workArea: WindowRectValue,
): WindowPositionValue {
  const workRight = workArea.x + workArea.width;
  const workBottom = workArea.y + workArea.height;
  const currentRight = current.x + current.width;
  const currentBottom = current.y + current.height;
  const targetMaxX = workRight - targetSize.width;
  const targetMaxY = workBottom - targetSize.height;

  const shouldAnchorRight = current.x + targetSize.width > workRight
    || Math.abs(currentRight - workRight) <= edgeAnchorTolerance;
  const shouldAnchorBottom = current.y + targetSize.height > workBottom
    || Math.abs(currentBottom - workBottom) <= edgeAnchorTolerance;

  const nextX = shouldAnchorRight ? currentRight - targetSize.width : current.x;
  const nextY = shouldAnchorBottom ? currentBottom - targetSize.height : current.y;

  return {
    x: clamp(Math.round(nextX), workArea.x, targetMaxX),
    y: clamp(Math.round(nextY), workArea.y, targetMaxY),
  };
}
