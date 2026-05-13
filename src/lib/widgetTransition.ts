export type WidgetStableView = "collapsed" | "expanded" | "settings";
export type WidgetRenderView = WidgetStableView | "opening" | "closing";
export type WidgetTransitionMode = "open" | "close";

export const transitionDurations = {
  openingMs: 0,
  closingMs: 0,
} as const;

export function isWidgetTransitionView(view: WidgetRenderView): view is "opening" | "closing" {
  void view;
  return false;
}

export function getTransitionMode(view: WidgetRenderView): WidgetTransitionMode | null {
  void view;
  return null;
}

export function getTransitionWindowTarget(view: WidgetRenderView): WidgetStableView {
  if (view === "opening" || view === "closing") return "collapsed";
  return view;
}
