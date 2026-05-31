export function resolveRefreshCompletion(requestGeneration: number, latestGeneration: number): boolean {
  return requestGeneration === latestGeneration;
}
