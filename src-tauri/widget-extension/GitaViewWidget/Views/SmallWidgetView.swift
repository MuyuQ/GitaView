import SwiftUI

struct SmallWidgetView: View {
    let data: WidgetData

    var body: some View {
        VStack(alignment: .leading, spacing: 8) {
            HStack {
                Text("GitaView")
                    .font(.headline)
                    .foregroundColor(.primary)
                Spacer()
                Text("\(data.summary.total) repos")
                    .font(.caption)
                    .foregroundColor(.secondary)
            }

            Spacer()

            VStack(alignment: .leading, spacing: 4) {
                StatusRow(label: "synced", count: data.summary.synced, color: .green)
                StatusRow(label: "local ahead", count: data.summary.localAhead, color: .yellow)
                StatusRow(label: "remote ahead", count: data.summary.remoteAhead, color: .yellow)
                StatusRow(label: "diverged", count: data.summary.diverged, color: .red)
                StatusRow(label: "no remote", count: data.summary.noRemote, color: .gray)
            }

            Spacer()

            Text(formattedDate(data.lastUpdated))
                .font(.caption2)
                .foregroundColor(.secondary)
        }
        .padding()
    }

    private func formattedDate(_ date: Date) -> String {
        let formatter = RelativeDateTimeFormatter()
        formatter.unitsStyle = .abbreviated
        return formatter.localizedString(for: date, relativeTo: Date())
    }
}

struct StatusRow: View {
    let label: String
    let count: Int
    let color: Color

    var body: some View {
        HStack(spacing: 6) {
            Circle()
                .fill(color)
                .frame(width: 8, height: 8)
            Text(label)
                .font(.caption)
                .foregroundColor(.primary)
            Spacer()
            Text("\(count)")
                .font(.caption)
                .fontWeight(.medium)
                .foregroundColor(.primary)
        }
    }
}
