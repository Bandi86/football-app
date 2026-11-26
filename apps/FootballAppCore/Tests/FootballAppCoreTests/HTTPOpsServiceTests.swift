import Foundation
import Testing
@testable import FootballAppCore

/// Saját protokoll-osztály: a `StubURLProtocol` globális handlerjét használó
/// másik suite-tal párhuzamosan is fusson, egymást ne írják felül.
private final class OpsStubURLProtocol: URLProtocol {
    nonisolated(unsafe) static var handler: ((URLRequest) throws -> (HTTPURLResponse, Data))?

    override class func canInit(with request: URLRequest) -> Bool { true }
    override class func canonicalRequest(for request: URLRequest) -> URLRequest { request }

    override func startLoading() {
        guard let handler = Self.handler else {
            client?.urlProtocol(self, didFailWithError: URLError(.unknown))
            return
        }
        do {
            let (response, data) = try handler(request)
            client?.urlProtocol(self, didReceive: response, cacheStoragePolicy: .notAllowed)
            client?.urlProtocol(self, didLoad: data)
            client?.urlProtocolDidFinishLoading(self)
        } catch {
            client?.urlProtocol(self, didFailWithError: error)
        }
    }

    override func stopLoading() {}
}

@Suite(.serialized)
struct HTTPOpsServiceTests {
    private func makeService(
        handler: @escaping (URLRequest) throws -> (HTTPURLResponse, Data)
    ) -> HTTPOpsService {
        OpsStubURLProtocol.handler = handler
        let config = URLSessionConfiguration.ephemeral
        config.protocolClasses = [OpsStubURLProtocol.self]
        return HTTPOpsService(
            baseURL: URL(string: "http://127.0.0.1:56782")!,
            opsToken: "ops-token",
            session: URLSession(configuration: config)
        )
    }

    private func response(_ url: URL?, _ code: Int) -> HTTPURLResponse {
        HTTPURLResponse(url: url!, statusCode: code, httpVersion: nil, headerFields: nil)!
    }

    private var issuesBody: Data {
        Data("""
        {"issues":[{"id":"00000000-0000-0000-0000-000000000001","subject_kind":"match",
          "subject_id":"00000000-0000-0000-0000-000000000002","code":"missing_odds",
          "severity":"warning","first_seen_at":"2026-11-26T08:00:00Z",
          "last_seen_at":"2026-11-26T09:30:00Z","resolved_at":null,"source":"daily-check"}],
         "count":1}
        """.utf8)
    }

    @Test func issuesSendsFiltersAndOpsToken() async throws {
        defer { OpsStubURLProtocol.handler = nil }
        let captured = CapturedRequest()
        let service = makeService { request in
            captured.store(request)
            return (self.response(request.url, 200), self.issuesBody)
        }
        let result = try await service.issues(query: IssuesQueryDTO(
            scope: "day", date: "2026-11-26", code: "missing_odds",
            severity: "warning", openOnly: true
        ))
        let request = try #require(captured.value)
        #expect(request.url?.path == "/v1/issues")
        #expect(request.url?.query?.contains("scope=day") == true)
        #expect(request.url?.query?.contains("date=2026-11-26") == true)
        #expect(request.url?.query?.contains("code=missing_odds") == true)
        #expect(request.url?.query?.contains("severity=warning") == true)
        #expect(request.url?.query?.contains("open_only=true") == true)
        #expect(request.value(forHTTPHeaderField: "Authorization") == "Bearer ops-token")
        #expect(result.count == 1)
        #expect(result.issues[0].code == .missingOdds)
    }

    @Test func historySendsClampedLimitAndDecodesEntries() async throws {
        defer { OpsStubURLProtocol.handler = nil }
        let captured = CapturedRequest()
        let body = Data("""
        {"entries":[{"at":"2026-11-26T10:00:00Z","origin":"audit","actor":"ui",
          "action":"note_added","field_key":null,"before":null,"after":null,"source":null}]}
        """.utf8)
        let service = makeService { request in
            captured.store(request)
            return (self.response(request.url, 200), body)
        }
        let result = try await service.history(
            matchID: UUID(uuidString: "00000000-0000-0000-0000-000000000002")!, limit: 500
        )
        #expect(captured.value?.url?.path == "/v1/match/00000000-0000-0000-0000-000000000002/history")
        #expect(captured.value?.url?.query?.contains("limit=200") == true)
        #expect(result.entries[0].action == "note_added")
    }

    @Test func opsErrorsMapToUserMessages() async throws {
        defer { OpsStubURLProtocol.handler = nil }
        for code in [401, 400, 500] {
            let service = makeService { request in
                (self.response(request.url, code), Data())
            }
            var mapped: APIError?
            do {
                _ = try await service.issues(query: IssuesQueryDTO())
            } catch let error as APIError {
                mapped = error
            }
            #expect(mapped != nil, "A \(code) válasznak hibát kellett volna adnia")
            switch code {
            case 401: #expect(mapped == .unauthorized)
            case 400: #expect(mapped == .badRequest)
            default: #expect(mapped == .server)
            }
        }
    }
}
