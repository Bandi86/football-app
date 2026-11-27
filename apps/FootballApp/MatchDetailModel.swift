import Foundation
import Observation
import FootballAppCore

@MainActor
@Observable
final class MatchDetailModel {
    enum State: Equatable {
        case idle
        case loading
        case loaded(MatchDetail)
        case error(APIError)
    }

    var state: State = .idle
    /// A három mikro-végpont adata; a részletező önállóan tölti, hogy a fő
    /// meccsadat ne várjon rá. Hiány = üres listák.
    var micro: MatchMicro?
    /// A részletes odds-piacok; a fő részletezőtől függetlenül töltődnek be.
    var markets: MarketsResponseDTO?
    /// Shadow predikció (nem ellenőrzött, nem fogadási tanács).
    var prediction: ShadowPrediction?
    /// Együttműködési adatok: jegyzetek és mező-szintű előzmények.
    var notes: [MatchNoteDTO] = []
    var history: [HistoryEntryDTO] = []
    let logos = LogoStore()
    /// A kézi bevitel kimenetele — az űrlap ezt jeleníti meg.
    var submitError: APIError?
    var isSubmitting = false

    private let loader: any RuntimeInfoLoading
    private let serviceFactory: (RuntimeInfo) throws -> MatchDetailService
    private let microServiceFactory: (RuntimeInfo) throws -> MatchMicroService
    private let marketServiceFactory: (RuntimeInfo) throws -> MatchMarketService
    private let predictionServiceFactory: (RuntimeInfo) throws -> MatchPredictionService
    private let manualServiceFactory: (RuntimeInfo) throws -> ManualEntryService
    private let opsServiceFactory: (RuntimeInfo) throws -> OpsService
    private var generation = 0
    private var microGeneration = 0

    init(
        loader: any RuntimeInfoLoading = RuntimeInfoLoader(),
        serviceFactory: ((RuntimeInfo) throws -> MatchDetailService)? = nil,
        microServiceFactory: ((RuntimeInfo) throws -> MatchMicroService)? = nil,
        marketServiceFactory: ((RuntimeInfo) throws -> MatchMarketService)? = nil,
        predictionServiceFactory: ((RuntimeInfo) throws -> MatchPredictionService)? = nil,
        manualServiceFactory: ((RuntimeInfo) throws -> ManualEntryService)? = nil,
        opsServiceFactory: ((RuntimeInfo) throws -> OpsService)? = nil
    ) {
        self.loader = loader
        self.serviceFactory = serviceFactory ?? Self.makeService
        self.microServiceFactory = microServiceFactory ?? Self.makeMicroService
        self.marketServiceFactory = marketServiceFactory ?? Self.makeMarketService
        self.predictionServiceFactory = predictionServiceFactory ?? Self.makePredictionService
        self.manualServiceFactory = manualServiceFactory ?? Self.makeManualService
        self.opsServiceFactory = opsServiceFactory ?? Self.makeOpsService
    }

    func load(matchId: UUID) async {
        generation &+= 1
        let currentGeneration = generation
        state = .loading
        do {
            let info = try loader.load()
            if let baseURL = URL(string: "http://127.0.0.1:\(info.port)") {
                logos.configure(baseURL: baseURL, token: info.readToken)
            }
            let service = try serviceFactory(info)
            let detail = try await service.detail(for: matchId)
            guard currentGeneration == generation else { return }
            state = .loaded(detail)
            // A mikro-adatok külön végpontokról jönnek; itt töltjük be, hogy
            // a fő részletező már megjelenjen, de a szekciók is frissüljenek.
            await loadMicro(matchId: matchId)
            await loadMarkets(matchId: matchId)
            await loadPrediction(matchId: matchId)
            await loadCollaboration(matchId: matchId)
        } catch is CancellationError {
            return
        } catch let error as APIError {
            guard currentGeneration == generation else { return }
            state = .error(error)
        } catch {
            guard currentGeneration == generation else { return }
            state = .error(.network)
        }
    }

    /// A mikro-adatok önálló betöltése; a hiba nem dönti el a fő részletezőt
    /// (a hiány szakaszszinten jelenik meg).
    func loadMicro(matchId: UUID) async {
        microGeneration &+= 1
        let currentGeneration = microGeneration
        do {
            let info = try loader.load()
            let service = try microServiceFactory(info)
            let micro = try await service.load(matchId: matchId)
            guard currentGeneration == microGeneration else { return }
            self.micro = micro
        } catch {
            guard currentGeneration == microGeneration else { return }
            self.micro = MatchMicro(lineups: [], playerStats: [], shots: [])
        }
    }

    /// Részletes odds-piacok; a hiány üres lista, a hiba nem dönti el a fő
    /// részletezőt (az Odds fül üres állapotot mutat).
    func loadMarkets(matchId: UUID) async {
        do {
            let info = try loader.load()
            let service = try marketServiceFactory(info)
            self.markets = try await service.load(matchId: matchId)
        } catch {
            self.markets = MarketsResponseDTO(quotes: [], count: 0)
        }
    }

    /// Jegyzetek és előzmények; a hiány üres lista, a hiba nem dönti el a
    /// fő részletezőt (a panel üres állapotot mutat).
    /// Shadow predikció; hiány vagy hiba esetén `nil`, a fő részletező marad.
    func loadPrediction(matchId: UUID) async {
        do {
            let info = try loader.load()
            let service = try predictionServiceFactory(info)
            let response = try await service.load(matchId: matchId)
            self.prediction = ShadowPrediction(dto: response)
        } catch {
            self.prediction = nil
        }
    }

    func loadCollaboration(matchId: UUID) async {
        do {
            let info = try loader.load()
            let service = try opsServiceFactory(info)
            async let notes = service.notes(matchID: matchId)
            async let history = service.history(matchID: matchId, limit: 50)
            let loadedNotes = try await notes
            let loadedHistory = try await history
            self.notes = loadedNotes.notes
            self.history = loadedHistory.entries
        } catch {
            self.notes = []
            self.history = []
        }
    }

    /// Új jegyzet írása ops tokennel, majd a panel frissítése.
    func addNote(matchId: UUID, body: String, kind: String) async throws {
        let info = try loader.load()
        let service = try opsServiceFactory(info)
        _ = try await service.addNote(matchID: matchId, author: "ui", body: body, kind: kind)
        await loadCollaboration(matchId: matchId)
    }

    /// A betöltött meccs részletei, ha éppen van ilyen.
    var detail: MatchDetail? {
        if case .loaded(let detail) = state { return detail }
        return nil
    }

    /// Kézi eredmény felvétele, majd a részletező frissítése a kanonikus adatból.
    func submitResult(matchId: UUID, homeFt: Int, awayFt: Int, note: String) async {
        guard homeFt >= 0, awayFt >= 0, !note.trimmingCharacters(in: .whitespacesAndNewlines).isEmpty else {
            submitError = .badRequest
            return
        }
        isSubmitting = true
        submitError = nil
        defer { isSubmitting = false }
        do {
            let info = try loader.load()
            let service = try manualServiceFactory(info)
            _ = try await service.submitResult(
                matchId: matchId,
                request: ManualResultRequest(homeFt: homeFt, awayFt: awayFt, note: note)
            )
            await load(matchId: matchId)
        } catch is CancellationError {
            return
        } catch let error as APIError {
            submitError = error
        } catch {
            submitError = .network
        }
    }

    /// Teljes meccsszerkesztés mentése (kezdés, státusz, eredmény, indoklás),
    /// majd a részletező frissítése. A szerverüzenet a `submitError`-on át
    /// kerül az űrlapra.
    func submitEdit(matchId: UUID, request: ManualEditRequest) async {
        guard !request.note.trimmingCharacters(in: .whitespacesAndNewlines).isEmpty else {
            submitError = .badRequest
            return
        }
        isSubmitting = true
        submitError = nil
        defer { isSubmitting = false }
        do {
            let info = try loader.load()
            let service = try manualServiceFactory(info)
            _ = try await service.submitEdit(matchId: matchId, request: request)
            await load(matchId: matchId)
        } catch is CancellationError {
            return
        } catch let error as APIError {
            submitError = error
        } catch {
            submitError = .network
        }
    }

    private static func makeService(_ info: RuntimeInfo) throws -> MatchDetailService {
        guard let baseURL = URL(string: "http://127.0.0.1:\(info.port)") else {
            throw APIError.runtimeMissing
        }
        return HTTPMatchDetailService(baseURL: baseURL, readToken: info.readToken)
    }

    private static func makeMicroService(_ info: RuntimeInfo) throws -> MatchMicroService {
        guard let baseURL = URL(string: "http://127.0.0.1:\(info.port)") else {
            throw APIError.runtimeMissing
        }
        return HTTPMatchMicroService(baseURL: baseURL, readToken: info.readToken)
    }

    private static func makeMarketService(_ info: RuntimeInfo) throws -> MatchMarketService {
        guard let baseURL = URL(string: "http://127.0.0.1:\(info.port)") else {
            throw APIError.runtimeMissing
        }
        return HTTPMatchMarketService(baseURL: baseURL, readToken: info.readToken)
    }

    private static func makePredictionService(_ info: RuntimeInfo) throws -> MatchPredictionService {
        guard let baseURL = URL(string: "http://127.0.0.1:\(info.port)") else {
            throw APIError.runtimeMissing
        }
        return HTTPMatchPredictionService(baseURL: baseURL, readToken: info.readToken)
    }

    private static func makeOpsService(_ info: RuntimeInfo) throws -> OpsService {
        guard let baseURL = URL(string: "http://127.0.0.1:\(info.port)") else {
            throw APIError.runtimeMissing
        }
        guard let opsToken = info.opsToken, !opsToken.isEmpty else {
            throw APIError.unauthorized
        }
        return HTTPOpsService(baseURL: baseURL, opsToken: opsToken)
    }

    private static func makeManualService(_ info: RuntimeInfo) throws -> ManualEntryService {
        guard let baseURL = URL(string: "http://127.0.0.1:\(info.port)") else {
            throw APIError.runtimeMissing
        }
        guard let opsToken = info.opsToken, !opsToken.isEmpty else {
            throw APIError.unauthorized
        }
        return HTTPManualEntryService(baseURL: baseURL, opsToken: opsToken)
    }
}
