import SwiftUI

struct LargeWidgetView: View {
    let data: WidgetData

    var body: some View {
        VStack(alignment: .leading, spacing: 12) {
            HStack {
                Text("GitaView")
                    .font(.headline)
                    .foregroundColor(.primary)
                Spacer()
                Text(formattedDate(data.lastUpdated))
                    .font(.caption)
                    .foregroundColor(.secondary)
            }

            Divider()

            VStack(alignment: .leading, spacing: 8) {
                ForEach(data.repos) { repo in
                    RepoRow(repo: repo)
                }
            }

            Spacer()

            Divider()

            HStack {
                Text("总计: \(data.summary.total) 个仓库")
                    .font(.caption)
                    .foregroundColor(.secondary)
                Spacer()
                HStack(spacing: 12) {
                    SummaryBadge(label: "synced", count: data.summary.synced, color: .green)
                    SummaryBadge(label: "ahead", count: data.summary.localAhead + data.summary.remoteAhead, color: .yellow)
                    SummaryBadge(label: "diverged", count: data.summary.diverged, color: .red)
                }
            }
        }
        .padding()
    }

    private func formattedDate(_ date: Date) -> String {
        let formatter = RelativeDateTimeFormatter()
        formatter.unitsStyle = .abbreviated
        return formatter.localizedString(for: date, relativeTo: Date())
    }
}

struct RepoRow: View {
    let repo: WidgetData.RepoInfo

    var body: some View {
        HStack {
            Circle()
                .fill(colorForRelation(repo.relation))
                .frame(width: 8, height: 8)
            VStack(alignment: .leading) {
                Text(repo.name)
                    .font(.subheadline)
                    .foregroundColor(.primary)
                Text(repo.branch)
                    .font(.caption2)
                    .foregroundColor(.secondary)
            }
            Spacer()
            Text(repo.hint)
                .font(.caption)
                .foregroundColor(.secondary)
        }
    }

    private func colorForRelation(_ relation: String) -> Color {
        switch relation {
        case "synced": return .green
        case "local_ahead", "remote_ahead": return .yellow
        case "diverged": return .red
        default: return .gray
        }
    }
}

struct SummaryBadge: View {
    let label: String
    let count: Int
    let color: Color

    var body: some View {
        HStack(spacing: 4) {
            Circle()
                .fill(color)
                .frame(width: 6, height: 6)
            Text("\(count)")
                .font(.caption)
                .fontWeight(.medium)
            Text(label)
                .font(.caption2)
                .foregroundColor(.secondary)
        }
    }
}
