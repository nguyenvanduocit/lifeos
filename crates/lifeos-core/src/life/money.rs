//! Money: `transactions` (signed, in each currency's minor unit) and monthly
//! `budgets` per category.

use std::collections::BTreeMap;

use anyhow::{Result, bail};
use chrono::{Datelike, Duration, NaiveDate};
use rusqlite::params;

use super::{Life, key, today};
use lifeos_wire::{Budget, BudgetLine, CategoryAmount, CurrencySummary, DayAmount, MoneyMonth, Transaction};

input! {
    pub struct AddTransaction {
        /// Signed: negative for money out (spending), positive for money in.
        pub amount: f64,
        /// ISO code, e.g. VND, USD.
        pub currency: String,
        /// Short lowercase category, e.g. "ăn uống", "đi lại", "lương".
        pub category: String,
        pub note: Option<String>,
        /// YYYY-MM-DD. Defaults to today.
        pub date: Option<NaiveDate>,
    }
}

input! {
    pub struct DateRange {
        /// YYYY-MM-DD, inclusive. Defaults to the first day of this month.
        pub from: Option<NaiveDate>,
        /// YYYY-MM-DD, inclusive. Defaults to today.
        pub to: Option<NaiveDate>,
    }
}

input! {
    pub struct SetBudget {
        pub category: String,
        pub currency: String,
        /// Per month, positive. 0 removes the budget.
        pub monthly: f64,
    }
}

input! {
    pub struct MonthInput {
        /// YYYY-MM. Defaults to this month.
        pub month: Option<String>,
    }
}

impl DateRange {
    pub fn resolve(&self) -> (NaiveDate, NaiveDate) {
        let to = self.to.unwrap_or_else(today);
        (self.from.unwrap_or_else(|| month_start(to)), to)
    }
}

pub fn month_start(d: NaiveDate) -> NaiveDate {
    d.with_day(1).expect("day 1 exists")
}

fn month_end(start: NaiveDate) -> NaiveDate {
    let next = if start.month() == 12 {
        NaiveDate::from_ymd_opt(start.year() + 1, 1, 1)
    } else {
        NaiveDate::from_ymd_opt(start.year(), start.month() + 1, 1)
    };
    next.expect("valid month") - Duration::days(1)
}

/// Digits after the decimal point for a currency's minor unit.
fn minor_exponent(currency: &str) -> i32 {
    match currency {
        "VND" | "JPY" | "KRW" | "IDR" => 0,
        _ => 2,
    }
}

fn to_minor(amount: f64, currency: &str) -> i64 {
    (amount * 10f64.powi(minor_exponent(currency))).round() as i64
}

fn from_minor(minor: i64, currency: &str) -> f64 {
    minor as f64 / 10f64.powi(minor_exponent(currency))
}

fn currency_code(c: &str) -> Result<String> {
    let c = c.trim().to_uppercase();
    if c.len() != 3 || !c.chars().all(|ch| ch.is_ascii_alphabetic()) {
        bail!("currency must be a 3-letter ISO code such as VND or USD");
    }
    Ok(c)
}

impl Life {
    pub fn add_transaction(&self, t: AddTransaction) -> Result<Transaction> {
        let currency = currency_code(&t.currency)?;
        let category = key(&t.category);
        let day = t.date.unwrap_or_else(today);
        let minor = to_minor(t.amount, &currency);
        self.conn.execute(
            "INSERT INTO transactions(id, day, amount_minor, currency, category, note)
                 VALUES ((SELECT MAX(COALESCE(MAX(id), 0),
                    COALESCE((SELECT highest FROM retired_row_ids WHERE table_name = 'transactions'), 0)) + 1 FROM transactions), ?1, ?2, ?3, ?4, ?5)",
            params![day.to_string(), minor, currency, category, t.note],
        )?;
        Ok(Transaction {
            id: self.conn.last_insert_rowid(),
            day: day.to_string(),
            amount: from_minor(minor, &currency),
            currency,
            category,
            note: t.note,
        })
    }

    pub fn delete_transaction(&self, id: i64) -> Result<()> {
        if self.conn.execute("DELETE FROM transactions WHERE id = ?1", [id])? == 0 {
            bail!("no transaction {id}");
        }
        Ok(())
    }

    /// Oldest first.
    pub fn transactions(&self, from: NaiveDate, to: NaiveDate) -> Result<Vec<Transaction>> {
        let mut stmt = self.conn.prepare(
            "SELECT id, day, amount_minor, currency, category, note FROM transactions
             WHERE day BETWEEN ?1 AND ?2 ORDER BY day, id",
        )?;
        let rows = stmt.query_map(params![from.to_string(), to.to_string()], |r| {
            let currency: String = r.get(3)?;
            Ok(Transaction {
                id: r.get(0)?,
                day: r.get(1)?,
                amount: from_minor(r.get(2)?, &currency),
                currency,
                category: r.get(4)?,
                note: r.get(5)?,
            })
        })?;
        Ok(rows.collect::<rusqlite::Result<_>>()?)
    }

    pub fn finance_summary(&self, from: NaiveDate, to: NaiveDate) -> Result<Vec<CurrencySummary>> {
        let mut by_currency: BTreeMap<String, (i64, i64, BTreeMap<String, i64>)> = BTreeMap::new();
        let mut stmt = self.conn.prepare(
            "SELECT currency, category, amount_minor FROM transactions WHERE day BETWEEN ?1 AND ?2",
        )?;
        let rows = stmt.query_map(params![from.to_string(), to.to_string()], |r| {
            Ok((r.get::<_, String>(0)?, r.get::<_, String>(1)?, r.get::<_, i64>(2)?))
        })?;
        for row in rows {
            let (currency, category, minor) = row?;
            let entry = by_currency.entry(currency).or_default();
            if minor >= 0 {
                entry.0 += minor;
            } else {
                entry.1 += -minor;
                *entry.2.entry(category).or_default() += -minor;
            }
        }
        Ok(by_currency
            .into_iter()
            .map(|(currency, (income, spending, cats))| {
                let mut cats: Vec<CategoryAmount> = cats
                    .into_iter()
                    .map(|(category, m)| CategoryAmount { category, amount: from_minor(m, &currency) })
                    .collect();
                cats.sort_by(|a, b| b.amount.total_cmp(&a.amount));
                CurrencySummary {
                    income: from_minor(income, &currency),
                    spending: from_minor(spending, &currency),
                    net: from_minor(income - spending, &currency),
                    spending_by_category: cats,
                    currency,
                }
            })
            .collect())
    }

    pub fn set_budget(&self, b: SetBudget) -> Result<Vec<Budget>> {
        let currency = currency_code(&b.currency)?;
        let category = key(&b.category);
        if b.monthly < 0.0 {
            bail!("a budget is a positive amount per month");
        }
        if b.monthly == 0.0 {
            self.conn.execute(
                "DELETE FROM budgets WHERE category = ?1 AND currency = ?2",
                params![category, currency],
            )?;
        } else {
            self.conn.execute(
                "INSERT INTO budgets(category, currency, monthly_minor) VALUES (?1, ?2, ?3)
                 ON CONFLICT(category, currency) DO UPDATE SET monthly_minor = ?3",
                params![category, currency, to_minor(b.monthly, &currency)],
            )?;
        }
        self.budgets()
    }

    pub fn budgets(&self) -> Result<Vec<Budget>> {
        let mut stmt = self
            .conn
            .prepare("SELECT category, currency, monthly_minor FROM budgets ORDER BY currency, category")?;
        let rows = stmt.query_map([], |r| {
            let currency: String = r.get(1)?;
            Ok(Budget { category: r.get(0)?, monthly: from_minor(r.get(2)?, &currency), currency })
        })?;
        Ok(rows.collect::<rusqlite::Result<_>>()?)
    }

    /// A month at a glance, up to `upto` when it falls inside the month.
    pub fn money_month(&self, month: Option<&str>, upto: NaiveDate) -> Result<MoneyMonth> {
        let start = match month {
            Some(m) => NaiveDate::parse_from_str(&format!("{m}-01"), "%Y-%m-%d")
                .map_err(|_| anyhow::anyhow!("month must be YYYY-MM, not '{m}'"))?,
            None => month_start(upto),
        };
        let end = month_end(start).min(upto.max(start));
        let currencies = self.finance_summary(start, end)?;
        let transactions = self.transactions(start, end)?;

        let budgets = self
            .budgets()?
            .into_iter()
            .map(|b| {
                let spent = currencies
                    .iter()
                    .find(|c| c.currency == b.currency)
                    .and_then(|c| c.spending_by_category.iter().find(|x| x.category == b.category))
                    .map(|x| x.amount)
                    .unwrap_or(0.0);
                BudgetLine { category: b.category, currency: b.currency, budget: b.monthly, spent }
            })
            .collect();

        let mut counts: BTreeMap<&str, usize> = BTreeMap::new();
        for t in &transactions {
            *counts.entry(t.currency.as_str()).or_default() += 1;
        }
        let primary = counts.into_iter().max_by_key(|(_, n)| *n).map(|(c, _)| c.to_string());

        let mut daily_spending = Vec::new();
        if let Some(primary) = &primary {
            let mut day = start;
            while day <= end {
                let ds = day.to_string();
                let amount = transactions
                    .iter()
                    .filter(|t| &t.currency == primary && t.day == ds && t.amount < 0.0)
                    .map(|t| -t.amount)
                    .sum();
                daily_spending.push(DayAmount { day: ds, amount });
                day += Duration::days(1);
            }
        }

        let recent = transactions.into_iter().rev().take(12).collect();
        Ok(MoneyMonth {
            month: start.format("%Y-%m").to_string(),
            currencies,
            budgets,
            primary_currency: primary,
            daily_spending,
            recent,
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::life::test_support::*;

    fn spend(life: &Life, amount: f64, currency: &str, category: &str, day: &str) {
        life.add_transaction(AddTransaction {
            amount,
            currency: currency.into(),
            category: category.into(),
            date: Some(d(day)),
            ..Default::default()
        })
        .unwrap();
    }

    #[test]
    fn money_is_stored_in_minor_units_per_currency() {
        let (_dir, life) = life();
        let day = "2026-11-25";
        spend(&life, -45_000.0, "vnd", "Ăn uống", day);
        spend(&life, -30_000.0, "VND", "ăn uống", day);
        spend(&life, 20_000_000.0, "VND", "lương", day);
        spend(&life, -9.99, "USD", "software", day);
        let s = life.finance_summary(d(day), d(day)).unwrap();
        let usd = s.iter().find(|c| c.currency == "USD").unwrap();
        assert_eq!(usd.spending, 9.99);
        let vnd = s.iter().find(|c| c.currency == "VND").unwrap();
        assert_eq!(vnd.spending, 75_000.0);
        assert_eq!(vnd.net, 19_925_000.0);
        assert_eq!(vnd.spending_by_category[0].category, "ăn uống");
        assert!(life.add_transaction(AddTransaction { currency: "đồng".into(), ..Default::default() }).is_err());
    }

    #[test]
    fn a_month_with_budgets_and_daily_spending() {
        let (_dir, life) = life();
        spend(&life, -100_000.0, "VND", "ăn uống", "2026-11-02");
        spend(&life, -50_000.0, "VND", "ăn uống", "2026-11-03");
        spend(&life, -20_000.0, "VND", "đi lại", "2026-11-03");
        spend(&life, -5.0, "USD", "app", "2026-11-03");
        spend(&life, -1.0, "VND", "ăn uống", "2026-10-31"); // another month
        life.set_budget(SetBudget { category: "Ăn uống".into(), currency: "vnd".into(), monthly: 3_000_000.0 })
            .unwrap();

        let m = life.money_month(None, d("2026-11-05")).unwrap();
        assert_eq!(m.month, "2026-11");
        assert_eq!(m.primary_currency.as_deref(), Some("VND"));
        assert_eq!(m.daily_spending.len(), 5);
        assert_eq!(m.daily_spending[2].amount, 70_000.0);
        assert_eq!(m.budgets, vec![BudgetLine {
            category: "ăn uống".into(),
            currency: "VND".into(),
            budget: 3_000_000.0,
            spent: 150_000.0
        }]);
        assert_eq!(m.recent.len(), 4);
        assert_eq!(life.money_month(Some("2026-10"), d("2026-11-05")).unwrap().daily_spending.len(), 31);
        assert!(life.money_month(Some("tháng 10"), d("2026-11-05")).is_err());

        life.set_budget(SetBudget { category: "ăn uống".into(), currency: "VND".into(), monthly: 0.0 }).unwrap();
        assert!(life.budgets().unwrap().is_empty());
    }
}
