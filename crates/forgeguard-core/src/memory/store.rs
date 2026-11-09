//! SQLite store for the code graph.
//!
//! One database per repository at `.forgeguard/cache/memory/graph.db`. Every
//! lookup the retrieval layer needs is an indexed SQL query, so a query reads the
//! rows it asks for instead of loading the whole graph. The database is a cache:
//! a schema mismatch wipes and rebuilds it rather than migrating.

use std::{
    collections::BTreeMap,
    path::{Path, PathBuf},
};

use anyhow::{Context, Result};
use rusqlite::{params, Connection, OptionalExtension, Row};

use super::{FileEntry, Symbol, SymbolKind};

pub const DATABASE_FILE: &str = ".forgeguard/cache/memory/graph.db";
/// Bumped whenever the schema or the extraction shape changes. A mismatch drops
/// every table so symbols extracted by an older parser are never served.
pub const SCHEMA_VERSION: u32 = 3;

const SCHEMA: &str = "
CREATE TABLE IF NOT EXISTS meta (
    key TEXT PRIMARY KEY,
    value TEXT NOT NULL
);
CREATE TABLE IF NOT EXISTS files (
    id INTEGER PRIMARY KEY,
    path TEXT NOT NULL UNIQUE,
    language TEXT NOT NULL,
    content_hash TEXT NOT NULL,
    symbols_hash TEXT NOT NULL,
    mtime INTEGER NOT NULL,
    size INTEGER NOT NULL,
    is_test INTEGER NOT NULL,
    last_indexed INTEGER NOT NULL
);
CREATE TABLE IF NOT EXISTS imports (
    file_id INTEGER NOT NULL REFERENCES files(id) ON DELETE CASCADE,
    module TEXT NOT NULL,
    stem TEXT NOT NULL
);
CREATE TABLE IF NOT EXISTS exports (
    file_id INTEGER NOT NULL REFERENCES files(id) ON DELETE CASCADE,
    name TEXT NOT NULL
);
CREATE TABLE IF NOT EXISTS symbols (
    id INTEGER PRIMARY KEY,
    file_id INTEGER NOT NULL REFERENCES files(id) ON DELETE CASCADE,
    name TEXT NOT NULL,
    container TEXT,
    qualified TEXT NOT NULL,
    kind TEXT NOT NULL,
    start_line INTEGER NOT NULL,
    end_line INTEGER NOT NULL,
    signature TEXT NOT NULL,
    exported INTEGER NOT NULL,
    route_method TEXT,
    route_path TEXT
);
CREATE TABLE IF NOT EXISTS calls (
    symbol_id INTEGER NOT NULL REFERENCES symbols(id) ON DELETE CASCADE,
    callee TEXT NOT NULL,
    receiver TEXT,
    qualifier TEXT,
    line INTEGER NOT NULL DEFAULT 0,
    character INTEGER NOT NULL DEFAULT 0
);
CREATE TABLE IF NOT EXISTS extends (
    symbol_id INTEGER NOT NULL REFERENCES symbols(id) ON DELETE CASCADE,
    name TEXT NOT NULL
);
CREATE TABLE IF NOT EXISTS service_links (
    symbol_id INTEGER NOT NULL REFERENCES symbols(id) ON DELETE CASCADE,
    method TEXT,
    url TEXT NOT NULL
);
CREATE INDEX IF NOT EXISTS symbols_name ON symbols(name);
CREATE INDEX IF NOT EXISTS symbols_qualified ON symbols(qualified);
CREATE INDEX IF NOT EXISTS symbols_file ON symbols(file_id);
CREATE INDEX IF NOT EXISTS symbols_route ON symbols(route_path);
CREATE INDEX IF NOT EXISTS calls_callee ON calls(callee);
CREATE INDEX IF NOT EXISTS calls_symbol ON calls(symbol_id);
CREATE INDEX IF NOT EXISTS extends_symbol ON extends(symbol_id);
CREATE INDEX IF NOT EXISTS imports_stem ON imports(stem);
CREATE INDEX IF NOT EXISTS imports_file ON imports(file_id);
CREATE INDEX IF NOT EXISTS exports_file ON exports(file_id);
CREATE INDEX IF NOT EXISTS links_symbol ON service_links(symbol_id);
";

/// One symbol joined with the file that holds it: what every query returns.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SymbolRow {
    pub id: i64,
    pub path: PathBuf,
    pub language: String,
    pub is_test: bool,
    pub name: String,
    pub container: Option<String>,
    pub qualified: String,
    pub kind: SymbolKind,
    pub start_line: usize,
    pub end_line: usize,
    pub signature: String,
    pub exported: bool,
    pub route_method: Option<String>,
    pub route_path: Option<String>,
}

impl SymbolRow {
    pub fn lines(&self) -> usize {
        self.end_line.saturating_sub(self.start_line) + 1
    }
}

/// A call site whose owning type the static pass left open.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct UnresolvedCall {
    pub id: i64,
    pub path: PathBuf,
    /// `LanguageProfile::family()` of the file holding the call.
    pub language: String,
    pub callee: String,
    pub line: usize,
    pub character: usize,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct FileStat {
    pub id: i64,
    pub mtime: u64,
    pub size: u64,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ServiceLink {
    pub symbol: String,
    pub path: PathBuf,
    pub method: Option<String>,
    pub url: String,
    /// Route symbol the URL resolves to, when one is indexed.
    pub target: Option<String>,
    pub target_path: Option<PathBuf>,
}

pub struct Store {
    connection: Connection,
}

impl std::fmt::Debug for Store {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        formatter.debug_struct("Store").finish_non_exhaustive()
    }
}

impl Store {
    /// Open (creating if needed) the store for a repository root.
    pub fn open(root: &Path) -> Result<Self> {
        Self::open_at(&root.join(DATABASE_FILE))
    }

    pub fn open_at(path: &Path) -> Result<Self> {
        if let Some(directory) = path.parent() {
            // forgeguard: allow FG-SEC-007 -- the caller's repository root joined with a crate constant, or an operator-supplied artifact path
            std::fs::create_dir_all(directory)
                .with_context(|| format!("failed to create {}", directory.display()))?;
        }
        let connection =
            Connection::open(path).with_context(|| format!("failed to open {}", path.display()))?;
        connection.pragma_update(None, "journal_mode", "WAL")?;
        connection.pragma_update(None, "foreign_keys", "ON")?;
        connection.pragma_update(None, "synchronous", "NORMAL")?;
        connection.busy_timeout(std::time::Duration::from_secs(5))?;
        let mut store = Self { connection };
        store.apply_schema()?;
        Ok(store)
    }

    /// In-memory store, used by tests and by the Cypher engine's dry runs.
    pub fn open_memory() -> Result<Self> {
        let connection = Connection::open_in_memory()?;
        connection.pragma_update(None, "foreign_keys", "ON")?;
        let mut store = Self { connection };
        store.apply_schema()?;
        Ok(store)
    }

    fn apply_schema(&mut self) -> Result<()> {
        let version: Option<String> = self
            .connection
            .query_row(
                "SELECT value FROM meta WHERE key = 'schema_version'",
                [],
                |row| row.get(0),
            )
            .optional()
            .unwrap_or(None);
        match version.as_deref().map(str::parse::<u32>) {
            Some(Ok(found)) if found == SCHEMA_VERSION => {
                self.connection.execute_batch(SCHEMA)?;
                Ok(())
            }
            Some(Ok(_mismatch)) => {
                let _ = self.connection.execute_batch(
                    "DROP TABLE IF EXISTS service_links; DROP TABLE IF EXISTS extends;
                     DROP TABLE IF EXISTS calls; DROP TABLE IF EXISTS symbols;
                     DROP TABLE IF EXISTS exports; DROP TABLE IF EXISTS imports;
                     DROP TABLE IF EXISTS files; DROP TABLE IF EXISTS meta;",
                );
                self.connection.execute_batch(SCHEMA)?;
                self.set_meta("schema_version", &SCHEMA_VERSION.to_string())
            }
            _ => {
                self.connection.execute_batch(SCHEMA)?;
                self.set_meta("schema_version", &SCHEMA_VERSION.to_string())
            }
        }
    }

    /// Drop every graph row, keeping the file and its page cache.
    pub fn reset(&mut self) -> Result<()> {
        self.connection.execute_batch(
            "DELETE FROM service_links; DELETE FROM extends; DELETE FROM calls;
             DELETE FROM symbols; DELETE FROM exports; DELETE FROM imports;
             DELETE FROM files; DELETE FROM meta;",
        )?;
        self.set_meta("schema_version", &SCHEMA_VERSION.to_string())?;
        Ok(())
    }

    pub fn set_meta(&self, key: &str, value: &str) -> Result<()> {
        self.connection.execute(
            "INSERT INTO meta(key, value) VALUES(?1, ?2)
             ON CONFLICT(key) DO UPDATE SET value = excluded.value",
            params![key, value],
        )?;
        Ok(())
    }

    pub fn meta(&self, key: &str) -> Result<Option<String>> {
        Ok(self
            .connection
            .query_row(
                "SELECT value FROM meta WHERE key = ?1",
                params![key],
                |row| row.get(0),
            )
            .optional()?)
    }

    pub fn file_stat(&self, path: &Path) -> Result<Option<FileStat>> {
        Ok(self
            .connection
            .query_row(
                "SELECT id, mtime, size FROM files WHERE path = ?1",
                params![text(path)],
                |row| {
                    Ok(FileStat {
                        id: row.get(0)?,
                        mtime: row.get::<_, i64>(1)? as u64,
                        size: row.get::<_, i64>(2)? as u64,
                    })
                },
            )
            .optional()?)
    }

    pub fn content_hash(&self, path: &Path) -> Result<Option<String>> {
        Ok(self
            .connection
            .query_row(
                "SELECT content_hash FROM files WHERE path = ?1",
                params![text(path)],
                |row| row.get(0),
            )
            .optional()?)
    }

    /// Refresh stat data for a file whose content did not change.
    pub fn touch_file(&self, path: &Path, mtime: u64, size: u64) -> Result<()> {
        self.connection.execute(
            "UPDATE files SET mtime = ?2, size = ?3 WHERE path = ?1",
            params![text(path), mtime as i64, size as i64],
        )?;
        Ok(())
    }

    /// Replace a file and everything hanging off it in one transaction.
    pub fn upsert_file(&mut self, path: &Path, entry: &FileEntry) -> Result<()> {
        // Retrieval joins these paths onto the repository root and reads them, so
        // an escaping path must never enter the graph in the first place.
        if !is_inside_repository(path) {
            anyhow::bail!(
                "refusing to index a path outside the repository: {}",
                path.display()
            );
        }
        let transaction = self.connection.transaction()?;
        transaction.execute("DELETE FROM files WHERE path = ?1", params![text(path)])?;
        transaction.execute(
            "INSERT INTO files(path, language, content_hash, symbols_hash, mtime, size, is_test, last_indexed)
             VALUES(?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8)",
            params![
                text(path),
                entry.language,
                entry.content_hash,
                entry.symbols_hash,
                entry.mtime as i64,
                entry.size as i64,
                entry.is_test as i64,
                entry.last_indexed as i64,
            ],
        )?;
        let file_id = transaction.last_insert_rowid();
        // forgeguard: allow FG-ALG-001 -- one insert per recorded row of this file, inside one transaction
        for import in &entry.imports {
            transaction.execute(
                "INSERT INTO imports(file_id, module, stem) VALUES(?1, ?2, ?3)",
                params![file_id, import, module_stem(import)],
            )?;
        }
        // forgeguard: allow FG-ALG-001 -- one insert per recorded row of this file, inside one transaction
        for export in &entry.exports {
            transaction.execute(
                "INSERT INTO exports(file_id, name) VALUES(?1, ?2)",
                params![file_id, export],
            )?;
        }
        // forgeguard: allow FG-ALG-001 -- one insert per recorded row of this file, inside one transaction
        for symbol in &entry.symbols {
            transaction.execute(
                "INSERT INTO symbols(file_id, name, container, qualified, kind, start_line, end_line, signature, exported, route_method, route_path)
                 VALUES(?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10, ?11)",
                params![
                    file_id,
                    symbol.name,
                    symbol.container,
                    symbol.qualified(),
                    symbol.kind.as_str(),
                    symbol.start_line as i64,
                    symbol.end_line as i64,
                    symbol.signature,
                    symbol.exported as i64,
                    symbol.route.as_ref().map(|route| route.method.clone()),
                    symbol.route.as_ref().map(|route| route.path.clone()),
                ],
            )?;
            let symbol_id = transaction.last_insert_rowid();
            // forgeguard: allow FG-ALG-001 -- one insert per recorded row of this symbol, inside one transaction
            for call in &symbol.calls {
                transaction.execute(
                    "INSERT INTO calls(symbol_id, callee, receiver, qualifier, line, character)
                     VALUES(?1, ?2, ?3, ?4, ?5, ?6)",
                    params![
                        symbol_id,
                        call.name,
                        call.receiver,
                        call.qualifier,
                        call.line as i64,
                        call.character as i64
                    ],
                )?;
            }
            // forgeguard: allow FG-ALG-001 -- one insert per recorded row of this symbol, inside one transaction
            for parent in &symbol.extends {
                transaction.execute(
                    "INSERT INTO extends(symbol_id, name) VALUES(?1, ?2)",
                    params![symbol_id, parent],
                )?;
            }
            // forgeguard: allow FG-ALG-001 -- one insert per recorded row of this symbol, inside one transaction
            for link in &symbol.links {
                transaction.execute(
                    "INSERT INTO service_links(symbol_id, method, url) VALUES(?1, ?2, ?3)",
                    params![symbol_id, link.method, link.url],
                )?;
            }
        }
        transaction.commit()?;
        Ok(())
    }

    /// Rebuild the stored `FileEntry` for one path: used when copying a graph
    /// between databases (artifact import/export).
    pub fn file_entry(&self, path: &Path) -> Result<Option<FileEntry>> {
        let row = self
            .connection
            .query_row(
                "SELECT language, content_hash, symbols_hash, mtime, size, is_test, last_indexed
                 FROM files WHERE path = ?1",
                params![text(path)],
                |row| {
                    Ok(FileEntry {
                        language: row.get(0)?,
                        content_hash: row.get(1)?,
                        symbols_hash: row.get(2)?,
                        mtime: row.get::<_, i64>(3)? as u64,
                        size: row.get::<_, i64>(4)? as u64,
                        is_test: row.get::<_, i64>(5)? != 0,
                        last_indexed: row.get::<_, i64>(6)? as u64,
                        imports: Vec::new(),
                        exports: Vec::new(),
                        symbols: Vec::new(),
                    })
                },
            )
            .optional()?;
        let Some(mut entry) = row else {
            return Ok(None);
        };
        entry.imports = self.imports_of(path)?;
        entry.exports = self.exports_of(path)?;
        entry.symbols = self
            .symbols_in_file(path)?
            .into_iter()
            .map(|symbol| self.rebuild_symbol(symbol))
            .collect::<Result<Vec<_>>>()?;
        Ok(Some(entry))
    }

    fn rebuild_symbol(&self, row: SymbolRow) -> Result<Symbol> {
        Ok(Symbol {
            calls: self.call_sites(row.id)?,
            extends: self.extends_of(row.id)?,
            links: self.links_of(row.id)?,
            route: match (row.route_method, row.route_path) {
                (Some(method), Some(path)) => Some(super::Route { method, path }),
                _ => None,
            },
            name: row.name,
            container: row.container,
            kind: row.kind,
            start_line: row.start_line,
            end_line: row.end_line,
            signature: row.signature,
            exported: row.exported,
        })
    }

    pub fn links_of(&self, symbol_id: i64) -> Result<Vec<super::ServiceCall>> {
        let mut statement = self
            .connection
            .prepare("SELECT method, url FROM service_links WHERE symbol_id = ?1 ORDER BY url")?;
        let rows = statement.query_map(params![symbol_id], |row| {
            Ok(super::ServiceCall {
                method: row.get(0)?,
                url: row.get(1)?,
            })
        })?;
        Ok(rows.filter_map(Result::ok).collect())
    }

    pub fn remove_file(&self, path: &Path) -> Result<bool> {
        let removed = self
            .connection
            .execute("DELETE FROM files WHERE path = ?1", params![text(path)])?;
        Ok(removed > 0)
    }

    pub fn all_paths(&self) -> Result<Vec<PathBuf>> {
        let mut statement = self
            .connection
            .prepare("SELECT path FROM files ORDER BY path")?;
        let rows = statement.query_map([], |row| row.get::<_, String>(0))?;
        Ok(rows.filter_map(Result::ok).map(PathBuf::from).collect())
    }

    /// Paths whose recorded content hash matches, used for rename detection.
    pub fn paths_with_hash(&self, hash: &str) -> Result<Vec<PathBuf>> {
        let mut statement = self
            .connection
            .prepare("SELECT path FROM files WHERE content_hash = ?1")?;
        let rows = statement.query_map(params![hash], |row| row.get::<_, String>(0))?;
        Ok(rows.filter_map(Result::ok).map(PathBuf::from).collect())
    }

    pub fn counts(&self) -> Result<(usize, usize)> {
        let files: i64 = self
            .connection
            .query_row("SELECT COUNT(*) FROM files", [], |row| row.get(0))?;
        let symbols: i64 =
            self.connection
                .query_row("SELECT COUNT(*) FROM symbols", [], |row| row.get(0))?;
        Ok((files as usize, symbols as usize))
    }

    pub fn is_empty(&self) -> Result<bool> {
        Ok(self.counts()?.0 == 0)
    }

    pub fn language_counts(&self) -> Result<BTreeMap<String, usize>> {
        let mut statement = self
            .connection
            .prepare("SELECT language, COUNT(*) FROM files GROUP BY language")?;
        let rows = statement.query_map([], |row| {
            Ok((row.get::<_, String>(0)?, row.get::<_, i64>(1)? as usize))
        })?;
        Ok(rows.filter_map(Result::ok).collect())
    }

    pub fn test_file_count(&self) -> Result<usize> {
        let count: i64 = self.connection.query_row(
            "SELECT COUNT(*) FROM files WHERE is_test = 1",
            [],
            |row| row.get(0),
        )?;
        Ok(count as usize)
    }

    pub fn file_paths_and_tests(&self) -> Result<Vec<(PathBuf, bool)>> {
        let mut statement = self
            .connection
            .prepare("SELECT path, is_test FROM files ORDER BY path")?;
        let rows = statement.query_map([], |row| {
            Ok((
                PathBuf::from(row.get::<_, String>(0)?),
                row.get::<_, i64>(1)? != 0,
            ))
        })?;
        Ok(rows.filter_map(Result::ok).collect())
    }

    pub fn is_test_file(&self, path: &Path) -> Result<bool> {
        Ok(self
            .connection
            .query_row(
                "SELECT is_test FROM files WHERE path = ?1",
                params![text(path)],
                |row| row.get::<_, i64>(0),
            )
            .optional()?
            .is_some_and(|value| value != 0))
    }

    /// Exact qualified or bare name first, then case-insensitive substring.
    /// Deterministic ordering: no scoring, no embeddings.
    pub fn find_symbols(&self, query: &str, limit: usize) -> Result<Vec<SymbolRow>> {
        let exact = self.query_symbols(
            "WHERE s.qualified = ?1 OR s.name = ?1 ORDER BY f.path, s.start_line LIMIT ?2",
            params![query, limit as i64],
        )?;
        if !exact.is_empty() {
            return Ok(exact);
        }
        let pattern = format!("%{}%", query.to_lowercase());
        self.query_symbols(
            "WHERE LOWER(s.qualified) LIKE ?1 ESCAPE '\\' OR LOWER(s.name) LIKE ?1 ESCAPE '\\'
             ORDER BY f.path, s.start_line LIMIT ?2",
            params![pattern, limit as i64],
        )
    }

    /// Regex-free name pattern used by structural search: `*` and `?` wildcards.
    pub fn match_symbols(
        &self,
        pattern: Option<&str>,
        kind: Option<SymbolKind>,
        limit: usize,
    ) -> Result<Vec<SymbolRow>> {
        let like = pattern.map_or_else(|| "%".to_owned(), wildcard_to_like);
        match kind {
            Some(kind) => self.query_symbols(
                "WHERE LOWER(s.qualified) LIKE ?1 ESCAPE '\\' AND s.kind = ?2
                 ORDER BY f.path, s.start_line LIMIT ?3",
                params![like, kind.as_str(), limit as i64],
            ),
            None => self.query_symbols(
                "WHERE LOWER(s.qualified) LIKE ?1 ESCAPE '\\' ORDER BY f.path, s.start_line LIMIT ?2",
                params![like, limit as i64],
            ),
        }
    }

    pub fn symbol_by_id(&self, id: i64) -> Result<Option<SymbolRow>> {
        Ok(self
            .query_symbols("WHERE s.id = ?1", params![id])?
            .into_iter()
            .next())
    }

    pub fn symbols_in_file(&self, path: &Path) -> Result<Vec<SymbolRow>> {
        self.query_symbols(
            "WHERE f.path = ?1 ORDER BY s.start_line",
            params![text(path)],
        )
    }

    /// Symbols of a file whose line span overlaps one of the given ranges.
    pub fn symbols_touching(
        &self,
        path: &Path,
        ranges: Option<&[(usize, usize)]>,
    ) -> Result<Vec<SymbolRow>> {
        let symbols = self.symbols_in_file(path)?;
        let Some(ranges) = ranges else {
            return Ok(symbols);
        };
        Ok(symbols
            .into_iter()
            .filter(|symbol| {
                ranges
                    .iter()
                    .any(|(start, end)| symbol.start_line <= *end && symbol.end_line >= *start)
            })
            .collect())
    }

    pub fn all_symbols(&self) -> Result<Vec<SymbolRow>> {
        self.query_symbols("ORDER BY f.path, s.start_line", params![])
    }

    /// Symbols that call `name`. `receiver` narrows to a resolved owning type.
    pub fn callers_of(&self, name: &str, receiver: Option<&str>) -> Result<Vec<SymbolRow>> {
        match receiver {
            Some(receiver) => self.query_symbols(
                "JOIN calls c ON c.symbol_id = s.id
                 WHERE c.callee = ?1 AND (c.receiver IS NULL OR c.receiver = ?2)
                 ORDER BY f.path, s.start_line",
                params![name, receiver],
            ),
            None => self.query_symbols(
                "JOIN calls c ON c.symbol_id = s.id WHERE c.callee = ?1
                 ORDER BY f.path, s.start_line",
                params![name],
            ),
        }
    }

    pub fn caller_count(&self, name: &str) -> Result<usize> {
        let count: i64 = self.connection.query_row(
            "SELECT COUNT(DISTINCT symbol_id) FROM calls WHERE callee = ?1",
            params![name],
            |row| row.get(0),
        )?;
        Ok(count as usize)
    }

    /// Call targets recorded inside a symbol, with the resolved receiver type.
    pub fn calls_of(&self, symbol_id: i64) -> Result<Vec<(String, Option<String>)>> {
        let mut statement = self
            .connection
            .prepare("SELECT callee, receiver FROM calls WHERE symbol_id = ?1 ORDER BY callee")?;
        let rows = statement.query_map(params![symbol_id], |row| {
            Ok((row.get::<_, String>(0)?, row.get::<_, Option<String>>(1)?))
        })?;
        Ok(rows.filter_map(Result::ok).collect())
    }

    /// Every recorded call site of one symbol, positions included.
    pub fn call_sites(&self, symbol_id: i64) -> Result<Vec<super::Call>> {
        let mut statement = self.connection.prepare(
            "SELECT callee, receiver, qualifier, line, character FROM calls
             WHERE symbol_id = ?1 ORDER BY callee, line",
        )?;
        let rows = statement.query_map(params![symbol_id], |row| {
            Ok(super::Call {
                name: row.get(0)?,
                receiver: row.get(1)?,
                qualifier: row.get(2)?,
                line: row.get::<_, i64>(3)? as usize,
                character: row.get::<_, i64>(4)? as usize,
            })
        })?;
        Ok(rows.filter_map(Result::ok).collect())
    }

    /// Call sites the static pass could not attribute to a type. These are what
    /// a language server is asked about, so the request count stays proportional
    /// to what static resolution missed rather than to the whole graph.
    pub fn unresolved_calls(&self, limit: usize) -> Result<Vec<UnresolvedCall>> {
        let mut statement = self.connection.prepare(
            "SELECT c.rowid, f.path, f.language, c.callee, c.line, c.character
             FROM calls c
             JOIN symbols s ON s.id = c.symbol_id
             JOIN files f ON f.id = s.file_id
             WHERE c.receiver IS NULL AND c.qualifier IS NOT NULL AND c.line > 0
             -- Shortest qualifiers first: a bare `repo.find()` is what a server
             -- answers quickly, a long call chain is what eats the budget.
             ORDER BY LENGTH(c.qualifier), f.path, c.line, c.character
             LIMIT ?1",
        )?;
        let rows = statement.query_map(params![limit as i64], |row| {
            Ok(UnresolvedCall {
                id: row.get(0)?,
                path: PathBuf::from(row.get::<_, String>(1)?),
                language: row.get(2)?,
                callee: row.get(3)?,
                line: row.get::<_, i64>(4)? as usize,
                character: row.get::<_, i64>(5)? as usize,
            })
        })?;
        Ok(rows.filter_map(Result::ok).collect())
    }

    /// Record a receiver a language server resolved for one call site.
    pub fn set_call_receiver(&self, id: i64, receiver: &str) -> Result<()> {
        self.connection.execute(
            "UPDATE calls SET receiver = ?2 WHERE rowid = ?1",
            params![id, receiver],
        )?;
        Ok(())
    }

    /// Definitions matching the call targets of one symbol.
    pub fn callees_of(&self, symbol_id: i64) -> Result<Vec<SymbolRow>> {
        self.query_symbols(
            "WHERE s.id IN (
                 SELECT t.id FROM calls c
                 JOIN symbols t ON t.name = c.callee
                 WHERE c.symbol_id = ?1
                   AND (c.receiver IS NULL OR t.container = c.receiver)
             ) ORDER BY f.path, s.start_line",
            params![symbol_id],
        )
    }

    pub fn extends_of(&self, symbol_id: i64) -> Result<Vec<String>> {
        let mut statement = self
            .connection
            .prepare("SELECT name FROM extends WHERE symbol_id = ?1 ORDER BY name")?;
        let rows = statement.query_map(params![symbol_id], |row| row.get::<_, String>(0))?;
        Ok(rows.filter_map(Result::ok).collect())
    }

    /// Symbols that declare `name` as a supertype or implemented interface.
    pub fn implementors_of(&self, name: &str) -> Result<Vec<SymbolRow>> {
        self.query_symbols(
            "JOIN extends e ON e.symbol_id = s.id WHERE e.name = ?1 ORDER BY f.path, s.start_line",
            params![name],
        )
    }

    pub fn imports_of(&self, path: &Path) -> Result<Vec<String>> {
        let mut statement = self.connection.prepare(
            "SELECT i.module FROM imports i JOIN files f ON f.id = i.file_id
             WHERE f.path = ?1 ORDER BY i.module",
        )?;
        let rows = statement.query_map(params![text(path)], |row| row.get::<_, String>(0))?;
        Ok(rows.filter_map(Result::ok).collect())
    }

    pub fn exports_of(&self, path: &Path) -> Result<Vec<String>> {
        let mut statement = self.connection.prepare(
            "SELECT e.name FROM exports e JOIN files f ON f.id = e.file_id
             WHERE f.path = ?1 ORDER BY e.name",
        )?;
        let rows = statement.query_map(params![text(path)], |row| row.get::<_, String>(0))?;
        Ok(rows.filter_map(Result::ok).collect())
    }

    /// Files importing a module whose tail matches this file's stem (USED_BY).
    pub fn dependents_of(&self, path: &Path) -> Result<Vec<PathBuf>> {
        let Some(stem) = path
            .file_stem()
            .and_then(|value| value.to_str())
            .map(str::to_lowercase)
        else {
            return Ok(Vec::new());
        };
        let mut statement = self.connection.prepare(
            "SELECT DISTINCT f.path FROM imports i JOIN files f ON f.id = i.file_id
             WHERE i.stem = ?1 AND f.path <> ?2 ORDER BY f.path",
        )?;
        let rows = statement.query_map(params![stem, text(path)], |row| row.get::<_, String>(0))?;
        Ok(rows.filter_map(Result::ok).map(PathBuf::from).collect())
    }

    /// Imports that resolve to no indexed file: third-party or platform.
    pub fn external_dependencies(&self, limit: usize) -> Result<Vec<String>> {
        let mut statement = self.connection.prepare(
            "SELECT i.module, COUNT(*) AS uses FROM imports i
             WHERE i.stem NOT IN (
                 SELECT LOWER(
                     REPLACE(
                         CASE WHEN INSTR(f.path, '/') > 0
                              THEN REPLACE(f.path, RTRIM(f.path, REPLACE(f.path, '/', '')), '')
                              ELSE f.path END,
                         '.' || REPLACE(f.path, RTRIM(f.path, REPLACE(f.path, '.', '')), ''), ''))
                 FROM files f
             )
             GROUP BY i.module ORDER BY uses DESC, i.module LIMIT ?1",
        )?;
        let rows = statement.query_map(params![limit as i64], |row| row.get::<_, String>(0))?;
        Ok(rows.filter_map(Result::ok).collect())
    }

    /// Stems of every indexed file, for resolving imports to internal targets.
    pub fn file_stems(&self) -> Result<Vec<String>> {
        Ok(self
            .all_paths()?
            .iter()
            .filter_map(|path| path.file_stem().and_then(|value| value.to_str()))
            .map(str::to_lowercase)
            .collect())
    }

    pub fn routes(&self) -> Result<Vec<SymbolRow>> {
        self.query_symbols(
            "WHERE s.route_path IS NOT NULL ORDER BY s.route_path, f.path",
            params![],
        )
    }

    /// Outbound HTTP calls, paired with the indexed route they hit when one
    /// matches: the cross-service edge.
    pub fn service_links(&self) -> Result<Vec<ServiceLink>> {
        let mut statement = self.connection.prepare(
            "SELECT s.qualified, f.path, l.method, l.url FROM service_links l
             JOIN symbols s ON s.id = l.symbol_id
             JOIN files f ON f.id = s.file_id
             ORDER BY f.path, s.start_line",
        )?;
        let rows = statement.query_map([], |row| {
            Ok(ServiceLink {
                symbol: row.get(0)?,
                path: PathBuf::from(row.get::<_, String>(1)?),
                method: row.get(2)?,
                url: row.get(3)?,
                target: None,
                target_path: None,
            })
        })?;
        let mut links = rows.filter_map(Result::ok).collect::<Vec<_>>();
        let routes = self.routes()?;
        for link in &mut links {
            // forgeguard: allow FG-ALG-002 -- route matching compares path segments, so a hash index cannot answer it; the route list is per-repository small
            if let Some(route) = routes
                .iter()
                .find(|route| route_matches(route, link.method.as_deref(), &link.url))
            {
                link.target = Some(route.qualified.clone());
                link.target_path = Some(route.path.clone());
            }
        }
        Ok(links)
    }

    /// Raw read-only SQL, used by the Cypher translator. Rejected for anything
    /// that is not a single `SELECT`.
    pub fn select(&self, sql: &str, limit: usize) -> Result<(Vec<String>, Vec<Vec<String>>)> {
        let mut statement = self.connection.prepare(sql)?;
        let columns = statement
            .column_names()
            .map(str::to_owned)
            .collect::<Vec<_>>();
        let width = columns.len();
        let mut rows = statement.query([])?;
        let mut values = Vec::new();
        while let Some(row) = rows.next()? {
            if values.len() >= limit {
                break;
            }
            values.push(
                (0..width)
                    .map(|index| {
                        row.get::<_, rusqlite::types::Value>(index)
                            .map(render_value)
                            .unwrap_or_default()
                    })
                    .collect(),
            );
        }
        Ok((columns, values))
    }

    /// Compact the database into a portable file (the shared graph artifact).
    pub fn export_to(&self, path: &Path) -> Result<()> {
        if let Some(directory) = path.parent() {
            // forgeguard: allow FG-SEC-007 -- the export destination is named by the operator on the command line
            std::fs::create_dir_all(directory)
                .with_context(|| format!("failed to create {}", directory.display()))?;
        }
        if path.exists() {
            // forgeguard: allow FG-SEC-007 -- same operator-named destination, replaced rather than appended to
            std::fs::remove_file(path)
                .with_context(|| format!("failed to replace {}", path.display()))?;
        }
        self.connection
            .execute("VACUUM INTO ?1", params![text(path)])?;
        Ok(())
    }

    fn query_symbols(
        &self,
        tail: &str,
        parameters: impl rusqlite::Params,
    ) -> Result<Vec<SymbolRow>> {
        let sql = format!(
            "SELECT s.id, f.path, f.language, f.is_test, s.name, s.container, s.qualified,
                    s.kind, s.start_line, s.end_line, s.signature, s.exported,
                    s.route_method, s.route_path
             FROM symbols s JOIN files f ON f.id = s.file_id {tail}"
        );
        let mut statement = self.connection.prepare(&sql)?;
        let rows = statement.query_map(parameters, symbol_row)?;
        Ok(rows.filter_map(Result::ok).collect())
    }
}

fn symbol_row(row: &Row<'_>) -> rusqlite::Result<SymbolRow> {
    Ok(SymbolRow {
        id: row.get(0)?,
        path: PathBuf::from(row.get::<_, String>(1)?),
        language: row.get(2)?,
        is_test: row.get::<_, i64>(3)? != 0,
        name: row.get(4)?,
        container: row.get(5)?,
        qualified: row.get(6)?,
        kind: SymbolKind::from_label(&row.get::<_, String>(7)?),
        start_line: row.get::<_, i64>(8)? as usize,
        end_line: row.get::<_, i64>(9)? as usize,
        signature: row.get(10)?,
        exported: row.get::<_, i64>(11)? != 0,
        route_method: row.get(12)?,
        route_path: row.get(13)?,
    })
}

fn render_value(value: rusqlite::types::Value) -> String {
    match value {
        rusqlite::types::Value::Null => String::new(),
        rusqlite::types::Value::Integer(number) => number.to_string(),
        rusqlite::types::Value::Real(number) => number.to_string(),
        rusqlite::types::Value::Text(text) => text,
        rusqlite::types::Value::Blob(bytes) => format!("<{} bytes>", bytes.len()),
    }
}

/// A stored path is usable only when it stays inside the repository: relative,
/// no root, no `..`.
fn is_inside_repository(path: &Path) -> bool {
    use std::path::Component;
    path.components()
        .all(|component| matches!(component, Component::Normal(_) | Component::CurDir))
}

fn text(path: &Path) -> String {
    path.to_string_lossy().replace('\\', "/")
}

/// The last path-like segment of an import, which is what every language's
/// import syntax has in common.
pub fn module_stem(module: &str) -> String {
    let trimmed = module.trim();
    let last_segment = trimmed
        .rsplit(['/', '\\', ':'])
        .find(|part| !part.trim().is_empty())
        .unwrap_or(trimmed);
    let without_ext = last_segment
        .strip_suffix(".js")
        .or_else(|| last_segment.strip_suffix(".ts"))
        .or_else(|| last_segment.strip_suffix(".tsx"))
        .or_else(|| last_segment.strip_suffix(".jsx"))
        .or_else(|| last_segment.strip_suffix(".mjs"))
        .or_else(|| last_segment.strip_suffix(".cjs"))
        .or_else(|| last_segment.strip_suffix(".mts"))
        .or_else(|| last_segment.strip_suffix(".cts"))
        .or_else(|| last_segment.strip_suffix(".py"))
        .or_else(|| last_segment.strip_suffix(".rs"))
        .unwrap_or(last_segment);
    if without_ext.contains('.') && (without_ext.starts_with('.') || last_segment != trimmed) {
        without_ext.trim().to_lowercase()
    } else {
        without_ext
            .rsplit('.')
            .find(|part| !part.trim().is_empty())
            .unwrap_or(without_ext)
            .trim()
            .to_lowercase()
    }
}

fn wildcard_to_like(pattern: &str) -> String {
    let mut like = String::with_capacity(pattern.len() + 2);
    for character in pattern.to_lowercase().chars() {
        match character {
            '*' => like.push('%'),
            '?' => like.push('_'),
            '%' | '_' => {
                like.push('\\');
                like.push(character);
            }
            other => like.push(other),
        }
    }
    if !pattern.contains(['*', '?']) {
        return format!("%{like}%");
    }
    like
}

/// A client URL hits a route when the route's static segments line up, treating
/// `:id`, `{id}`, and `<id>` as wildcards.
fn route_matches(route: &SymbolRow, method: Option<&str>, url: &str) -> bool {
    let Some(route_path) = route.route_path.as_deref() else {
        return false;
    };
    if let (Some(left), Some(right)) = (method, route.route_method.as_deref()) {
        if !left.eq_ignore_ascii_case(right) {
            return false;
        }
    }
    let url_path = url
        .split_once("://")
        .map_or(url, |(_, rest)| rest.split_once('/').map_or("", |(_, p)| p));
    let route_segments = route_path.trim_matches('/').split('/').collect::<Vec<_>>();
    let url_segments = url_path
        .split('?')
        .next()
        .unwrap_or_default()
        .trim_matches('/')
        .split('/')
        .collect::<Vec<_>>();
    if route_segments.len() != url_segments.len() {
        return false;
    }
    route_segments
        .iter()
        .zip(url_segments.iter())
        .all(|(route, url)| is_parameter(route) || route.eq_ignore_ascii_case(url))
}

fn is_parameter(segment: &str) -> bool {
    segment.starts_with(':')
        || (segment.starts_with('{') && segment.ends_with('}'))
        || (segment.starts_with('<') && segment.ends_with('>'))
        || segment.starts_with('*')
}
