// SPDX-FileCopyrightText: Qubernetic and the InboxCraft contributors
// SPDX-License-Identifier: MIT OR Apache-2.0

//! Faults made in a store's database from outside, the way a second program, a damaged disk or a crafted file would
//! make them, for tests that check what a caller reads when the store fails: [`hold`] (busy), [`damage`] (damaged),
//! [`refuse_updates`] (a constraint failure), [`drop_table`] (a failure of no kind of its own), [`stale_version`] (a
//! failed migration), and [`leaks`], what of the database's own words an answer gives away.

use std::collections::BTreeSet;
use std::error::Error;
use std::path::Path;

use inboxcraft_store::DATABASE_FILE;
use rusqlite::Connection;

/// The name of the trigger [`refuse_updates`] adds, which is also its refusal's words.
pub const REFUSAL: &str = "fault_refusal";

fn open(dir: &Path) -> Result<Connection, Box<dyn Error>> {
    Ok(Connection::open(dir.join(DATABASE_FILE))?)
}

/// Every name the store's schema holds, lower case: tables (with the full-text index's own), indexes, triggers and
/// every table's columns. Read from the database itself, so a new table or column is covered.
///
/// # Errors
///
/// The database cannot be read.
pub fn schema_words(dir: &Path) -> Result<BTreeSet<String>, Box<dyn Error>> {
    let conn = open(dir)?;
    let mut words = BTreeSet::new();
    let mut tables = Vec::new();
    let mut names = conn.prepare("SELECT type, name, tbl_name FROM sqlite_master")?;
    let rows = names.query_map([], |row| Ok((row.get::<_, String>(0)?, row.get::<_, String>(1)?, row.get::<_, String>(2)?)))?;
    for row in rows {
        let (kind, name, table) = row?;
        if kind == "table" {
            tables.push(name.clone());
        }
        words.insert(name.to_lowercase());
        words.insert(table.to_lowercase());
    }
    for table in tables {
        let mut columns = conn.prepare("SELECT name FROM pragma_table_info(?1)")?;
        for column in columns.query_map([&table], |row| row.get::<_, String>(0))? {
            words.insert(column?.to_lowercase());
        }
    }
    Ok(words)
}

/// A second connection that holds the database's write lock until it drops: the store's writes (and its opening, which
/// migrates in a write transaction) meet a busy database; its reads still answer, as the write-ahead log lets them.
pub struct Hold(Connection);

impl Hold {
    /// Lets the database go.
    ///
    /// # Errors
    ///
    /// The hold's transaction cannot end.
    pub fn release(self) -> Result<(), Box<dyn Error>> {
        self.0.execute_batch("COMMIT")?;
        Ok(())
    }
}

/// Holds the write lock of the database in `dir` with a write left open, so nobody else can write it.
///
/// # Errors
///
/// The database cannot be locked.
pub fn hold(dir: &Path) -> Result<Hold, Box<dyn Error>> {
    let conn = open(dir)?;
    conn.execute_batch("BEGIN IMMEDIATE; UPDATE meta SET value = value;")?;
    Ok(Hold(conn))
}

/// Points the table `table` at a page the file does not have and tells every connection the schema changed: the
/// next read of `table` meets a damaged database.
///
/// # Errors
///
/// The database cannot be written.
pub fn damage(dir: &Path, table: &str) -> Result<(), Box<dyn Error>> {
    let conn = open(dir)?;
    let version: i64 = conn.pragma_query_value(None, "schema_version", |row| row.get(0))?;
    conn.execute_batch("PRAGMA writable_schema = ON;")?;
    conn.execute("UPDATE sqlite_master SET rootpage = 1000000 WHERE name = ?1", [table])?;
    conn.execute_batch(&format!("PRAGMA writable_schema = OFF; PRAGMA schema_version = {};", version.saturating_add(1)))?;
    Ok(())
}

/// Adds a trigger that refuses every update of `table` with [`REFUSAL`]: a constraint failure.
///
/// # Errors
///
/// The database cannot be written.
pub fn refuse_updates(dir: &Path, table: &str) -> Result<(), Box<dyn Error>> {
    open(dir)?.execute_batch(&format!("CREATE TRIGGER {REFUSAL} BEFORE UPDATE ON {table} BEGIN SELECT RAISE(ABORT, '{REFUSAL}'); END;"))?;
    Ok(())
}

/// Removes the table `table`: a read of it fails with no kind of its own.
///
/// # Errors
///
/// The database cannot be written.
pub fn drop_table(dir: &Path, table: &str) -> Result<(), Box<dyn Error>> {
    open(dir)?.execute_batch(&format!("DROP TABLE {table};"))?;
    Ok(())
}

/// Sets the schema version back to 3 and removes the `job` table: the step to version 4 then meets the `meta` table it
/// would make, and the migration fails.
///
/// # Errors
///
/// The database cannot be written.
pub fn stale_version(dir: &Path) -> Result<(), Box<dyn Error>> {
    open(dir)?.execute_batch("PRAGMA user_version = 3; DROP TABLE job;")?;
    Ok(())
}

/// What `answer` gives away of the store in `dir`: each word of it (letters, digits and `_`) that is one of `words`
/// (from [`schema_words`]), the word `sqlite` or `sql` in any case, and the directory's path.
#[must_use]
pub fn leaks(answer: &str, words: &BTreeSet<String>, dir: &Path) -> Vec<String> {
    let mut found: Vec<String> = answer
        .split(|c: char| !(c.is_alphanumeric() || c == '_'))
        .map(str::to_lowercase)
        .filter(|word| words.contains(word) || word.contains("sql"))
        .collect();
    let path = dir.display().to_string();
    if answer.contains(&path) || answer.contains('/') || answer.contains('\\') {
        found.push(path);
    }
    found
}
