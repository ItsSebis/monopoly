//! SQLite storage for archived runs. One table, no migrations tool (see
//! `docs/roadmap.md` Phase 4's scoping notes) — a single `CREATE TABLE IF
//! NOT EXISTS` at startup is all a single-table, single-user local archive
//! needs. `strategies` is a denormalized, comma-delimited (with leading and
//! trailing commas) copy of each player's strategy id, existing purely so
//! `GET /runs?strategy=` can filter with a `LIKE` clause that can't
//! false-positive on a substring (e.g. `buy_good` vs. `buy_goodie`) without
//! needing SQLite's JSON1 extension.
use rusqlite::{params, Connection, OptionalExtension};

const SCHEMA: &str = "
    CREATE TABLE IF NOT EXISTS runs (
        id TEXT PRIMARY KEY,
        kind TEXT NOT NULL,
        created_at TEXT NOT NULL,
        strategies TEXT NOT NULL,
        record TEXT NOT NULL
    )
";

pub fn open(path: &str) -> rusqlite::Result<Connection> {
    let conn = Connection::open(path)?;
    conn.execute_batch(SCHEMA)?;
    Ok(conn)
}

pub fn insert(
    conn: &Connection,
    id: &str,
    kind: &str,
    created_at: &str,
    strategies: &str,
    record: &str,
) -> rusqlite::Result<()> {
    conn.execute(
        "INSERT INTO runs (id, kind, created_at, strategies, record) VALUES (?1, ?2, ?3, ?4, ?5)",
        params![id, kind, created_at, strategies, record],
    )?;
    Ok(())
}

pub fn get(conn: &Connection, id: &str) -> rusqlite::Result<Option<String>> {
    conn.query_row(
        "SELECT record FROM runs WHERE id = ?1",
        params![id],
        |row| row.get(0),
    )
    .optional()
}

pub fn list(
    conn: &Connection,
    kind: Option<&str>,
    strategy: Option<&str>,
) -> rusqlite::Result<Vec<(String, String, String, String)>> {
    let mut stmt = conn.prepare(
        "SELECT id, kind, created_at, record FROM runs
         WHERE (?1 IS NULL OR kind = ?1)
           AND (?2 IS NULL OR strategies LIKE '%,' || ?2 || ',%')
         ORDER BY created_at DESC",
    )?;
    let rows = stmt.query_map(params![kind, strategy], |row| {
        Ok((row.get(0)?, row.get(1)?, row.get(2)?, row.get(3)?))
    })?;
    rows.collect()
}

/// Returns whether a row was actually deleted.
pub fn delete(conn: &Connection, id: &str) -> rusqlite::Result<bool> {
    let affected = conn.execute("DELETE FROM runs WHERE id = ?1", params![id])?;
    Ok(affected > 0)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn memory_db() -> Connection {
        open(":memory:").unwrap()
    }

    #[test]
    fn insert_then_get_round_trips_the_record() {
        let conn = memory_db();
        insert(
            &conn,
            "run_1",
            "single",
            "2026-01-01T00:00:00Z",
            ",buy_good,",
            "{}",
        )
        .unwrap();
        assert_eq!(get(&conn, "run_1").unwrap(), Some("{}".to_string()));
        assert_eq!(get(&conn, "run_missing").unwrap(), None);
    }

    #[test]
    fn list_filters_by_kind_and_strategy_without_substring_false_positives() {
        let conn = memory_db();
        insert(&conn, "a", "single", "t1", ",buy_good,", "{}").unwrap();
        insert(&conn, "b", "batch", "t2", ",buy_goodie,buy_bad,", "{}").unwrap();
        insert(&conn, "c", "batch", "t3", ",buy_good,buy_bad,", "{}").unwrap();

        assert_eq!(list(&conn, Some("batch"), None).unwrap().len(), 2);
        // Among the batch rows, only "c" actually has the exact "buy_good"
        // segment; "b"'s "buy_goodie" must not match.
        let matches = list(&conn, Some("batch"), Some("buy_good")).unwrap();
        assert_eq!(matches.len(), 1);
        assert_eq!(matches[0].0, "c");
    }

    #[test]
    fn delete_reports_whether_a_row_actually_existed() {
        let conn = memory_db();
        insert(&conn, "a", "single", "t1", ",buy_good,", "{}").unwrap();
        assert!(delete(&conn, "a").unwrap());
        assert!(!delete(&conn, "a").unwrap());
    }
}
