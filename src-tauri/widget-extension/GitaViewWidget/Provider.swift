import WidgetKit
import Foundation
import os.log

private let logger = Logger(subsystem: "com.gitaview.desktop.widget", category: "Provider")

struct Provider: TimelineProvider {
    func placeholder(in context: Context) -> WidgetEntry {
        WidgetEntry.placeholder
    }

    func getSnapshot(in context: Context, completion: @escaping (WidgetEntry) -> Void) {
        completion(loadEntry() ?? WidgetEntry.placeholder)
    }

    func getTimeline(in context: Context, completion: @escaping (Timeline<WidgetEntry>) -> Void) {
        let entry = loadEntry() ?? WidgetEntry.empty
        let timeline = Timeline(entries: [entry], policy: .never)
        completion(timeline)
    }

    private func loadEntry() -> WidgetEntry? {
        let path = NSString(string: "~/Library/Application Support/GitaView/widget-data.json")
            .expandingTildeInPath
        
        guard let data = FileManager.default.contents(atPath: path) else {
            logger.info("widget data file not found: \(path)")
            return nil
        }
        
        do {
            let widgetData = try JSONDecoder().decode(WidgetData.self, from: data)
            logger.info("loaded widget data: \(widgetData.repos.count) repos")
            return WidgetEntry(date: Date(), data: widgetData)
        } catch {
            logger.error("failed to decode widget data: \(error.localizedDescription)")
            return nil
        }
    }
}
