import XCTest
@testable import GitaViewWidgetExtension

/// 与 Rust `widget_payload_matches_cross_language_fixture` 共享同一份 fixture
/// （Fixtures/widget-data.json，由 buildPhase: resources 打进测试 bundle）。
/// Rust 端改动 WidgetPayload 序列化后，这里会以解码失败的形式暴露漂移。
final class WidgetDataDecodingTests: XCTestCase {
    private func loadFixtureData() throws -> Data {
        let bundle = Bundle(for: WidgetDataDecodingTests.self)
        let url = try XCTUnwrap(
            bundle.url(forResource: "widget-data", withExtension: "json"),
            "widget-data fixture must be bundled with the test target"
        )
        return try Data(contentsOf: url)
    }

    private func makeDecoder() -> JSONDecoder {
        let decoder = JSONDecoder()
        decoder.dateDecodingStrategy = .iso8601
        return decoder
    }

    func testFixtureDecodesWithProductionDecoderSettings() throws {
        let data = try loadFixtureData()
        let widgetData = try makeDecoder().decode(WidgetData.self, from: data)

        XCTAssertEqual(widgetData.version, 1)
        XCTAssertEqual(widgetData.repos.count, 2)
        XCTAssertEqual(widgetData.summary.total, 2)
        XCTAssertEqual(widgetData.summary.localAhead, 1)
        XCTAssertEqual(widgetData.summary.noRemote, 0)
    }

    func testRelationValuesStaySnakeCaseForColorMapping() throws {
        let data = try loadFixtureData()
        let widgetData = try makeDecoder().decode(WidgetData.self, from: data)

        // LargeWidgetView.colorForRelation 依赖带下划线的 snake_case 取值
        XCTAssertEqual(widgetData.repos[0].relation, "synced")
        XCTAssertEqual(widgetData.repos[1].relation, "local_ahead")
    }

    func testLastUpdatedDecodesIso8601Timestamp() throws {
        let data = try loadFixtureData()
        let widgetData = try makeDecoder().decode(WidgetData.self, from: data)

        let expected = Date(timeIntervalSince1970: 1_788_652_800) // 2026-09-06T00:00:00Z
        XCTAssertEqual(
            widgetData.lastUpdated.timeIntervalSince1970,
            expected.timeIntervalSince1970,
            accuracy: 1
        )
    }

    func testDecoderRejectsLegacySnakeCasePayload() throws {
        // 防回归：Rust 端若退回 snake_case 键（2026-09 前的形状），解码必须失败可见
        let legacy = """
        {"version":1,"last_updated":"2026-09-06T00:00:00Z","repos":[],"summary":{"synced":0,"local_ahead":0,"remote_ahead":0,"diverged":0,"no_remote":0,"total":0}}
        """
        XCTAssertThrowsError(try makeDecoder().decode(WidgetData.self, from: Data(legacy.utf8)))
    }
}
