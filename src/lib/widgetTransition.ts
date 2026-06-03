export type WidgetStableView = "collapsed" | "expanded" | "settings";
export type WidgetRenderView = WidgetStableView;

export function getTransitionWindowTarget(view: WidgetRenderView): WidgetStableView {
  return view;
}
