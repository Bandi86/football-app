# SwiftUI „Ma" kliens — Implementációs terv

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Egy macOS SwiftUI „Ma" képernyő, amely a futó `footballd` helyi API-ról megjeleníti a kiválasztott nap ellenőrzött meccslistáját; a logika UI-mentes SwiftPM magban, parancssorból tesztelhetően.

**Architecture:** Két egység. `apps/FootballAppCore/` SwiftPM library (DTO-k, API kliens, `runtime.json` parser) — Xcode nélkül `swift test`-elhető. `apps/FootballApp/` vékony Xcode `.app` (`@Observable` `DayModel` + SwiftUI nézetek), a Core-ra épül, XcodeGen `project.yml`-ból generált projekttel. A kliens csak olvas, csak `127.0.0.1`-hez kapcsolódik, a read tokent a `runtime.json`-ból veszi.

**Tech Stack:** Swift 6.4 / SwiftPM 6.0, SwiftUI + Observation (`@Observable`), Foundation `URLSession` (async/await), XCTest, XcodeGen. Deployment target: macOS 14.0.

**Spec:** [SwiftUI „Ma" kliens — Design](../specs/2026-11-10-swiftui-daily-list-design.md)

## Global Constraints

- A Core (`apps/FootballAppCore/`) UI- és Xcode-mentes; **`swift test`-tel a Command Line Tools-szal is fut** (nincs teljes Xcode a gépen).
- A `.app` (`apps/FootballApp/`) build/teszt/futtatás **csak teljes Xcode + XcodeGen után** lehetséges — ez **fáziskapu** (lásd Task 4).
- A kliens csak `http://127.0.0.1:{port}`-hoz kapcsolódik; a read tokent a `~/Library/Application Support/FootballAppV2/runtime.json` adja (`port`, `read_token`, `ops_token`); az ops tokent ez a szelet nem használja.
- Hiányzó adat `nil`/státusz, **soha nem 0** (`home_score`/`away_score` opcionális).
- Minden felhasználói szöveg magyar; a kód azonosítói angolul.
- Production hálózati/kódolási úton nincs force-unwrap (`!`); a tesztekben megengedett.
- A repo publikus: a tokenek nem kerülnek naplóba, a `runtime.json` gitignore-olt.
- **Commit/staging lépés csak a felhasználó kifejezett kérésére hajtható végre** (AGENTS.md §9). A lépés a tervben szerepel, de a végrehajtó jóváhagyás nélkül kihagyja.

## Fájlszerkezet

```text
apps/
  FootballAppCore/
    Package.swift
    Sources/FootballAppCore/
      APIError.swift            # tipizált hiba + magyar üzenet
      RuntimeInfo.swift         # Codable (port, read_token, ops_token)
      RuntimeInfoLoader.swift   # runtime.json beolvasó + RuntimeInfoLoading protocol
      DayModels.swift           # DayResponse, DayMatch, Blocker, PublishState
      APIDecoding.swift         # JSONDecoder factory (ISO-8601 dátum)
      DayService.swift          # DayService protocol
      HTTPDayService.swift      # URLSession implementáció
    Tests/FootballAppCoreTests/
      RuntimeInfoLoaderTests.swift
      DayModelsTests.swift
      HTTPDayServiceTests.swift
      StubURLProtocol.swift
  FootballApp/
    Info.plist                  # ATS NSAllowsLocalNetworking, LSMinimumSystemVersion
    FootballAppApp.swift        # @main
    DayModel.swift              # @Observable @MainActor
    DayView.swift
    MatchRowView.swift
    StatusBadge.swift
    ErrorStateView.swift
    EmptyStateView.swift
  FootballAppTests/
    DayModelTests.swift
  project.yml                   # XcodeGen
```

---

### Task 1: Core csomag, APIError, RuntimeInfo, RuntimeInfoLoader

**Files:**
- Create: `apps/FootballAppCore/Package.swift`
- Create: `apps/FootballAppCore/Sources/FootballAppCore/APIError.swift`
- Create: `apps/FootballAppCore/Sources/FootballAppCore/RuntimeInfo.swift`
- Create: `apps/FootballAppCore/Sources/FootballAppCore/RuntimeInfoLoader.swift`
- Test: `apps/FootballAppCore/Tests/FootballAppCoreTests/RuntimeInfoLoaderTests.swift`

**Interfaces:**
- Produces:
  - `public enum APIError: Error, Equatable, Sendable { case runtimeMissing, unauthorized, badRequest, server, network, decoding }` + `var userMessage: String`.
  - `public struct RuntimeInfo: Codable, Equatable, Sendable { let port: Int; let readToken: String; let opsToken: String? }`.
  - `public protocol RuntimeInfoLoading: Sendable { func load() throws -> RuntimeInfo }`.
  - `public struct RuntimeInfoLoader: RuntimeInfoLoading { let fileURL: URL; init(fileURL: URL = defaultURL); static var defaultURL: URL; func load() throws -> RuntimeInfo }`.

- [ ] **Step 1: Írd meg a bukó tesztet**

`apps/FootballAppCore/Tests/FootballAppCoreTests/RuntimeInfoLoaderTests.swift`:

```swift
import XCTest
@testable import FootballAppCore

final class RuntimeInfoLoaderTests: XCTestCase {
    func testLoadsRuntimeInfoFromFile() throws {
        let url = FileManager.default.temporaryDirectory
            .appendingPathComponent(UUID().uuidString + ".json")
        let json = #"{"port": 56782, "read_token": "abc", "ops_token": "def"}"#
        try Data(json.utf8).write(to: url)
        defer { try? FileManager.default.removeItem(at: url) }

        let info = try RuntimeInfoLoader(fileURL: url).load()
        XCTAssertEqual(info.port, 56782)
        XCTAssertEqual(info.readToken, "abc")
        XCTAssertEqual(info.opsToken, "def")
    }

    func testMissingFileThrowsRuntimeMissing() {
        let url = FileManager.default.temporaryDirectory
            .appendingPathComponent(UUID().uuidString + ".json")
        XCTAssertThrowsError(try RuntimeInfoLoader(fileURL: url).load()) { error in
            XCTAssertEqual(error as? APIError, .runtimeMissing)
        }
    }

    func testMalformedJSONThrowsDecoding() throws {
        let url = FileManager.default.temporaryDirectory
            .appendingPathComponent(UUID().uuidString + ".json")
        try Data("nem json".utf8).write(to: url)
        defer { try? FileManager.default.removeItem(at: url) }
        XCTAssertThrowsError(try RuntimeInfoLoader(fileURL: url).load()) { error in
            XCTAssertEqual(error as? APIError, .decoding)
        }
    }
}
```

- [ ] **Step 2: Futtasd, igazold a bukást**

Run: `cd apps/FootballAppCore && swift test`
Expected: FAIL — `no such module 'FootballAppCore'` / a csomag/típusok hiányoznak.

- [ ] **Step 3: Írd meg a csomagot és a típusokat**

`apps/FootballAppCore/Package.swift`:

```swift
// swift-tools-version: 6.0
import PackageDescription

let package = Package(
    name: "FootballAppCore",
    platforms: [.macOS(.v14)],
    products: [
        .library(name: "FootballAppCore", targets: ["FootballAppCore"])
    ],
    targets: [
        .target(name: "FootballAppCore"),
        .testTarget(
            name: "FootballAppCoreTests",
            dependencies: ["FootballAppCore"]
        )
    ]
)
```

`apps/FootballAppCore/Sources/FootballAppCore/APIError.swift`:

```swift
import Foundation

public enum APIError: Error, Equatable, Sendable {
    case runtimeMissing
    case unauthorized
    case badRequest
    case server
    case network
    case decoding

    public var userMessage: String {
        switch self {
        case .runtimeMissing:
            return "A footballd daemon nem fut (runtime.json nem található). Indítsd el a daemont."
        case .unauthorized:
            return "Érvénytelen vagy hiányzó hozzáférési token."
        case .badRequest:
            return "Hibás dátum."
        case .server:
            return "A szerver hibát adott. Próbáld újra."
        case .network:
            return "Nem érhető el a helyi API. Fut a daemon?"
        case .decoding:
            return "Váratlan válaszformátum."
        }
    }
}
```

`apps/FootballAppCore/Sources/FootballAppCore/RuntimeInfo.swift`:

```swift
import Foundation

public struct RuntimeInfo: Codable, Equatable, Sendable {
    public let port: Int
    public let readToken: String
    public let opsToken: String?

    enum CodingKeys: String, CodingKey {
        case port
        case readToken = "read_token"
        case opsToken = "ops_token"
    }

    public init(port: Int, readToken: String, opsToken: String? = nil) {
        self.port = port
        self.readToken = readToken
        self.opsToken = opsToken
    }
}
```

`apps/FootballAppCore/Sources/FootballAppCore/RuntimeInfoLoader.swift`:

```swift
import Foundation

public protocol RuntimeInfoLoading: Sendable {
    func load() throws -> RuntimeInfo
}

public struct RuntimeInfoLoader: RuntimeInfoLoading {
    public let fileURL: URL

    public static var defaultURL: URL {
        FileManager.default.homeDirectoryForCurrentUser
            .appendingPathComponent("Library/Application Support/FootballAppV2/runtime.json")
    }

    public init(fileURL: URL = RuntimeInfoLoader.defaultURL) {
        self.fileURL = fileURL
    }

    public func load() throws -> RuntimeInfo {
        guard let data = try? Data(contentsOf: fileURL) else {
            throw APIError.runtimeMissing
        }
        do {
            return try JSONDecoder().decode(RuntimeInfo.self, from: data)
        } catch {
            throw APIError.decoding
        }
    }
}
```

- [ ] **Step 4: Futtasd, igazold a zöldet**

Run: `cd apps/FootballAppCore && swift test`
Expected: PASS — `RuntimeInfoLoaderTests` 3 tesztje zöld.

- [ ] **Step 5: Commit (csak kifejezett felhasználói kérésre)**

```bash
git add apps/FootballAppCore
git commit -m "feat: add FootballAppCore package with runtime.json loader"
```

---

### Task 2: DTO-k és JSON-dekódolás

**Files:**
- Create: `apps/FootballAppCore/Sources/FootballAppCore/DayModels.swift`
- Create: `apps/FootballAppCore/Sources/FootballAppCore/APIDecoding.swift`
- Test: `apps/FootballAppCore/Tests/FootballAppCoreTests/DayModelsTests.swift`

**Interfaces:**
- Consumes: `APIError` (Task 1).
- Produces:
  - `public struct DayResponse: Decodable, Sendable { let date: String; let timezone: String; let matches: [DayMatch] }`.
  - `public struct DayMatch: Decodable, Equatable, Identifiable, Sendable { let id: UUID; let kickoffAt: Date; let status: String; let home: String; let away: String; let homeScore: Int?; let awayScore: Int?; let publishState: PublishState; let blockers: [Blocker] }`.
  - `public struct Blocker: Decodable, Equatable, Sendable { let ruleKey: String; let severity: String; let field: String?; let message: String }`.
  - `public enum PublishState: String, Decodable, Sendable { case verifiedPublish, provisionalPublish, withheld, quarantined, unknown }`.
  - `public enum APIDecoding { static func makeDecoder() -> JSONDecoder }`.

- [ ] **Step 1: Írd meg a bukó teszteket**

`apps/FootballAppCore/Tests/FootballAppCoreTests/DayModelsTests.swift`:

```swift
import XCTest
@testable import FootballAppCore

final class DayModelsTests: XCTestCase {
    func testDecodesDayResponseFixture() throws {
        let json = """
        {"date":"2026-11-09","timezone":"Europe/Budapest","matches":[
          {"id":"00000000-0000-0000-0000-000000000101","kickoff_at":"2026-11-09T18:00:00Z",
           "status":"scheduled","home":"Újpest","away":"Győr","home_score":null,"away_score":null,
           "publish_state":"verified_publish","blockers":[]}
        ]}
        """.data(using: .utf8)!
        let response = try APIDecoding.makeDecoder().decode(DayResponse.self, from: json)
        XCTAssertEqual(response.date, "2026-11-09")
        XCTAssertEqual(response.timezone, "Europe/Budapest")
        XCTAssertEqual(response.matches.count, 1)
        let match = response.matches[0]
        XCTAssertEqual(match.home, "Újpest")
        XCTAssertEqual(match.away, "Győr")
        XCTAssertNil(match.homeScore)
        XCTAssertNil(match.awayScore)
        XCTAssertEqual(match.publishState, .verifiedPublish)
        XCTAssertTrue(match.blockers.isEmpty)
    }

    func testUnknownPublishStateMapsToUnknown() throws {
        let json = """
        {"date":"2026-11-09","timezone":"Europe/Budapest","matches":[
          {"id":"00000000-0000-0000-0000-000000000102","kickoff_at":"2026-11-09T19:00:00Z",
           "status":"scheduled","home":"A","away":"B","home_score":null,"away_score":null,
           "publish_state":"valami_uj","blockers":[]}
        ]}
        """.data(using: .utf8)!
        let response = try APIDecoding.makeDecoder().decode(DayResponse.self, from: json)
        XCTAssertEqual(response.matches[0].publishState, .unknown)
    }

    func testDecodesResultAndBlocker() throws {
        let json = """
        {"date":"2026-11-09","timezone":"Europe/Budapest","matches":[
          {"id":"00000000-0000-0000-0000-000000000103","kickoff_at":"2026-11-09T18:00:00Z",
           "status":"finished","home":"A","away":"B","home_score":2,"away_score":1,
           "publish_state":"withheld","blockers":[
             {"rule_key":"MATERIAL_SOURCE_CONFLICT","severity":"blocker","field":"kickoff_at","message":"Források eltérnek."}
           ]}
        ]}
        """.data(using: .utf8)!
        let match = try APIDecoding.makeDecoder().decode(DayResponse.self, from: json).matches[0]
        XCTAssertEqual(match.homeScore, 2)
        XCTAssertEqual(match.awayScore, 1)
        XCTAssertEqual(match.publishState, .withheld)
        XCTAssertEqual(match.blockers.first?.ruleKey, "MATERIAL_SOURCE_CONFLICT")
        XCTAssertEqual(match.blockers.first?.severity, "blocker")
    }
}
```

- [ ] **Step 2: Futtasd, igazold a bukást**

Run: `cd apps/FootballAppCore && swift test`
Expected: FAIL — `DayResponse`/`APIDecoding`/`DayMatch` nem található.

- [ ] **Step 3: Írd meg a modelleket és a dekódolót**

`apps/FootballAppCore/Sources/FootballAppCore/DayModels.swift`:

```swift
import Foundation

public enum PublishState: String, Decodable, Sendable {
    case verifiedPublish = "verified_publish"
    case provisionalPublish = "provisional_publish"
    case withheld = "withheld"
    case quarantined = "quarantined"
    case unknown = "unknown"

    public init(from decoder: Decoder) throws {
        let raw = try decoder.singleValueContainer().decode(String.self)
        self = PublishState(rawValue: raw) ?? .unknown
    }
}

public struct Blocker: Decodable, Equatable, Sendable {
    public let ruleKey: String
    public let severity: String
    public let field: String?
    public let message: String

    enum CodingKeys: String, CodingKey {
        case ruleKey = "rule_key"
        case severity
        case field
        case message
    }
}

public struct DayMatch: Decodable, Equatable, Identifiable, Sendable {
    public let id: UUID
    public let kickoffAt: Date
    public let status: String
    public let home: String
    public let away: String
    public let homeScore: Int?
    public let awayScore: Int?
    public let publishState: PublishState
    public let blockers: [Blocker]

    enum CodingKeys: String, CodingKey {
        case id
        case kickoffAt = "kickoff_at"
        case status
        case home
        case away
        case homeScore = "home_score"
        case awayScore = "away_score"
        case publishState = "publish_state"
        case blockers
    }
}

public struct DayResponse: Decodable, Sendable {
    public let date: String
    public let timezone: String
    public let matches: [DayMatch]
}
```

`apps/FootballAppCore/Sources/FootballAppCore/APIDecoding.swift`:

```swift
import Foundation

public enum APIDecoding {
    public static func makeDecoder() -> JSONDecoder {
        let decoder = JSONDecoder()
        decoder.dateDecodingStrategy = .custom { decoder in
            let container = try decoder.singleValueContainer()
            let string = try container.decode(String.self)
            let fractional = ISO8601DateFormatter()
            fractional.formatOptions = [.withInternetDateTime, .withFractionalSeconds]
            if let date = fractional.date(from: string) { return date }
            let plain = ISO8601DateFormatter()
            plain.formatOptions = [.withInternetDateTime]
            if let date = plain.date(from: string) { return date }
            throw DecodingError.dataCorruptedError(
                in: container,
                debugDescription: "Érvénytelen ISO-8601 dátum: \(string)"
            )
        }
        return decoder
    }
}
```

- [ ] **Step 4: Futtasd, igazold a zöldet**

Run: `cd apps/FootballAppCore && swift test`
Expected: PASS — `DayModelsTests` 3 tesztje + a Task 1 tesztjei zöldek.

- [ ] **Step 5: Commit (csak kifejezett felhasználói kérésre)**

```bash
git add apps/FootballAppCore
git commit -m "feat: add day DTOs and JSON decoding"
```

---

### Task 3: DayService protocol és HTTPDayService

**Files:**
- Create: `apps/FootballAppCore/Sources/FootballAppCore/DayService.swift`
- Create: `apps/FootballAppCore/Sources/FootballAppCore/HTTPDayService.swift`
- Test: `apps/FootballAppCore/Tests/FootballAppCoreTests/StubURLProtocol.swift`
- Test: `apps/FootballAppCore/Tests/FootballAppCoreTests/HTTPDayServiceTests.swift`

**Interfaces:**
- Consumes: `DayResponse`, `DayMatch`, `APIDecoding`, `APIError` (Task 1–2).
- Produces:
  - `public protocol DayService: Sendable { func day(for date: Date) async throws -> [DayMatch] }`.
  - `public struct HTTPDayService: DayService { init(baseURL: URL, readToken: String, session: URLSession = .shared, calendar: Calendar = Calendar(identifier: .gregorian)) }`.

- [ ] **Step 1: Írd meg a bukó teszteket**

`apps/FootballAppCore/Tests/FootballAppCoreTests/StubURLProtocol.swift`:

```swift
import Foundation

final class StubURLProtocol: URLProtocol {
    nonisolated(unsafe) static var handler: ((URLRequest) throws -> (HTTPURLResponse, Data))?

    override class func canInit(with request: URLRequest) -> Bool { true }
    override class func canonicalRequest(for request: URLRequest) -> URLRequest { request }

    override func startLoading() {
        guard let handler = StubURLProtocol.handler else {
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
```

`apps/FootballAppCore/Tests/FootballAppCoreTests/HTTPDayServiceTests.swift`:

```swift
import XCTest
@testable import FootballAppCore

final class HTTPDayServiceTests: XCTestCase {
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

    func testSendsBearerTokenAndParsesMatches() async throws {
        let service = makeService { request in
            XCTAssertEqual(request.url?.path, "/v1/day/2026-11-09")
            XCTAssertEqual(request.value(forHTTPHeaderField: "Authorization"), "Bearer test-token")
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
        XCTAssertEqual(matches.count, 1)
        XCTAssertEqual(matches[0].home, "Újpest")
    }

    func testMapsStatusCodesToErrors() async {
        let cases: [(Int, APIError)] = [(401, .unauthorized), (400, .badRequest), (500, .server)]
        for (code, expected) in cases {
            let service = makeService { request in (self.response(request.url, code), Data()) }
            do {
                _ = try await service.day(for: sampleDate)
                XCTFail("Várt hiba a \(code) státuszra")
            } catch let error as APIError {
                XCTAssertEqual(error, expected)
            } catch {
                XCTFail("Váratlan hibatípus: \(error)")
            }
        }
    }

    func testBadJSONMapsToDecoding() async {
        let service = makeService { request in
            (self.response(request.url, 200), Data("nem json".utf8))
        }
        do {
            _ = try await service.day(for: sampleDate)
            XCTFail("Várt decoding hiba")
        } catch let error as APIError {
            XCTAssertEqual(error, .decoding)
        } catch {
            XCTFail("Váratlan hibatípus: \(error)")
        }
    }
}
```

- [ ] **Step 2: Futtasd, igazold a bukást**

Run: `cd apps/FootballAppCore && swift test`
Expected: FAIL — `DayService`/`HTTPDayService` nem található.

- [ ] **Step 3: Írd meg a service-t**

`apps/FootballAppCore/Sources/FootballAppCore/DayService.swift`:

```swift
import Foundation

public protocol DayService: Sendable {
    func day(for date: Date) async throws -> [DayMatch]
}
```

`apps/FootballAppCore/Sources/FootballAppCore/HTTPDayService.swift`:

```swift
import Foundation

public struct HTTPDayService: DayService {
    private let baseURL: URL
    private let readToken: String
    private let session: URLSession
    private let calendar: Calendar

    public init(
        baseURL: URL,
        readToken: String,
        session: URLSession = .shared,
        calendar: Calendar = Calendar(identifier: .gregorian)
    ) {
        self.baseURL = baseURL
        self.readToken = readToken
        self.session = session
        self.calendar = calendar
    }

    public func day(for date: Date) async throws -> [DayMatch] {
        let dateString = Self.formatDate(date, calendar: calendar)
        let request = makeRequest(dateString: dateString)

        let data: Data
        let response: URLResponse
        do {
            (data, response) = try await session.data(for: request)
        } catch {
            throw APIError.network
        }
        guard let http = response as? HTTPURLResponse else {
            throw APIError.network
        }
        switch http.statusCode {
        case 200...299:
            do {
                return try APIDecoding.makeDecoder().decode(DayResponse.self, from: data).matches
            } catch {
                throw APIError.decoding
            }
        case 401:
            throw APIError.unauthorized
        case 400:
            throw APIError.badRequest
        default:
            throw APIError.server
        }
    }

    private func makeRequest(dateString: String) -> URLRequest {
        let url = baseURL.appendingPathComponent("v1/day/\(dateString)")
        var request = URLRequest(url: url)
        request.httpMethod = "GET"
        request.setValue("Bearer \(readToken)", forHTTPHeaderField: "Authorization")
        return request
    }

    static func formatDate(_ date: Date, calendar: Calendar) -> String {
        let formatter = DateFormatter()
        formatter.calendar = calendar
        formatter.timeZone = calendar.timeZone
        formatter.locale = Locale(identifier: "en_US_POSIX")
        formatter.dateFormat = "yyyy-MM-dd"
        return formatter.string(from: date)
    }
}
```

- [ ] **Step 4: Futtasd, igazold a zöldet**

Run: `cd apps/FootballAppCore && swift test`
Expected: PASS — `HTTPDayServiceTests` 3 tesztje + a korábbiak zöldek.

- [ ] **Step 5: Commit (csak kifejezett felhasználói kérésre)**

```bash
git add apps/FootballAppCore
git commit -m "feat: add HTTPDayService against the local day endpoint"
```

---

### Task 4: Xcode app — DayModel, SwiftUI nézetek, projekt (FÁZISKAPU: teljes Xcode)

> **FÁZISKAPU:** ez a task csak **teljes Xcode + XcodeGen** telepítése után hajtható végre (`xcodegen generate`, `xcodebuild`). A gépen jelenleg nincs teljes Xcode. A Task 1–3 a Core-ra vonatkozik és azok Xcode nélkül lezárhatók; ez a task a hiányzó eszközökig **nem futtatható**, csak a fájlok írhatók meg.

**Files:**
- Create: `apps/project.yml`
- Create: `apps/FootballApp/Info.plist`
- Create: `apps/FootballApp/FootballAppApp.swift`
- Create: `apps/FootballApp/DayModel.swift`
- Create: `apps/FootballApp/DayView.swift`
- Create: `apps/FootballApp/MatchRowView.swift`
- Create: `apps/FootballApp/StatusBadge.swift`
- Create: `apps/FootballApp/ErrorStateView.swift`
- Create: `apps/FootballApp/EmptyStateView.swift`
- Test: `apps/FootballAppTests/DayModelTests.swift`

**Interfaces:**
- Consumes: `RuntimeInfoLoading`, `RuntimeInfo`, `DayService`, `HTTPDayService`, `DayMatch`, `PublishState`, `APIError`, `APIDecoding`, `DayResponse` (Task 1–3).
- Produces: `DayModel` (`@Observable @MainActor`, `state`, `selectedDate`, `load()`); a SwiftUI nézetek; a XcodeGen projekt.

- [ ] **Step 1: Írd meg a DayModel bukó tesztet**

`apps/FootballAppTests/DayModelTests.swift`:

```swift
import XCTest
import FootballAppCore
@testable import FootballApp

@MainActor
final class DayModelTests: XCTestCase {
    private struct StubLoader: RuntimeInfoLoading {
        let result: Result<RuntimeInfo, APIError>
        func load() throws -> RuntimeInfo { try result.get() }
    }

    private struct StubService: DayService {
        let result: Result<[DayMatch], APIError>
        func day(for date: Date) async throws -> [DayMatch] { try result.get() }
    }

    private let info = RuntimeInfo(port: 56782, readToken: "t")

    func testEmptyState() async {
        let model = DayModel(
            loader: StubLoader(result: .success(info)),
            serviceFactory: { _ in StubService(result: .success([])) }
        )
        await model.load()
        XCTAssertEqual(model.state, .empty)
    }

    func testErrorWhenRuntimeMissing() async {
        let model = DayModel(
            loader: StubLoader(result: .failure(.runtimeMissing)),
            serviceFactory: { _ in StubService(result: .success([])) }
        )
        await model.load()
        XCTAssertEqual(model.state, .error(.runtimeMissing))
    }

    func testErrorOnServiceFailure() async {
        let model = DayModel(
            loader: StubLoader(result: .success(info)),
            serviceFactory: { _ in StubService(result: .failure(.unauthorized)) }
        )
        await model.load()
        XCTAssertEqual(model.state, .error(.unauthorized))
    }

    func testLoadedState() async throws {
        let json = """
        {"date":"2026-11-09","timezone":"Europe/Budapest","matches":[
          {"id":"00000000-0000-0000-0000-000000000101","kickoff_at":"2026-11-09T18:00:00Z",
           "status":"scheduled","home":"Újpest","away":"Győr","home_score":null,"away_score":null,
           "publish_state":"verified_publish","blockers":[]}
        ]}
        """.data(using: .utf8)!
        let matches = try APIDecoding.makeDecoder().decode(DayResponse.self, from: json).matches
        let model = DayModel(
            loader: StubLoader(result: .success(info)),
            serviceFactory: { _ in StubService(result: .success(matches)) }
        )
        await model.load()
        XCTAssertEqual(model.state, .loaded(matches))
    }
}
```

- [ ] **Step 2: (Xcode után) futtasd, igazold a bukást**

Run: `cd apps && xcodegen generate && xcodebuild -scheme FootballApp test`
Expected: FAIL — `DayModel` és a nézetek nem léteznek (fordítási hiba). *(Xcode nélkül ez a lépés nem futtatható — fáziskapu.)*

- [ ] **Step 3: Írd meg a `DayModel`-t**

`apps/FootballApp/DayModel.swift`:

```swift
import Foundation
import Observation
import FootballAppCore

@MainActor
@Observable
final class DayModel {
    enum State: Equatable {
        case idle
        case loading
        case loaded([DayMatch])
        case empty
        case error(APIError)
    }

    var selectedDate: Date = Date()
    var state: State = .idle

    private let loader: any RuntimeInfoLoading
    private let serviceFactory: (RuntimeInfo) throws -> DayService

    init(
        loader: any RuntimeInfoLoading = RuntimeInfoLoader(),
        serviceFactory: ((RuntimeInfo) throws -> DayService)? = nil
    ) {
        self.loader = loader
        self.serviceFactory = serviceFactory ?? DayModel.makeService
    }

    func load() async {
        state = .loading
        do {
            let info = try loader.load()
            let service = try serviceFactory(info)
            let matches = try await service.day(for: selectedDate)
            state = matches.isEmpty ? .empty : .loaded(matches)
        } catch let error as APIError {
            state = .error(error)
        } catch {
            state = .error(.network)
        }
    }

    private static func makeService(_ info: RuntimeInfo) throws -> DayService {
        guard let baseURL = URL(string: "http://127.0.0.1:\(info.port)") else {
            throw APIError.runtimeMissing
        }
        return HTTPDayService(baseURL: baseURL, readToken: info.readToken)
    }
}
```

- [ ] **Step 4: Írd meg a SwiftUI nézeteket**

`apps/FootballApp/FootballAppApp.swift`:

```swift
import SwiftUI

@main
struct FootballAppApp: App {
    @State private var model = DayModel()

    var body: some Scene {
        WindowGroup {
            DayView(model: model)
        }
    }
}
```

`apps/FootballApp/DayView.swift`:

```swift
import SwiftUI
import FootballAppCore

struct DayView: View {
    @Bindable var model: DayModel

    var body: some View {
        VStack(alignment: .leading, spacing: 12) {
            HStack {
                DatePicker("Nap", selection: $model.selectedDate, displayedComponents: .date)
                    .datePickerStyle(.compact)
                Spacer()
                Button {
                    Task { await model.load() }
                } label: {
                    Label("Frissítés", systemImage: "arrow.clockwise")
                }
            }
            .padding([.horizontal, .top])

            content
                .frame(maxWidth: .infinity, maxHeight: .infinity)
        }
        .task { await model.load() }
        .onChange(of: model.selectedDate) { Task { await model.load() } }
    }

    @ViewBuilder
    private var content: some View {
        switch model.state {
        case .idle, .loading:
            ProgressView("Betöltés…")
        case .empty:
            EmptyStateView()
        case .error(let error):
            ErrorStateView(message: error.userMessage) {
                Task { await model.load() }
            }
        case .loaded(let matches):
            List(matches) { match in
                MatchRowView(match: match)
            }
        }
    }
}
```

`apps/FootballApp/MatchRowView.swift`:

```swift
import SwiftUI
import FootballAppCore

struct MatchRowView: View {
    let match: DayMatch

    private static let kickoffFormatter: DateFormatter = {
        let formatter = DateFormatter()
        formatter.dateStyle = .none
        formatter.timeStyle = .short
        return formatter
    }()

    var body: some View {
        HStack(spacing: 12) {
            Text(Self.kickoffFormatter.string(from: match.kickoffAt))
                .font(.callout.monospacedDigit())
                .foregroundStyle(.secondary)
                .frame(width: 64, alignment: .leading)

            VStack(alignment: .leading, spacing: 2) {
                Text("\(match.home) – \(match.away)")
                Text(match.status)
                    .font(.caption)
                    .foregroundStyle(.secondary)
            }

            Spacer()

            if let home = match.homeScore, let away = match.awayScore {
                Text("\(home) – \(away)")
                    .font(.headline.monospacedDigit())
            }

            StatusBadge(state: match.publishState)
        }
        .padding(.vertical, 4)
    }
}
```

`apps/FootballApp/StatusBadge.swift`:

```swift
import SwiftUI
import FootballAppCore

struct StatusBadge: View {
    let state: PublishState

    var body: some View {
        Text(label)
            .font(.caption2.weight(.semibold))
            .padding(.horizontal, 8)
            .padding(.vertical, 3)
            .background(color.opacity(0.15), in: Capsule())
            .foregroundStyle(color)
    }

    private var label: String {
        switch state {
        case .verifiedPublish: return "ellenőrzött"
        case .provisionalPublish: return "ideiglenes"
        case .withheld: return "visszatartott"
        case .quarantined: return "karantén"
        case .unknown: return "ismeretlen"
        }
    }

    private var color: Color {
        switch state {
        case .verifiedPublish: return .green
        case .provisionalPublish: return .orange
        case .withheld, .quarantined: return .red
        case .unknown: return .gray
        }
    }
}
```

`apps/FootballApp/ErrorStateView.swift`:

```swift
import SwiftUI

struct ErrorStateView: View {
    let message: String
    let retry: () -> Void

    var body: some View {
        VStack(spacing: 12) {
            Image(systemName: "exclamationmark.triangle")
                .font(.largeTitle)
                .foregroundStyle(.orange)
            Text(message)
                .multilineTextAlignment(.center)
                .foregroundStyle(.secondary)
            Button("Újra", action: retry)
        }
        .padding()
    }
}
```

`apps/FootballApp/EmptyStateView.swift`:

```swift
import SwiftUI

struct EmptyStateView: View {
    var body: some View {
        VStack(spacing: 8) {
            Image(systemName: "calendar")
                .font(.largeTitle)
                .foregroundStyle(.secondary)
            Text("Nincs meccs ezen a napon.")
                .foregroundStyle(.secondary)
        }
        .padding()
    }
}
```

- [ ] **Step 5: Írd meg a projektet (XcodeGen + Info.plist)**

`apps/project.yml`:

```yaml
name: FootballApp
options:
  bundleIdPrefix: hu.bandi86
  deploymentTarget:
    macOS: "14.0"
packages:
  FootballAppCore:
    path: FootballAppCore
targets:
  FootballApp:
    type: application
    platform: macOS
    sources:
      - FootballApp
    info:
      path: FootballApp/Info.plist
    dependencies:
      - package: FootballAppCore
    settings:
      base:
        PRODUCT_BUNDLE_IDENTIFIER: hu.bandi86.FootballApp
  FootballAppTests:
    type: bundle.unit-test
    platform: macOS
    sources:
      - FootballAppTests
    dependencies:
      - target: FootballApp
```

`apps/FootballApp/Info.plist`:

```xml
<?xml version="1.0" encoding="UTF-8"?>
<!DOCTYPE plist PUBLIC "-//Apple//DTD PLIST 1.0//EN" "http://www.apple.com/DTDs/PropertyList-1.0.dtd">
<plist version="1.0">
<dict>
    <key>CFBundleName</key>
    <string>FootballApp</string>
    <key>LSMinimumSystemVersion</key>
    <string>14.0</string>
    <key>NSAppTransportSecurity</key>
    <dict>
        <key>NSAllowsLocalNetworking</key>
        <true/>
    </dict>
</dict>
</plist>
```

- [ ] **Step 6: (Xcode után) generáld a projektet és futtasd a teszteket**

Run: `cd apps && xcodegen generate && xcodebuild -scheme FootballApp test`
Expected: PASS — `DayModelTests` 4 tesztje zöld. *(Fáziskapu: teljes Xcode kell.)*

- [ ] **Step 7: (Xcode után) futtasd és szemrevételezd az appot**

Run: `cd apps && xcodebuild -scheme FootballApp build` (majd indítsd az appot), a `footballd` futása mellett.
Expected: „Ma" ablak; a 2026-11-09 (fixture) napra az Újpest–Győr meccs „ellenőrzött" badge-dzsel; dátumváltásra újratöltés; daemon nélkül a magyar hibaállapot + „Újra". *(Fáziskapu: teljes Xcode + futó footballd.)*

- [ ] **Step 8: Commit (csak kifejezett felhasználói kérésre)**

```bash
git add apps/project.yml apps/FootballApp apps/FootballAppTests
git commit -m "feat: add SwiftUI Ma screen (daily list)"
```

---

## Önellenőrzés (a terv elkészítése után)

- **Spec-lefedettség:** Cél/hatókör → Task 4 (DayView/MatchRowView/badge/állapotok); DTO-k → Task 2; runtime.json → Task 1; API hívás + hibák → Task 3; hibakezelés tábla → `APIError` (Task 1) + `ErrorStateView`/`EmptyStateView` (Task 4); tesztelés → Task 1–3 (Core, most) + Task 4 (app, Xcode után); projekt-generálás → Task 4; biztonság → konstansok + `HTTPDayService` (csak 127.0.0.1).
- **Fáziskapu:** Task 1–3 Xcode nélkül zárható (`swift test`); Task 4 Xcode + XcodeGen után.
- **Placeholder-mentes:** minden lépés konkrét kódot/parancsot ad.
- **Típus-konzisztencia:** `DayMatch`, `DayResponse`, `Blocker`, `PublishState`, `DayService.day(for:)`, `RuntimeInfoLoading.load()`, `DayModel.State` egységesen használt a taskok között.

---

## Megvalósítási jegyzet (Task 1–3 — tényleges eltérések)

A Core (Task 1–3) implementálásakor a Command Line Tools környezet miatt a következő, jóváhagyott eltérések történtek (a fenti kódblokkok a szándékolt XCTest-változatot mutatják; a tényleges fájlok ettől ezekben térnek el):

- **Teszt-keretrendszer:** a Command Line Tools alatt **nincs XCTest** (az csak teljes Xcode-dal jön), ezért a Core tesztek **Swift Testinget** használnak (`@Suite`/`@Test`/`#expect`, `XCTFail` → `Issue.record`). A tesztek száma és asszertjei azonosak (3/3/3 = 9).
- **`Package.swift`:** a teszt-target `-plugin-path` kapcsolót kap a Swift Testing makró pluginhez (a CLT-ben `host/plugins/testing` alatt van); ez **csak a teszt-targetet** érinti, a `FootballAppCore` library terméket nem.
- **`HTTPDayServiceTests`:** `@Suite(.serialized)` (a közös stub-handler miatt); a kérés ellenőrzése egy `CapturedRequest` szálbiztos (NSLock) dobozon keresztül a **teszt-taskon** fut az `await` után; minden teszt `defer { StubURLProtocol.handler = nil }`-t kap.
- **Task 4 app tesztek:** teljes Xcode alatt az XCTest elérhető marad (a terv szerinti), vagy Swift Testingre válthatók a konzisztencia kedvéért — ezt a Task 4 végrehajtásakor kell eldönteni.
