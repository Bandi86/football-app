import Foundation
import Testing
@testable import FootballAppCore

@Suite struct MatchPredictionModelsTests {
    private let json = """
    {"rows":[
      {"model_version":"dixon-coles-eb-v1","market":"1x2","line":0,"outcome":"home","probability":0.437,"fair_odds":2.29,"edge":null,"quality":"ok","inputs_hash":"abc","as_of":"2026-11-27T10:00:00Z"},
      {"model_version":"dixon-coles-eb-v1","market":"1x2","line":0,"outcome":"draw","probability":0.252,"fair_odds":3.97,"edge":null,"quality":"ok","inputs_hash":"abc","as_of":"2026-11-27T10:00:00Z"},
      {"model_version":"dixon-coles-eb-v1","market":"1x2","line":0,"outcome":"away","probability":0.311,"fair_odds":3.22,"edge":null,"quality":"ok","inputs_hash":"abc","as_of":"2026-11-27T10:00:00Z"}
    ],"count":3}
    """

    @Test func decodesThreeOutcomesAndFormatsHungarianProbabilities() throws {
        let dto = try APIDecoding.makeDecoder().decode(
            PredictionResponseDTO.self, from: Data(json.utf8)
        )
        let prediction = try #require(ShadowPrediction(dto: dto))
        #expect(prediction.modelVersion == "dixon-coles-eb-v1")
        #expect(prediction.home == 0.437 && prediction.draw == 0.252 && prediction.away == 0.311)
        #expect(prediction.fairHome == 2.29)
        #expect(prediction.probabilityText.contains("44%"))
    }

    @Test func incompletePredictionIsNilNotInvented() throws {
        let incomplete = """
        {"rows":[{"model_version":"v1","market":"1x2","line":0,"outcome":"home","probability":0.5,
         "fair_odds":null,"edge":null,"quality":"ok","inputs_hash":"x",
         "as_of":"2026-11-27T10:00:00Z"}],"count":1}
        """
        let dto = try APIDecoding.makeDecoder().decode(
            PredictionResponseDTO.self, from: Data(incomplete.utf8)
        )
        #expect(ShadowPrediction(dto: dto) == nil)
        #expect(ShadowPrediction(dto: PredictionResponseDTO(rows: [], count: 0)) == nil)
    }
}
