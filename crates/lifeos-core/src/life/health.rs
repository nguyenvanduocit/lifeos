//! Health: numbers measured over time in `metrics` (weight, sleep, steps,
//! mood, blood pressure…), one name per thing measured.

use std::collections::BTreeMap;

use anyhow::{Result, bail};
use chrono::{Duration, NaiveDate};
use rusqlite::params;

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
            "INSERT INTO metrics(day, name, value, unit, note) VALUES (?1, ?2, ?3, ?4, ?5)",
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
}
