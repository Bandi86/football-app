import Foundation

public protocol DayService: Sendable {
    func day(for date: Date) async throws -> DayFeed
}
