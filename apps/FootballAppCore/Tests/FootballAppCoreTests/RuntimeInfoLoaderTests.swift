import Foundation
import Testing
@testable import FootballAppCore

@Suite struct RuntimeInfoLoaderTests {
    @Test func loadsRuntimeInfoFromFile() throws {
        let url = FileManager.default.temporaryDirectory
            .appendingPathComponent(UUID().uuidString + ".json")
        let json = #"{"port": 56782, "read_token": "abc", "ops_token": "def"}"#
        try Data(json.utf8).write(to: url)
        defer { try? FileManager.default.removeItem(at: url) }

        let info = try RuntimeInfoLoader(fileURL: url).load()
        #expect(info.port == 56782)
        #expect(info.readToken == "abc")
        #expect(info.opsToken == "def")
    }

    @Test func missingFileThrowsRuntimeMissing() {
        let url = FileManager.default.temporaryDirectory
            .appendingPathComponent(UUID().uuidString + ".json")
        #expect(throws: APIError.runtimeMissing) {
            try RuntimeInfoLoader(fileURL: url).load()
        }
    }

    @Test func malformedJSONThrowsDecoding() throws {
        let url = FileManager.default.temporaryDirectory
            .appendingPathComponent(UUID().uuidString + ".json")
        try Data("nem json".utf8).write(to: url)
        defer { try? FileManager.default.removeItem(at: url) }
        #expect(throws: APIError.decoding) {
            try RuntimeInfoLoader(fileURL: url).load()
        }
    }
}
