import BackgroundTasks
import Foundation
import HealthKit
import Observation
import UIKit

/// Sends Apple Health to LifeOS: when the app opens, when the owner asks, when HealthKit wakes it with new samples,
/// and on a periodic background refresh that catches up wakes iOS dropped. Each sync sends again every day since
/// the day before the last one the server kept, and at least the last week (today's steps grow all day, a night's
/// sleep and the watch's samples arrive late); the server keeps one value per day, so a day sent twice is replaced.
/// `lastSync` moves only when the server answered 200, so a sync that failed (phone off the tailnet, server down)
/// is covered by the next one.
///
/// HealthKit cannot be read while the phone is locked (its store is file-protected), and background wakes mostly
/// come then: those wakes do nothing and wait for the next unlocked one, as PulsHealth found.
@MainActor @Observable
final class HealthSync {
    static let shared = HealthSync()

    enum Reason { case opened, asked, background, history }

    /// How far back the first sync reaches, and the least any later one does.
    nonisolated static let firstDays = 365
    nonisolated static let recentDays = 7
    /// The periodic catch-up; listed in Info.plist's `BGTaskSchedulerPermittedIdentifiers`.
    nonisolated static let refreshTask = "dev.aiocean.lifeos.health-refresh"

    private(set) var running = false
    private(set) var lastSync: Date? = Settings.lastSync
    private(set) var error: String?
    /// Today's values, newest sync.
    private(set) var today: [Reading] = []

    let store = HKHealthStore()
    private var observing = false

    var available: Bool { HKHealthStore.isHealthDataAvailable() }

    func authorize() async throws {
        try await store.requestAuthorization(toShare: [], read: HealthReader.readTypes)
    }

    func run(reason: Reason) async {
        guard available, !running else { return }
        guard UIApplication.shared.isProtectedDataAvailable else { return }
        running = true
        defer { running = false }
        do {
            if reason != .background { try await authorize() }
            let calendar = Calendar.current
            let from = Self.from(lastSync: reason == .history ? nil : lastSync, calendar: calendar)
            let readings = try await HealthReader(store: store).readings(from: from, calendar: calendar)
            try await Uploader(server: Settings.server).send(readings)
            let now = Date()
            Settings.lastSync = now
            lastSync = now
            error = nil
            let day = HealthReader.day(now, calendar)
            today = readings.filter { $0.day == day }.sorted { $0.measure.label < $1.measure.label }
        } catch {
            self.error = error.localizedDescription
        }
    }

    /// The first day a sync sends: the day before the last kept sync, at most `firstDays` and at least
    /// `recentDays` back.
    nonisolated static func from(lastSync: Date?, now: Date = Date(), calendar: Calendar) -> Date {
        let today = calendar.startOfDay(for: now)
        let oldest = calendar.date(byAdding: .day, value: 1 - firstDays, to: today)!
        let recent = calendar.date(byAdding: .day, value: 1 - recentDays, to: today)!
        guard let lastSync else { return oldest }
        let before = calendar.date(byAdding: .day, value: -1, to: calendar.startOfDay(for: lastSync))!
        return max(oldest, min(before, recent))
    }

    /// Wakes for new samples and for the periodic catch-up. Call once, before launch finishes.
    nonisolated func observe() {
        guard HKHealthStore.isHealthDataAvailable() else { return }
        _ = BGTaskScheduler.shared.register(forTaskWithIdentifier: Self.refreshTask, using: nil) { task in
            Self.scheduleRefresh()
            let work = Task { @MainActor in
                await HealthSync.shared.run(reason: .background)
                task.setTaskCompleted(success: true)
            }
            task.expirationHandler = { work.cancel() }
        }
        Self.scheduleRefresh()
        let store = HKHealthStore()
        for type in HealthReader.observedTypes {
            let query = HKObserverQuery(sampleType: type, predicate: nil) { _, done, error in
                guard error == nil else { return done() }
                Task { @MainActor in
                    // A wake gets a few seconds; ask for the half minute iOS grants, and tell HealthKit the wake is
                    // handled either way, or it backs off delivering to the app.
                    var assertion = UIBackgroundTaskIdentifier.invalid
                    assertion = UIApplication.shared.beginBackgroundTask(withName: "health-sync") {
                        UIApplication.shared.endBackgroundTask(assertion)
                        assertion = .invalid
                    }
                    await HealthSync.shared.run(reason: .background)
                    done()
                    if assertion != .invalid { UIApplication.shared.endBackgroundTask(assertion) }
                }
            }
            store.execute(query)
            store.enableBackgroundDelivery(for: type, frequency: .hourly) { _, _ in }
        }
    }

    nonisolated static func scheduleRefresh() {
        let request = BGAppRefreshTaskRequest(identifier: refreshTask)
        request.earliestBeginDate = Date(timeIntervalSinceNow: 3 * 3600)
        try? BGTaskScheduler.shared.submit(request)
    }
}

struct Uploader {
    let server: URL

    struct Failure: LocalizedError {
        let message: String
        var errorDescription: String? { message }
    }

    func send(_ readings: [Reading]) async throws {
        var request = URLRequest(url: server.appending(path: "api/health/apple"))
        request.httpMethod = "POST"
        request.setValue("application/json", forHTTPHeaderField: "Content-Type")
        // The server takes owner requests only with this header, which a web page elsewhere cannot send unasked.
        request.setValue("1", forHTTPHeaderField: "x-lifeos-account")
        request.timeoutInterval = 30
        request.httpBody = try JSONEncoder().encode(["readings": readings])
        let (body, response) = try await URLSession.shared.data(for: request)
        guard let http = response as? HTTPURLResponse else { throw Failure(message: "LifeOS không trả lời") }
        guard http.statusCode == 200 else {
            let detail = (try? JSONDecoder().decode([String: String].self, from: body))?["error"]
            throw Failure(message: "LifeOS trả \(http.statusCode)" + (detail.map { ": \($0)" } ?? ""))
        }
    }
}
