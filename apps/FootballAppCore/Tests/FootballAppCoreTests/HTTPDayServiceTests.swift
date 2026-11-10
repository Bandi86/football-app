import Foundation
import Testing
@testable import FootballAppCore

/// Szálbiztos tároló arra, hogy a `URLProtocol` callback szálán rögzített kérést
/// a teszt taszkjában lehessen ellenőrizni.
final class CapturedRequest: @unchecked Sendable {
    private let lock = NSLock()
    private var storage: URLRequest?

    func store(_ request: URLRequest) {
        lock.lock()
        defer { lock.unlock() }
        storage = request
    }

    var value: URLRequest? {
        lock.lock()
        defer { lock.unlock() }
        return storage
    }
}

@Suite(.serialized)
struct HTTPDayServiceTests {
    private func makeService(
        handler: @escaping (URLRequest) throws -> (HTTPURLResponse, Data)
    ) -> HTTPDayService {
        StubURLProtocol.handler = handler
        let config = URLSessionConfiguration.ephemeral
        config.protocolClasses = [StubURLProtocol.self]
        let session = URLSession(configuration: config)
        return HTTPDayService(
            baseURL: URL(string: "http://127.0.0.1:56782")!,
            readToken: "test-token",
            session: session
        )
    }

    private func response(_ url: URL?, _ code: Int) -> HTTPURLResponse {
        HTTPURLResponse(url: url!, statusCode: code, httpVersion: nil, headerFields: nil)!
    }

    private var sampleDate: Date {
        Calendar(identifier: .gregorian).date(from: DateComponents(year: 2026, month: 11, day: 9))!
    }

    @Test func sendsBearerTokenAndParsesMatches() async throws {
        defer { StubURLProtocol.handler = nil }
        let captured = CapturedRequest()
        let service = makeService { request in
            captured.store(request)
            let body = """
            {"date":"2026-11-09","timezone":"Europe/Budapest","matches":[
              {"id":"00000000-0000-0000-0000-000000000101","kickoff_at":"2026-11-09T18:00:00Z",
               "status":"scheduled","home":"Újpest","away":"Győr","home_score":null,"away_score":null,
               "publish_state":"verified_publish","blockers":[]}
            ]}
            """.data(using: .utf8)!
            return (self.response(request.url, 200), body)
        }
        let matches = try await service.day(for: sampleDate)
        let request = try #require(captured.value, "A stub nem rögzített kérést")
        #expect(request.url?.path == "/v1/day/2026-11-09")
        #expect(request.value(forHTTPHeaderField: "Authorization") == "Bearer test-token")
        #expect(matches.count == 1)
        #expect(matches[0].home == "Újpest")
    }

    @Test func mapsStatusCodesToErrors() async {
        defer { StubURLProtocol.handler = nil }
        let cases: [(Int, APIError)] = [
            (401, .unauthorized),
            (400, .badRequest),
            (404, .server),
            (500, .server),
        ]
        for (code, expected) in cases {
            let service = makeService { request in (self.response(request.url, code), Data()) }
            do {
                _ = try await service.day(for: sampleDate)
                Issue.record("Várt hiba a \(code) státuszra")
            } catch let error as APIError {
                #expect(error == expected)
            } catch {
                Issue.record("Váratlan hibatípus: \(error)")
            }
        }
    }

    /// A `URLSession` a megszakított kérést `URLError(.cancelled)` formában dobja,
    /// amit a szolgáltatásnak `CancellationError`-rá kell alakítania, hogy a hívó
    /// figyelmen kívül hagyhassa (és ne hálózati hibaként jelenjen meg).
    @Test func cancellationIsRethrownAsCancellationError() async {
        defer { StubURLProtocol.handler = nil }
        let service = makeService { _ in
            throw URLError(.cancelled)
        }
        do {
            _ = try await service.day(for: sampleDate)
            Issue.record("Várt megszakítási hiba")
        } catch is CancellationError {
            // Elvárt: a megszakítást nem alakítjuk hálózati hibává.
        } catch {
            Issue.record("Váratlan hibatípus: \(error)")
        }
    }

    @Test func networkErrorMapsToNetwork() async {
        defer { StubURLProtocol.handler = nil }
        let service = makeService { _ in
            throw URLError(.notConnectedToInternet)
        }
        do {
            _ = try await service.day(for: sampleDate)
            Issue.record("Várt hálózati hiba")
        } catch let error as APIError {
            #expect(error == .network)
        } catch {
            Issue.record("Váratlan hibatípus: \(error)")
        }
    }

    @Test func badJSONMapsToDecoding() async {
        defer { StubURLProtocol.handler = nil }
        let service = makeService { request in
            (self.response(request.url, 200), Data("nem json".utf8))
        }
        do {
            _ = try await service.day(for: sampleDate)
            Issue.record("Várt decoding hiba")
        } catch let error as APIError {
            #expect(error == .decoding)
        } catch {
            Issue.record("Váratlan hibatípus: \(error)")
        }
    }
}
