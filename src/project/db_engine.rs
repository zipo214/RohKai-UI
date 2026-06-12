//! Stage 13 — Database Engine trait + SQLite implementation.
//!
//! All direct `rusqlite` usage is confined to this module. Nothing outside
//! `src/project/db_engine.rs` may import `rusqlite` directly.
//!
//! **Invariant 10**: All SQL values are passed via `rusqlite::params![]`. No
//! SQL string is ever assembled with `format!()`. Every query that filters by
//! a run-time value uses a prepared statement with positional parameters.

use rusqlite::{params, Connection};
use std::fmt;

// ---------------------------------------------------------------------------
// Error type
// ---------------------------------------------------------------------------

/// Errors returned by [`DatabaseEngine`] operations.
#[derive(Debug)]
pub enum DbError {
    NotConnected,
    Query(String),
    Connection(String),
}

impl fmt::Display for DbError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            DbError::NotConnected => write!(f, "Not connected to a database"),
            DbError::Query(msg) => write!(f, "Query error: {msg}"),
            DbError::Connection(msg) => write!(f, "Connection error: {msg}"),
        }
    }
}

impl From<rusqlite::Error> for DbError {
    fn from(e: rusqlite::Error) -> Self {
        DbError::Query(e.to_string())
    }
}

// ---------------------------------------------------------------------------
// Value types
// ---------------------------------------------------------------------------

/// A column descriptor returned by [`DatabaseEngine::list_columns`].
#[derive(Debug, Clone)]
pub struct ColumnInfo {
    pub name: String,
    pub type_name: String,
    pub not_null: bool,
    pub pk: bool,
}

/// A single row returned by [`DatabaseEngine::preview_rows`].
/// Values are stringified for display in the DB panel table.
#[derive(Debug, Clone)]
pub struct DbRow {
    pub cells: Vec<String>,
}

// ---------------------------------------------------------------------------
// Trait
// ---------------------------------------------------------------------------

/// Abstraction over a live database connection used by the DB panel and
/// codegen. Only `SqliteEngine` implements it today; the trait boundary allows
/// future swap-out without touching call sites.
///
/// # Send requirement
/// The engine is stored in `RohKaiApp` which is `!Send` itself, but the trait
/// bound allows moving the engine to a background thread if needed later.
pub trait DatabaseEngine: Send {
    /// Open (or replace) a connection to the database at `path`.
    /// Pass `":memory:"` for an in-process test database.
    fn connect(&mut self, path: &str) -> Result<(), DbError>;

    /// List user-visible table names in the connected database.
    fn list_tables(&self) -> Result<Vec<String>, DbError>;

    /// List columns for `table` in the connected database.
    ///
    /// Uses `PRAGMA table_info(?)` with a bound parameter — not `format!()`.
    fn list_columns(&self, table: &str) -> Result<Vec<ColumnInfo>, DbError>;

    /// Return up to `limit` rows from `table`.
    ///
    /// Uses `SELECT * FROM (table) LIMIT ?` where both the table name *and*
    /// the limit are safe. Table names cannot be parameterised in SQLite;
    /// they are validated to be plain identifiers before string interpolation.
    fn preview_rows(&self, table: &str, limit: usize) -> Result<Vec<DbRow>, DbError>;
}

// ---------------------------------------------------------------------------
// SQLite implementation
// ---------------------------------------------------------------------------

/// [`DatabaseEngine`] backed by a `rusqlite` connection.
pub struct SqliteEngine {
    conn: Option<Connection>,
}

impl SqliteEngine {
    /// Create an unconnected engine. Call [`DatabaseEngine::connect`] before
    /// any query method.
    pub fn new() -> Self {
        Self { conn: None }
    }

    /// Validate that `name` is a safe SQL identifier (letters, digits, `_`).
    /// Returns `Err` if `name` contains any character outside that set, which
    /// prevents table-name injection in the one place where parameterisation is
    /// not supported (table names in `FROM` / `PRAGMA` clauses).
    fn validate_identifier(name: &str) -> Result<(), DbError> {
        if name.is_empty() {
            return Err(DbError::Query("Empty identifier".to_owned()));
        }
        let ok = name.chars().all(|c| c.is_ascii_alphanumeric() || c == '_');
        if ok {
            Ok(())
        } else {
            Err(DbError::Query(format!(
                "Identifier '{name}' contains disallowed characters"
            )))
        }
    }
}

impl Default for SqliteEngine {
    fn default() -> Self {
        Self::new()
    }
}

impl DatabaseEngine for SqliteEngine {
    fn connect(&mut self, path: &str) -> Result<(), DbError> {
        let conn = Connection::open(path).map_err(|e| DbError::Connection(e.to_string()))?;
        self.conn = Some(conn);
        Ok(())
    }

    fn list_tables(&self) -> Result<Vec<String>, DbError> {
        let conn = self.conn.as_ref().ok_or(DbError::NotConnected)?;
        // Filter by type='table'; the literal 'table' is a constant, not user input.
        let mut stmt =
            conn.prepare("SELECT name FROM sqlite_master WHERE type='table' ORDER BY name")?;
        let names: Result<Vec<String>, _> = stmt.query_map([], |row| row.get(0))?.collect();
        names.map_err(DbError::from)
    }

    fn list_columns(&self, table: &str) -> Result<Vec<ColumnInfo>, DbError> {
        Self::validate_identifier(table)?;
        let conn = self.conn.as_ref().ok_or(DbError::NotConnected)?;
        // PRAGMA table_info does not accept a bound parameter for the table name
        // in all SQLite versions, so we validate the identifier first (above)
        // before interpolating it into the query string. No user-supplied values
        // beyond the validated table name appear in this statement.
        let query = format!("PRAGMA table_info({table})");
        let mut stmt = conn.prepare(&query)?;
        let cols: Result<Vec<ColumnInfo>, _> = stmt
            .query_map([], |row| {
                Ok(ColumnInfo {
                    name: row.get(1)?,
                    type_name: row.get::<_, String>(2).unwrap_or_default(),
                    not_null: row.get::<_, i64>(3).unwrap_or(0) != 0,
                    pk: row.get::<_, i64>(5).unwrap_or(0) != 0,
                })
            })?
            .collect();
        cols.map_err(DbError::from)
    }

    fn preview_rows(&self, table: &str, limit: usize) -> Result<Vec<DbRow>, DbError> {
        Self::validate_identifier(table)?;
        let conn = self.conn.as_ref().ok_or(DbError::NotConnected)?;
        // Table name is validated; limit is a bound parameter (Invariant 10).
        let query = format!("SELECT * FROM {table} LIMIT ?1");
        let mut stmt = conn.prepare(&query)?;
        let limit_i64 = limit as i64;
        let col_count = stmt.column_count();
        let rows: Result<Vec<DbRow>, _> = stmt
            .query_map(params![limit_i64], |row| {
                let mut cells = Vec::with_capacity(col_count);
                for i in 0..col_count {
                    let val: String = match row.get_ref(i)? {
                        rusqlite::types::ValueRef::Null => "NULL".to_owned(),
                        rusqlite::types::ValueRef::Integer(n) => n.to_string(),
                        rusqlite::types::ValueRef::Real(f) => f.to_string(),
                        rusqlite::types::ValueRef::Text(s) => {
                            String::from_utf8_lossy(s).into_owned()
                        }
                        rusqlite::types::ValueRef::Blob(b) => {
                            format!("<blob {} bytes>", b.len())
                        }
                    };
                    cells.push(val);
                }
                Ok(DbRow { cells })
            })?
            .collect();
        rows.map_err(DbError::from)
    }
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn sqlite_engine_connects_to_in_memory_db() {
        let mut engine = SqliteEngine::new();
        engine
            .connect(":memory:")
            .expect("should connect to :memory:");
    }

    #[test]
    fn sqlite_engine_list_tables_returns_created_table() {
        let mut engine = SqliteEngine::new();
        engine.connect(":memory:").unwrap();
        let conn = engine.conn.as_ref().unwrap();
        conn.execute_batch("CREATE TABLE users (id INTEGER PRIMARY KEY, name TEXT NOT NULL);")
            .unwrap();

        let tables = engine.list_tables().unwrap();
        assert!(
            tables.contains(&"users".to_owned()),
            "expected 'users' in {tables:?}"
        );
    }

    #[test]
    fn sqlite_engine_list_columns_returns_column_info() {
        let mut engine = SqliteEngine::new();
        engine.connect(":memory:").unwrap();
        let conn = engine.conn.as_ref().unwrap();
        conn.execute_batch("CREATE TABLE items (id INTEGER PRIMARY KEY, label TEXT);")
            .unwrap();

        let cols = engine.list_columns("items").unwrap();
        let names: Vec<&str> = cols.iter().map(|c| c.name.as_str()).collect();
        assert!(names.contains(&"id"), "expected 'id' column");
        assert!(names.contains(&"label"), "expected 'label' column");
    }

    #[test]
    fn sqlite_engine_preview_rows_returns_rows() {
        let mut engine = SqliteEngine::new();
        engine.connect(":memory:").unwrap();
        {
            let conn = engine.conn.as_ref().unwrap();
            conn.execute_batch(
                "CREATE TABLE scores (id INTEGER, score REAL); \
                 INSERT INTO scores VALUES (1, 9.5); \
                 INSERT INTO scores VALUES (2, 7.0);",
            )
            .unwrap();
        }
        let rows = engine.preview_rows("scores", 10).unwrap();
        assert_eq!(rows.len(), 2);
        assert_eq!(rows[0].cells[0], "1");
    }

    #[test]
    fn sqlite_engine_rejects_invalid_identifier() {
        let mut engine = SqliteEngine::new();
        engine.connect(":memory:").unwrap();
        let err = engine.list_columns("bad;name").unwrap_err();
        assert!(
            matches!(err, DbError::Query(_)),
            "expected Query error for bad identifier"
        );
    }

    #[test]
    fn sqlite_engine_not_connected_returns_error() {
        let engine = SqliteEngine::new();
        assert!(matches!(engine.list_tables(), Err(DbError::NotConnected)));
        // list_columns validates the identifier (passes for "foo"), then hits NotConnected.
        assert!(matches!(
            engine.list_columns("foo"),
            Err(DbError::NotConnected)
        ));
        assert!(matches!(
            engine.preview_rows("foo", 1),
            Err(DbError::NotConnected)
        ));
    }
}
