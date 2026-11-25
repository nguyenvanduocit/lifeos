use chrono::NaiveDate;
use plugin_sdk::{Definition, Effect, Error, Tool};
use rusqlite::{Connection, params};
use schemars::JsonSchema;
use serde::{Deserialize, Serialize};
use std::sync::Mutex;

#[derive(Deserialize, JsonSchema)]
struct Month {
    month: String,
}
#[derive(Deserialize, JsonSchema)]
struct Remove {
    id: i64,
}
#[derive(Deserialize, Serialize, JsonSchema)]
struct Expense {
    id: Option<i64>,
    date: String,
    /// One of fuel, service, repair, parking, toll, insurance, other.
    category: String,
    amount: i64,
    km: Option<i64>,
    litres: Option<f64>,
    #[serde(default)]
    note: String,
}

fn list(db: &Connection, input: Month) -> Result<String, Error> {
    let start = NaiveDate::parse_from_str(&format!("{}-01", input.month), "%Y-%m-%d")?;
    if input.month.len() != 7 || start.format("%Y-%m").to_string() != input.month {
        return Err("Tháng không hợp lệ".into());
    }
    let mut query = db.prepare("SELECT id,date,category,amount,km,litres,note FROM plugin_car_expenses WHERE date >= ? AND date < ? ORDER BY date DESC,id DESC")?;
    let rows = query
        .query_map(
            params![start.to_string(), format!("{}-32", input.month)],
            |row| {
                Ok(Expense {
                    id: row.get(0)?,
                    date: row.get(1)?,
                    category: row.get(2)?,
                    amount: row.get(3)?,
                    km: row.get(4)?,
                    litres: row.get(5)?,
                    note: row.get(6)?,
                })
            },
        )?
        .collect::<Result<Vec<_>, _>>()?;
    Ok(serde_json::to_string(&rows)?)
}

fn save(db: &Connection, e: Expense) -> Result<String, Error> {
    let date = NaiveDate::parse_from_str(&e.date, "%Y-%m-%d")?;
    if date.to_string() != e.date
        || e.date.len() != 10
        || !(1..=999_999_999_999).contains(&e.amount)
        || !matches!(
            e.category.as_str(),
            "fuel" | "service" | "repair" | "parking" | "toll" | "insurance" | "other"
        )
        || e.km.is_some_and(|v| !(0..=99_999_999).contains(&v))
        || e.litres
            .is_some_and(|v| e.category != "fuel" || !(v > 0. && v <= 9999.))
        || e.note.chars().count() > 500
    {
        return Err("Thông tin khoản chi không hợp lệ".into());
    }
    let values = params![e.date, e.category, e.amount, e.km, e.litres, e.note];
    let id = if let Some(id) = e.id {
        if db.execute(
            "UPDATE plugin_car_expenses SET date=?,category=?,amount=?,km=?,litres=?,note=? WHERE id=?",
            params![e.date, e.category, e.amount, e.km, e.litres, e.note, id],
        )? == 0
        {
            return Err("Khoản chi không còn tồn tại".into());
        }
        id
    } else {
        db.execute(
            "INSERT INTO plugin_car_expenses(date,category,amount,km,litres,note) VALUES (?,?,?,?,?,?)",
            values,
        )?;
        db.last_insert_rowid()
    };
    Ok(id.to_string())
}

fn open_storage(shared: &std::path::Path, legacy: &std::path::Path) -> Result<Connection, Error> {
    let mut attached = false;
    let db = plugin_sdk::storage::open(shared, "car", 1, |tx, _| {
        tx.execute_batch("CREATE TABLE plugin_car_expenses(id INTEGER PRIMARY KEY AUTOINCREMENT,date TEXT NOT NULL,category TEXT NOT NULL,amount INTEGER NOT NULL,km INTEGER,litres REAL,note TEXT NOT NULL);
            CREATE INDEX plugin_car_expenses_date ON plugin_car_expenses(date);")?;
        if legacy.try_exists()? {
            tx.execute(
                "ATTACH DATABASE ? AS car_legacy",
                [legacy.to_str().ok_or("Invalid legacy path")?],
            )?;
            attached = true;
            tx.execute_batch("INSERT INTO plugin_car_expenses(id,date,category,amount,km,litres,note)
                SELECT id,date,category,amount,km,litres,note FROM car_legacy.expenses;
                UPDATE main.sqlite_sequence SET seq=MAX(seq,COALESCE((SELECT seq FROM car_legacy.sqlite_sequence WHERE name='expenses'),0))
                WHERE name='plugin_car_expenses';")?;
        }
        Ok(())
    })?;
    if attached {
        db.execute_batch("DETACH DATABASE car_legacy")?;
    }
    Ok(db)
}

fn main() -> Result<std::process::ExitCode, Error> {
    let path = std::env::args_os()
        .nth(1)
        .ok_or("Expected a database path")?;
    let path = std::path::PathBuf::from(path);
    let (shared, legacy) = if path.file_name().is_some_and(|n| n == "lifeos.db") {
        (path.clone(), path.with_file_name("car-expenses.sqlite"))
    } else {
        (path.with_file_name("lifeos.db"), path)
    };
    let db = open_storage(&shared, &legacy)?;
    let db = std::sync::Arc::new(Mutex::new(db));
    let read = db.clone();
    let write = db.clone();

    Ok(plugin_sdk::serve(Definition {
        tools: vec![
            Tool::handler("list_expenses", Effect::Read,
                move |_, m: Month| {
                    let result = list(&read.lock().unwrap(), m);
                    async move { result }
                },
            ),
            Tool::handler("save_expense", Effect::Reversible,
                move |_, e: Expense| {
                    let result = save(&write.lock().unwrap(), e);
                    async move { result }
                },
            ),
            Tool::handler("delete_expense", Effect::Irreversible,
                move |_, e: Remove| {
                    let result = db
                        .lock()
                        .unwrap()
                        .execute("DELETE FROM plugin_car_expenses WHERE id=?", [e.id]);
                    async move { Ok(result?.to_string()) }
                },
            ),
        ],
        ..Default::default()
    }.bind_manifest(include_str!("../manifest.json")).expect("package manifest")))
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn migration_preserves_entries_and_retired_ids_once() -> Result<(), Error> {
        let dir = tempfile::tempdir()?;
        let shared = dir.path().join("lifeos.db");
        Connection::open(&shared)?.pragma_update(None, "user_version", 24)?;
        let legacy = dir.path().join("car-expenses.sqlite");
        Connection::open(&legacy)?.execute_batch(
            "CREATE TABLE expenses(id INTEGER PRIMARY KEY AUTOINCREMENT,date,category,amount,km,litres,note);
             INSERT INTO expenses VALUES(8,'2026-11-01','fuel',150000,1200,5,'memo');
             INSERT INTO expenses DEFAULT VALUES; DELETE FROM expenses WHERE id=9;")?;
        let db = open_storage(&shared, &legacy)?;
        let rows = list(
            &db,
            Month {
                month: "2026-11".into(),
            },
        )?;
        let entries: Vec<Expense> = serde_json::from_str(&rows)?;
        assert_eq!(entries.len(), 1);
        let mut expense = entries.into_iter().next().unwrap();
        assert_eq!(
            (
                expense.id,
                expense.amount,
                expense.km,
                expense.litres,
                expense.note.as_str()
            ),
            (Some(8), 150000, Some(1200), Some(5.), "memo")
        );
        db.execute("DELETE FROM plugin_car_expenses", [])?;
        drop(db);
        let db = open_storage(&shared, &legacy)?;
        assert_eq!(
            list(
                &db,
                Month {
                    month: "2026-11".into()
                }
            )?,
            "[]"
        );
        expense.id = None;
        assert_eq!(save(&db, expense)?, "10");
        assert_eq!(
            db.query_row("PRAGMA user_version", [], |r| r.get::<_, u32>(0))?,
            24
        );
        Ok(())
    }
}
