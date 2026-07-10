import SwiftUI

struct MediumWidgetView: View {
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

            HStack(spacing: 16) {
                StatusCard(label: "synced", count: data.summary.synced, color: .green)
                StatusCard(label: "local ahead", count: data.summary.localAhead, color: .yellow)
                StatusCard(label: "remote ahead", count: data.summary.remoteAhead, color: .yellow)
                StatusCard(label: "diverged", count: data.summary.diverged, color: .red)
                StatusCard(label: "no remote", count: data.summary.noRemote, color: .gray)
            }

            Spacer()

            Text("\(data.summary.total) 个仓库")
                .font(.caption)
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

struct StatusCard: View {
    let label: String
    let count: Int
    let color: Color

    var body: some View {
        VStack(spacing: 4) {
            Text("\(count)")
                .font(.title2)
                .fontWeight(.bold)
                .foregroundColor(color)
            Text(label)
                .font(.caption2)
                .foregroundColor(.secondary)
                .multilineTextAlignment(.center)
        }
        .frame(maxWidth: .infinity)
    }
}
