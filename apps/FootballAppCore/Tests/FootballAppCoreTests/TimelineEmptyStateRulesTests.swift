import Foundation
import Testing
@testable import FootballAppCore

@Suite struct TimelineEmptyStateRulesTests {
    private func incident() throws -> MatchIncident {
        let json = """
        {"minute":34,"incident_type":"goal","side":"home","player_name":"Varga Barnabás"}
        """.data(using: .utf8)!
        return try JSONDecoder().decode(MatchIncident.self, from: json)
    }

    @Test func eventExpectedStatusesShowEmptyStateWhenIncidentsAreMissing() {
        for status in ["live", "halftime", "finished", "awarded", "abandoned", "penalties"] {
            #expect(
                TimelineEmptyStateRules.shouldShowTimelineEmptyState(
                    status: status, incidents: nil
                ),
                "hiányzó incidents esetén várható esemény: \(status)"
            )
            #expect(
                TimelineEmptyStateRules.shouldShowTimelineEmptyState(
                    status: status, incidents: []
                ),
                "üres incidents esetén várható esemény: \(status)"
            )
        }
    }

    @Test func statusWithoutExpectedIncidentsShowsNothing() {
        for status in ["scheduled", "confirmed", "postponed", "canceled", "valami_uj"] {
            #expect(
                !TimelineEmptyStateRules.shouldShowTimelineEmptyState(
                    status: status, incidents: nil
                ),
                "nem esemény-váró státusz: \(status)"
            )
        }
    }

    @Test func existingIncidentsNeverShowEmptyState() throws {
        let incidents = try [incident()]
        for status in ["live", "halftime", "finished", "awarded", "abandoned", "penalties"] {
            #expect(
                !TimelineEmptyStateRules.shouldShowTimelineEmptyState(
                    status: status, incidents: incidents
                ),
                "meglévő eseménynél nincs üres állapot: \(status)"
            )
        }
    }
}