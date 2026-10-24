import Dispatch
import WidgetKit

@_cdecl("reload_widget_timelines")
public func reloadWidgetTimelines() {
    DispatchQueue.main.async {
        WidgetCenter.shared.reloadAllTimelines()
    }
}
