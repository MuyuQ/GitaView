export type WidgetStableView = "collapsed" | "expanded" | "settings";
export type WidgetRenderView = WidgetStableView | "opening" | "closing";
export type WidgetTransitionMode = "open" | "close";

export const transitionDurations = {
  openingMs: 40,
  closingMs: 40,
} as const;

export function isWidgetTransitionView(view: WidgetRenderView): view is "opening" | "closing" {
  return view === "opening" || view === "closing";
}

export function getTransitionMode(view: WidgetRenderView): WidgetTransitionMode | null {
  if (view === "opening") return "open";
  if (view === "closing") return "close";
  return null;
}

export function getTransitionWindowTarget(view: WidgetRenderView): WidgetStableView {
  if (view === "opening" || view === "closing") return "expanded";
  return view;
}
