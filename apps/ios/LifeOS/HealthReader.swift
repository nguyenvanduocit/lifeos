import Foundation
import HealthKit

/// One day's value of one measure, as `POST /api/health/apple` takes it (lifeos-core's `health::Reading`).
struct Reading: Codable, Hashable {
    var day: String
    var measure: Measure
    var value: Double
}

enum Measure: String, Codable, CaseIterable {
    case steps
    case distanceKm = "distance_km"
    case sleepHours = "sleep_hours"
    case weightKg = "weight_kg"
    case restingHeartRate = "resting_heart_rate"
    case heartRateVariability = "heart_rate_variability"
    case activeEnergy = "active_energy"
    case exerciseMinutes = "exercise_minutes"

    var label: String {
        switch self {
        case .steps: "Bước chân"
        case .distanceKm: "Quãng đường"
        case .sleepHours: "Giấc ngủ"
        case .weightKg: "Cân nặng"
        case .restingHeartRate: "Nhịp tim nghỉ"
        case .heartRateVariability: "HRV"
        case .activeEnergy: "Năng lượng hoạt động"
        case .exerciseMinutes: "Phút tập"
        }
    }

    var unit: String {
        switch self {
        case .steps: "bước"
        case .distanceKm: "km"
        case .sleepHours: "giờ"
        case .weightKg: "kg"
        case .restingHeartRate: "bpm"
        case .heartRateVariability: "ms"
        case .activeEnergy: "kcal"
        case .exerciseMinutes: "phút"
        }
    }
}

/// Reads Apple Health into one value per measure per day.
struct HealthReader {
    let store: HKHealthStore

    /// The quantity measures: their type, how a day is summed up, the unit, and the decimals kept.
    private static let quantities: [(Measure, HKQuantityTypeIdentifier, HKStatisticsOptions, HKUnit, Double)] = [
        (.steps, .stepCount, .cumulativeSum, .count(), 1),
        (.distanceKm, .distanceWalkingRunning, .cumulativeSum, .meterUnit(with: .kilo), 100),
        (.activeEnergy, .activeEnergyBurned, .cumulativeSum, .kilocalorie(), 1),
        (.exerciseMinutes, .appleExerciseTime, .cumulativeSum, .minute(), 1),
        (.restingHeartRate, .restingHeartRate, .discreteAverage, .count().unitDivided(by: .minute()), 10),
        (.heartRateVariability, .heartRateVariabilitySDNN, .discreteAverage, .secondUnit(with: .milli), 10),
        (.weightKg, .bodyMass, .mostRecent, .gramUnit(with: .kilo), 10),
    ]

    static let sleep = HKCategoryType(.sleepAnalysis)

    /// Every type the app asks to read; the app writes nothing to Health.
    static var readTypes: Set<HKObjectType> {
        Set(quantities.map { HKQuantityType($0.1) as HKObjectType } + [sleep])
    }

    /// Types whose new samples wake the app to sync.
    static var observedTypes: [HKSampleType] {
        [HKQuantityType(.stepCount), HKQuantityType(.bodyMass), HKQuantityType(.restingHeartRate), sleep]
    }

    /// Readings for each day from `from` (a day start) through today.
    func readings(from: Date, calendar: Calendar = .current) async throws -> [Reading] {
        let now = Date()
        var all: [Reading] = []
        for (measure, id, options, unit, scale) in Self.quantities {
            let predicate = HKQuery.predicateForSamples(withStart: from, end: now)
            let query = HKStatisticsCollectionQueryDescriptor(
                predicate: .quantitySample(type: HKQuantityType(id), predicate: predicate),
                options: options,
                anchorDate: calendar.startOfDay(for: from),
                intervalComponents: DateComponents(day: 1)
            )
            let collection = try await query.result(for: store)
            collection.enumerateStatistics(from: from, to: now) { stats, _ in
                let quantity: HKQuantity? = switch options {
                case .cumulativeSum: stats.sumQuantity()
                case .discreteAverage: stats.averageQuantity()
                default: stats.mostRecentQuantity()
                }
                guard let quantity else { return }
                let value = (quantity.doubleValue(for: unit) * scale).rounded() / scale
                all.append(Reading(day: Self.day(stats.startDate, calendar), measure: measure, value: value))
            }
        }
        all += try await sleepReadings(from: from, to: now, calendar: calendar)
        return all
    }

    /// Hours asleep for each night, counted on the day it ended. The phone and the watch both record sleep, so
    /// overlapping stretches are joined before they are added up.
    private func sleepReadings(from: Date, to: Date, calendar: Calendar) async throws -> [Reading] {
        // A night that ended on `from` began the evening before.
        let start = calendar.date(byAdding: .hour, value: -18, to: from) ?? from
        let query = HKSampleQueryDescriptor(
            predicates: [.categorySample(type: Self.sleep, predicate: HKQuery.predicateForSamples(withStart: start, end: to))],
            sortDescriptors: [SortDescriptor(\.startDate)]
        )
        let asleep: Set<Int> = [
            HKCategoryValueSleepAnalysis.asleepUnspecified.rawValue,
            HKCategoryValueSleepAnalysis.asleepCore.rawValue,
            HKCategoryValueSleepAnalysis.asleepDeep.rawValue,
            HKCategoryValueSleepAnalysis.asleepREM.rawValue,
        ]
        let stretches = try await query.result(for: store)
            .filter { asleep.contains($0.value) }
            .map { ($0.startDate, $0.endDate) }
        var joined: [(Date, Date)] = []
        for (s, e) in stretches {
            if let last = joined.last, s <= last.1 {
                joined[joined.count - 1].1 = max(last.1, e)
            } else {
                joined.append((s, e))
            }
        }
        var hours: [String: Double] = [:]
        for (s, e) in joined where e >= from {
            hours[Self.day(e, calendar), default: 0] += e.timeIntervalSince(s) / 3600
        }
        return hours.map { Reading(day: $0.key, measure: .sleepHours, value: ($0.value * 100).rounded() / 100) }
    }

    static func day(_ date: Date, _ calendar: Calendar) -> String {
        let c = calendar.dateComponents([.year, .month, .day], from: date)
        return String(format: "%04d-%02d-%02d", c.year!, c.month!, c.day!)
    }
}
