import Foundation
import WidgetKit

struct WidgetData: Codable {
    let version: Int
    let lastUpdated: Date
    let repos: [RepoInfo]
    let summary: Summary

    struct RepoInfo: Codable, Identifiable {
        let id: String
        let name: String
        let group: String
        let branch: String
        let relation: String
        let changeLabel: String
        let hint: String
    }

    struct Summary: Codable {
        let synced: Int
        let localAhead: Int
        let remoteAhead: Int
        let diverged: Int
        let noRemote: Int
        let total: Int
    }
}

struct WidgetEntry: TimelineEntry {
    let date: Date
    let data: WidgetData?

    static var placeholder: WidgetEntry {
        WidgetEntry(
            date: Date(),
            data: WidgetData(
                version: 1,
                lastUpdated: Date(),
                repos: [
                    WidgetData.RepoInfo(id: "1", name: "frontend", group: "默认", branch: "main", relation: "synced", changeLabel: "clean", hint: "Up to date"),
                    WidgetData.RepoInfo(id: "2", name: "api-server", group: "默认", branch: "develop", relation: "local_ahead", changeLabel: "3 ahead", hint: "Push 3 commits"),
                ],
                summary: WidgetData.Summary(synced: 5, localAhead: 2, remoteAhead: 1, diverged: 0, noRemote: 3, total: 11)
            )
        )
    }

    static var empty: WidgetEntry {
        WidgetEntry(date: Date(), data: nil)
    }
}
