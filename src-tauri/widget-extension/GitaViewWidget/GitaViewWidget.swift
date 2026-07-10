import SwiftUI
import WidgetKit

@main
struct GitaViewWidget: Widget {
    let kind: String = "com.gitaview.widget"

    var body: some WidgetConfiguration {
        StaticConfiguration(kind: kind, provider: Provider()) { entry in
            if let data = entry.data {
                WidgetContentView(entry: entry, data: data)
            } else {
                EmptyStateView()
            }
        }
        .configurationDisplayName("GitaView")
        .description("Git 仓库状态概览")
        .supportedFamilies([.systemSmall, .systemMedium, .systemLarge])
    }
}

struct WidgetContentView: View {
    let entry: WidgetEntry
    let data: WidgetData

    @Environment(\.widgetFamily) var family

    var body: some View {
        switch family {
        case .systemSmall:
            SmallWidgetView(data: data)
        case .systemMedium:
            MediumWidgetView(data: data)
        case .systemLarge:
            LargeWidgetView(data: data)
        default:
            SmallWidgetView(data: data)
        }
    }
}

struct EmptyStateView: View {
    var body: some View {
        VStack(spacing: 8) {
            Image(systemName: "questionmark.folder")
                .font(.title2)
                .foregroundColor(.secondary)
            Text("打开 GitaView")
                .font(.caption)
                .foregroundColor(.secondary)
        }
    }
}
