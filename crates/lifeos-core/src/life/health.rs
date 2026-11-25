//! Health: numbers measured over time in `metrics` (weight, sleep, steps,
//! mood, blood pressure…), one name per thing measured.

use std::collections::BTreeMap;

use anyhow::{Result, bail};
use chrono::{Duration, NaiveDate};
use rusqlite::params;
use serde::Deserialize;

use super::{Life, key, today};
use lifeos_wire::{Metric, MetricPoint, MetricSeries};

input! {
    pub struct LogMetric {
        /// Stable lowercase name, e.g. "cân nặng", "giấc ngủ", "bước chân", "tâm trạng".
        pub name: String,
        pub value: f64,
        /// e.g. "kg", "giờ", "bước", "/10".
        pub unit: Option<String>,
        pub note: Option<String>,
        /// YYYY-MM-DD. Defaults to today.
        pub date: Option<NaiveDate>,
    }
}

input! {
    pub struct ListMetrics {
        /// Only this metric.
        pub name: Option<String>,
        /// YYYY-MM-DD, inclusive. Defaults to 30 days ago.
        pub from: Option<NaiveDate>,
        /// YYYY-MM-DD, inclusive. Defaults to today.
        pub to: Option<NaiveDate>,
    }
}

input! {
    pub struct SeriesInput {
        /// How many days back. Defaults to 30.
        pub days: Option<u32>,
    }
}

pub const SERIES_DAYS: u32 = 30;

/// What the iPhone app reads from Apple Health, one value per day. Each lands
/// in `metrics` under the name the owner already logs it by.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Measure {
    /// Sum for the day.
    Steps,
    /// Walking and running, km, sum for the day.
    DistanceKm,
    /// Asleep (core, deep, REM), hours, for the night that ended this day.
    SleepHours,
    /// Last weighing of the day, kg.
    WeightKg,
    /// Resting heart rate, bpm, average for the day.
    RestingHeartRate,
    /// HRV (SDNN), ms, average for the day.
    HeartRateVariability,
    /// Active energy, kcal, sum for the day.
    ActiveEnergy,
    /// Exercise minutes, sum for the day.
    ExerciseMinutes,
}

impl Measure {
    /// The metric name and unit it is kept under.
    pub fn metric(self) -> (&'static str, &'static str) {
        match self {
            Measure::Steps => ("bước chân", "bước"),
            Measure::DistanceKm => ("quãng đường", "km"),
            Measure::SleepHours => ("giấc ngủ", "giờ"),
            Measure::WeightKg => ("cân nặng", "kg"),
            Measure::RestingHeartRate => ("nhịp tim nghỉ", "bpm"),
            Measure::HeartRateVariability => ("hrv", "ms"),
            Measure::ActiveEnergy => ("năng lượng hoạt động", "kcal"),
            Measure::ExerciseMinutes => ("phút tập", "phút"),
        }
    }
}

#[derive(Debug, Clone, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Reading {
    pub day: NaiveDate,
    pub measure: Measure,
    pub value: f64,
}

/// The source Apple Health readings are kept under.
pub const APPLE_HEALTH: &str = "apple-health";

impl Life {
    pub fn log_metric(&self, m: LogMetric) -> Result<Metric> {
        let name = key(&m.name);
        if name.is_empty() {
            bail!("metric name is empty");
        }
        if !m.value.is_finite() {
            bail!("value must be a number");
        }
        let day = m.date.unwrap_or_else(today);
        self.conn.execute(
            "INSERT INTO metrics(id, day, name, value, unit, note)
                 VALUES ((SELECT MAX(COALESCE(MAX(id), 0),
                    COALESCE((SELECT highest FROM retired_row_ids WHERE table_name = 'metrics'), 0)) + 1 FROM metrics), ?1, ?2, ?3, ?4, ?5)",
            params![day.to_string(), name, m.value, m.unit, m.note],
        )?;
        Ok(Metric {
            id: self.conn.last_insert_rowid(),
            day: day.to_string(),
            name,
            value: m.value,
            unit: m.unit,
            note: m.note,
        })
    }

    /// Keeps a device's readings: one row per source, metric and day, so sending
    /// a day again (the app resends recent days on every sync) replaces it.
    /// Returns how many readings were kept.
    pub fn import_readings(&self, source: &str, readings: &[Reading]) -> Result<usize> {
        if source.trim().is_empty() {
            bail!("source is empty");
        }
        if let Some(r) = readings.iter().find(|r| !r.value.is_finite() || r.value < 0.0) {
            bail!("{:?} on {} is not a measurement: {}", r.measure, r.day, r.value);
        }
        let tx = self.conn.unchecked_transaction()?;
        {
            let mut upsert = tx.prepare(
                "INSERT INTO metrics(id, day, name, value, unit, note, source)
                 VALUES ((SELECT MAX(COALESCE(MAX(id), 0),
                    COALESCE((SELECT highest FROM retired_row_ids WHERE table_name = 'metrics'), 0)) + 1 FROM metrics), ?1, ?2, ?3, ?4, ?5, ?6)
                 ON CONFLICT(source, name, day) WHERE source IS NOT NULL
                 DO UPDATE SET value = excluded.value, unit = excluded.unit, note = excluded.note",
            )?;
            for r in readings {
                let (name, unit) = r.measure.metric();
                upsert.execute(params![r.day.to_string(), name, r.value, unit, source_note(source), source])?;
            }
        }
        tx.commit()?;
        Ok(readings.len())
    }

    pub fn delete_metric(&self, id: i64) -> Result<()> {
        if self.conn.execute("DELETE FROM metrics WHERE id = ?1", [id])? == 0 {
            bail!("no metric {id}");
        }
        Ok(())
    }

    /// Oldest first.
    pub fn metrics(&self, q: &ListMetrics) -> Result<Vec<Metric>> {
        let to = q.to.unwrap_or_else(today);
        let from = q.from.unwrap_or(to - Duration::days(SERIES_DAYS as i64));
        let mut stmt = self.conn.prepare(
            "SELECT id, day, name, value, unit, note FROM metrics
             WHERE (?1 IS NULL OR name = ?1) AND day BETWEEN ?2 AND ?3 ORDER BY day, id",
        )?;
        let name = q.name.as_deref().map(key);
        let rows = stmt.query_map(params![name, from.to_string(), to.to_string()], |r| {
            Ok(Metric { id: r.get(0)?, day: r.get(1)?, name: r.get(2)?, value: r.get(3)?, unit: r.get(4)?, note: r.get(5)? })
        })?;
        Ok(rows.collect::<rusqlite::Result<_>>()?)
    }

    /// One series per metric measured in the last `days` days, the most
    /// recently measured first. A day measured more than once keeps its last value.
    pub fn metric_series(&self, days: u32, on: NaiveDate) -> Result<Vec<MetricSeries>> {
        let all = self.metrics(&ListMetrics {
            from: Some(on - Duration::days(days.saturating_sub(1) as i64)),
            to: Some(on),
            ..Default::default()
        })?;
        let mut by_name: BTreeMap<String, (Option<String>, BTreeMap<String, f64>)> = BTreeMap::new();
        for m in all {
            let entry = by_name.entry(m.name).or_default();
            if m.unit.is_some() {
                entry.0 = m.unit;
            }
            entry.1.insert(m.day, m.value);
        }
        let mut series: Vec<MetricSeries> = by_name
            .into_iter()
            .map(|(name, (unit, days))| {
                let points: Vec<MetricPoint> =
                    days.into_iter().map(|(day, value)| MetricPoint { day, value }).collect();
                let latest = points.last().map(|p| p.value);
                let previous = points.len().checked_sub(2).map(|i| points[i].value);
                MetricSeries { name, unit, points, latest, previous }
            })
            .collect();
        series.sort_by(|a, b| {
            let last = |s: &MetricSeries| s.points.last().map(|p| p.day.clone());
            last(b).cmp(&last(a)).then(a.name.cmp(&b.name))
        });
        Ok(series)
    }
}

/// What an agent reading the metric sees of where it came from.
fn source_note(source: &str) -> &str {
    if source == APPLE_HEALTH { "Apple Health" } else { source }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::life::test_support::*;

    #[test]
    fn series_keep_one_point_per_day() {
        let (_dir, life) = life();
        let log = |name: &str, value: f64, day: &str| {
            life.log_metric(LogMetric {
                name: name.into(),
                value,
                unit: (name == "Cân nặng").then(|| "kg".into()),
                date: Some(d(day)),
                ..Default::default()
            })
            .unwrap()
        };
        log("Cân nặng", 70.0, "2026-11-20");
        log("cân nặng", 69.5, "2026-11-24");
        log("cân nặng", 69.2, "2026-11-24");
        log("giấc ngủ", 7.0, "2026-11-25");
        log("cân nặng", 80.0, "2026-09-01"); // outside the window
        let s = life.metric_series(30, d("2026-11-25")).unwrap();
        assert_eq!(s.len(), 2);
        assert_eq!(s[0].name, "giấc ngủ");
        let w = &s[1];
        assert_eq!(w.unit.as_deref(), Some("kg"));
        assert_eq!(w.points.len(), 2);
        assert_eq!((w.latest, w.previous), (Some(69.2), Some(70.0)));
        assert!(life.log_metric(LogMetric { name: " ".into(), ..Default::default() }).is_err());
    }

    #[test]
    fn readings_replace_their_own_day() {
        let (_dir, life) = life();
        let read = |measure, value, day: &str| Reading { day: d(day), measure, value };
        life.log_metric(LogMetric { name: "cân nặng".into(), value: 70.0, date: Some(d("2026-11-24")), ..Default::default() })
            .unwrap();
        life.import_readings(
            APPLE_HEALTH,
            &[read(Measure::Steps, 4000.0, "2026-11-25"), read(Measure::WeightKg, 69.8, "2026-11-24")],
        )
        .unwrap();
        // The day grew: the app sends it again.
        life.import_readings(APPLE_HEALTH, &[read(Measure::Steps, 9000.0, "2026-11-25")]).unwrap();
        let all = life
            .metrics(&ListMetrics { from: Some(d("2026-11-20")), to: Some(d("2026-11-25")), ..Default::default() })
            .unwrap();
        let steps: Vec<_> = all.iter().filter(|m| m.name == "bước chân").collect();
        assert_eq!(steps.len(), 1);
        assert_eq!((steps[0].value, steps[0].unit.as_deref()), (9000.0, Some("bước")));
        // A weight logged by hand stays beside the one the phone sent.
        assert_eq!(all.iter().filter(|m| m.name == "cân nặng").count(), 2);
        assert!(life.import_readings(APPLE_HEALTH, &[read(Measure::Steps, f64::NAN, "2026-11-25")]).is_err());
    }
}
