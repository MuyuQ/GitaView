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
        // 兜底刷新：应用崩溃/退出前未写数据时，widget 也不会永久停留旧数据；
        // 正常情况下应用侧刷新会驱动 reloadAllTimelines 提前更新。
        let nextRefresh = Calendar.current.date(byAdding: .minute, value: 15, to: Date()) ?? Date()
        let timeline = Timeline(entries: [entry], policy: .after(nextRefresh))
        completion(timeline)
    }

    private func loadEntry() -> WidgetEntry? {
        let path = NSString(string: "~/Library/Application Support/GitaView/widget-data.json")
            .expandingTildeInPath

        guard let data = FileManager.default.contents(atPath: path) else {
            // 日志只记文件名，不落全路径（避免用户名泄漏）
            logger.info("widget data file not found: \(NSString(string: path).lastPathComponent, privacy: .public)")
            return nil
        }
        
        do {
            // 与 Rust widget_data.rs 的契约：容器键为 camelCase（与模型声明一致），
            // lastUpdated 为 ISO-8601 字符串。fixture 见 Fixtures/widget-data.json，
            // 由 GitaViewWidgetTests 与 Rust 侧 golden 测试双向锁定。
            let decoder = JSONDecoder()
            decoder.dateDecodingStrategy = .iso8601
            let widgetData = try decoder.decode(WidgetData.self, from: data)
            logger.info("loaded widget data: \(widgetData.repos.count) repos")
            return WidgetEntry(date: Date(), data: widgetData)
        } catch {
            logger.error("failed to decode widget data: \(error.localizedDescription)")
            return nil
        }
    }
}
