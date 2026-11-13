//! The library index: one SQLite database with an FTS5 full-text index.
//!
//! `library.sqlite` holds every entry with its search fields, the full
//! definitions of symbols and footprints (compressed, content-addressed in
//! `blobs`), the duplicate groups and their decisions. It is built in one
//! go from the sources ([`Library::build`]) and then only read, except for
//! review decisions, which also go to `decisions.json`.

use std::collections::BTreeMap;
use std::io::{Read as _, Write as _};
use std::path::{Path, PathBuf};
use std::time::Instant;

use flate2::Compression;
use flate2::read::DeflateDecoder;
use flate2::write::DeflateEncoder;
use rusqlite::{Connection, OpenFlags, OptionalExtension, params};
use serde::{Deserialize, Serialize};

use crate::dedup::{self, Choice, Class, Decision, Decisions, GroupReport, KindSummary, Report};
use crate::entry::{Entry, Kind};
use crate::error::LibraryError;
use crate::fingerprint;

/// File name of the index.
pub const INDEX_FILE: &str = "library.sqlite";

/// Version of the index layout.
pub const FORMAT: u32 = 1;

const SCHEMA: &str = "
CREATE TABLE meta(key TEXT PRIMARY KEY, value TEXT NOT NULL) WITHOUT ROWID;
CREATE TABLE sources(
  id INTEGER PRIMARY KEY, name TEXT NOT NULL, version TEXT NOT NULL,
  license TEXT NOT NULL, url TEXT NOT NULL);
CREATE TABLE items(
  id INTEGER PRIMARY KEY, kind INTEGER NOT NULL, source INTEGER NOT NULL,
  library TEXT NOT NULL, name TEXT NOT NULL, description TEXT NOT NULL,
  keywords TEXT NOT NULL, value TEXT NOT NULL, package TEXT NOT NULL,
  mpn TEXT NOT NULL, manufacturer TEXT NOT NULL, datasheet TEXT NOT NULL,
  params TEXT NOT NULL, pins INTEGER NOT NULL, units INTEGER NOT NULL,
  mounting TEXT, width REAL, depth REAL, height REAL, model TEXT, file TEXT,
  mesh TEXT, data INTEGER, canonical INTEGER);
CREATE UNIQUE INDEX items_name ON items(kind, source, library, name);
CREATE INDEX items_browse ON items(kind, library, name);
CREATE INDEX items_model ON items(model);
CREATE TABLE blobs(id INTEGER PRIMARY KEY, hash TEXT NOT NULL UNIQUE, data BLOB NOT NULL);
CREATE TABLE groups(
  id INTEGER PRIMARY KEY, key TEXT NOT NULL UNIQUE, kind INTEGER NOT NULL,
  class TEXT NOT NULL, reason TEXT NOT NULL, decision TEXT NOT NULL,
  canonical INTEGER NOT NULL);
CREATE TABLE members(
  group_id INTEGER NOT NULL, item INTEGER NOT NULL, rank INTEGER NOT NULL,
  PRIMARY KEY(group_id, item)) WITHOUT ROWID;
CREATE INDEX members_item ON members(item);
CREATE VIRTUAL TABLE search USING fts5(
  name, library, description, keywords, value, package, mpn, params, aliases,
  content='', tokenize='unicode61 remove_diacritics 2', prefix='2 3');
";

/// Weights of the search columns (name … aliases) for ranking.
const WEIGHTS: &str = "10.0, 2.0, 1.0, 3.0, 5.0, 4.0, 8.0, 0.5, 2.0";

/// Where an index came from.
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct SourceInfo {
    /// Name (`KiCad`).
    pub name: String,
    /// Version of the libraries (`10.0.0`).
    pub version: String,
    /// License of the library data.
    pub license: String,
    /// Where the libraries come from.
    pub url: String,
}

/// An open library index.
pub struct Library {
    conn: Connection,
    dir: PathBuf,
}

/// A search request.
#[derive(Debug, Clone, Default, Deserialize)]
#[serde(rename_all = "camelCase", default)]
pub struct Query {
    /// Search text (empty: browse by library and name).
    pub text: String,
    /// Only this kind.
    pub kind: Option<Kind>,
    /// Only these libraries.
    pub libraries: Vec<String>,
    /// Only footprints with this mounting (`smd`, `tht`, `other`).
    pub mounting: Option<String>,
    /// Only footprints with a 3D model.
    pub with_model: bool,
    /// Only entries in a duplicate group (of any class).
    pub duplicates: bool,
    /// Also entries merged into another one.
    pub include_merged: bool,
    /// First result.
    pub offset: u32,
    /// Number of results (at most 500).
    pub limit: u32,
}

/// One search result.
#[derive(Debug, Clone, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Summary {
    /// Index id.
    pub id: i64,
    /// Kind.
    pub kind: Kind,
    /// Library.
    pub library: String,
    /// Name.
    pub name: String,
    /// Description.
    pub description: String,
    /// Value (symbols).
    pub value: String,
    /// Package.
    pub package: String,
    /// Pins or pads.
    pub pins: u32,
    /// Mounting of footprints.
    pub mounting: Option<String>,
    /// The entry this one is merged into.
    pub merged_into: Option<i64>,
    /// Number of duplicate groups the entry belongs to.
    pub groups: u32,
}

/// Search results.
#[derive(Debug, Clone, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct SearchResult {
    /// Matching entries in total.
    pub total: u64,
    /// The requested page.
    pub items: Vec<Summary>,
    /// Time the search took, in milliseconds.
    pub elapsed_ms: f64,
}

/// A library with its number of entries.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Facet {
    /// Library name.
    pub library: String,
    /// Visible entries.
    pub count: u64,
}

/// A member of a duplicate group.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Member {
    /// Index id.
    pub id: i64,
    /// `library:name`.
    pub name: String,
}

/// A duplicate group.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct GroupInfo {
    /// Stable key.
    pub key: String,
    /// Kind.
    pub kind: Kind,
    /// Class.
    pub class: Class,
    /// Why the members belong together.
    pub reason: String,
    /// Decision.
    pub decision: Decision,
    /// The entry that stays visible when merged.
    pub canonical: i64,
    /// Members, best first.
    pub members: Vec<Member>,
}

/// A page of duplicate groups.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct GroupPage {
    /// Matching groups in total.
    pub total: u64,
    /// The requested page.
    pub groups: Vec<GroupInfo>,
}

/// Everything about one entry.
#[derive(Debug, Clone, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Details {
    /// The search fields.
    #[serde(flatten)]
    pub summary: Summary,
    /// Search keywords.
    pub keywords: String,
    /// Manufacturer part number.
    pub mpn: String,
    /// Manufacturer.
    pub manufacturer: String,
    /// Datasheet link.
    pub datasheet: String,
    /// Other parameters.
    pub params: String,
    /// Units of a symbol.
    pub units: u32,
    /// Size in mm.
    pub size: Option<[f64; 3]>,
    /// Model key of a footprint's 3D model.
    pub model: Option<String>,
    /// The index id of that model, if the library has it.
    pub model_id: Option<i64>,
    /// Source file of a model, relative to the source's model folder.
    pub file: Option<String>,
    /// The source file on this computer, if it exists (models of libraries
    /// built from local folders).
    pub source_file: Option<String>,
    /// Mesh (GLB content hash) of a model.
    pub mesh: Option<String>,
    /// Entries merged into this one (`library:name`).
    pub aliases: Vec<String>,
    /// Duplicate groups of the entry.
    pub duplicate_groups: Vec<GroupInfo>,
    /// The source.
    pub source: SourceInfo,
    /// The full definition (symbols and footprints).
    pub definition: Option<serde_json::Value>,
}

/// Numbers of the index.
#[derive(Debug, Clone, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Status {
    /// Folder of the index.
    pub path: String,
    /// Index format.
    pub format: u32,
    /// Sources.
    pub sources: Vec<SourceInfo>,
    /// Entries by kind (`symbol`, `footprint`, `model`).
    pub counts: BTreeMap<String, u64>,
    /// Visible entries by kind.
    pub visible: BTreeMap<String, u64>,
    /// Duplicate groups by class.
    pub groups: BTreeMap<String, u64>,
    /// Near-duplicate groups waiting for a review.
    pub pending: u64,
    /// Models with a mesh.
    pub meshes: u64,
    /// Size of the index file in bytes.
    pub bytes: u64,
    /// The library release the index came from (`library-…`), if it was
    /// downloaded.
    pub release: Option<String>,
}

#[allow(clippy::needless_pass_by_value, reason = "used with map_err")]
fn sql_error(error: rusqlite::Error) -> LibraryError {
    LibraryError::Sqlite(error.to_string())
}

fn compress(text: &str) -> Vec<u8> {
    let mut encoder = DeflateEncoder::new(Vec::new(), Compression::new(6));
    let _ = encoder.write_all(text.as_bytes());
    encoder.finish().unwrap_or_default()
}

fn decompress(data: &[u8]) -> Option<String> {
    let mut out = String::new();
    DeflateDecoder::new(data).read_to_string(&mut out).ok()?;
    Some(out)
}

/// Turns user input into an FTS5 query: every word must match the start of
/// a token; words with punctuation (`SOT-23`) become phrases.
#[must_use]
pub fn fts_query(text: &str) -> Option<String> {
    let mut parts = Vec::new();
    for word in text.split_whitespace() {
        let tokens: Vec<String> = word
            .split(|c: char| !c.is_alphanumeric())
            .filter(|t| !t.is_empty())
            .map(str::to_lowercase)
            .collect();
        if !tokens.is_empty() {
            parts.push(format!("\"{}\"*", tokens.join(" ")));
        }
    }
    (!parts.is_empty()).then(|| parts.join(" "))
}

impl Library {
    /// Builds a new index in `dir` from `entries` (replacing an old one) and
    /// returns it with the deduplication report. Decisions from
    /// `decisions.json` in `dir` are applied.
    ///
    /// # Errors
    ///
    /// Fails if the index cannot be written.
    pub fn build(
        dir: &Path,
        sources: &[SourceInfo],
        mut entries: Vec<Entry>,
        tolerance: i64,
    ) -> Result<(Self, Report), LibraryError> {
        std::fs::create_dir_all(dir).map_err(|source| LibraryError::Io {
            path: dir.to_path_buf(),
            source,
        })?;
        entries.sort_by(|a, b| {
            (a.kind, a.source, &a.library, &a.name).cmp(&(b.kind, b.source, &b.library, &b.name))
        });
        entries.dedup_by(|a, b| {
            a.kind == b.kind && a.source == b.source && a.library == b.library && a.name == b.name
        });
        let groups = dedup::find(&entries, tolerance);
        let temp = dir.join(format!("{INDEX_FILE}.tmp"));
        let _ = std::fs::remove_file(&temp);
        let mut conn = Connection::open(&temp).map_err(sql_error)?;
        conn.execute_batch(
            "PRAGMA journal_mode = OFF; PRAGMA synchronous = OFF; PRAGMA page_size = 4096;",
        )
        .map_err(sql_error)?;
        conn.execute_batch(SCHEMA).map_err(sql_error)?;
        let tx = conn.transaction().map_err(sql_error)?;
        {
            let mut meta = tx
                .prepare("INSERT INTO meta(key, value) VALUES (?1, ?2)")
                .map_err(sql_error)?;
            meta.execute(params!["format", FORMAT.to_string()])
                .map_err(sql_error)?;
            meta.execute(params!["tolerance", tolerance.to_string()])
                .map_err(sql_error)?;
            let mut source = tx
                .prepare("INSERT INTO sources(id, name, version, license, url) VALUES (?1, ?2, ?3, ?4, ?5)")
                .map_err(sql_error)?;
            for (id, info) in sources.iter().enumerate() {
                source
                    .execute(params![
                        id as i64,
                        info.name,
                        info.version,
                        info.license,
                        info.url
                    ])
                    .map_err(sql_error)?;
            }
            insert_entries(&tx, &entries, &groups)?;
            insert_groups(&tx, &entries, &groups)?;
        }
        tx.commit().map_err(sql_error)?;
        conn.execute_batch("INSERT INTO search(search) VALUES('optimize'); VACUUM;")
            .map_err(sql_error)?;
        drop(conn);
        let path = dir.join(INDEX_FILE);
        std::fs::rename(&temp, &path).map_err(|source| LibraryError::Io {
            path: path.clone(),
            source,
        })?;
        let library = Self::open(dir)?;
        let decisions = Decisions::load(dir)?;
        library.apply(&decisions)?;
        let report = library.report()?;
        Ok((library, report))
    }

    /// Opens the index in `dir`.
    ///
    /// # Errors
    ///
    /// Fails if there is no index or it has another format.
    pub fn open(dir: &Path) -> Result<Self, LibraryError> {
        let path = dir.join(INDEX_FILE);
        if !path.is_file() {
            return Err(LibraryError::NotFound(path.display().to_string()));
        }
        let conn = Connection::open_with_flags(
            &path,
            OpenFlags::SQLITE_OPEN_READ_WRITE | OpenFlags::SQLITE_OPEN_NO_MUTEX,
        )
        .map_err(sql_error)?;
        let format: Option<String> = conn
            .query_row("SELECT value FROM meta WHERE key = 'format'", [], |row| {
                row.get(0)
            })
            .optional()
            .map_err(sql_error)?;
        if format.as_deref() != Some(&FORMAT.to_string()) {
            return Err(LibraryError::Format(format!(
                "{}: index format {} is not supported (expected {FORMAT})",
                path.display(),
                format.unwrap_or_default()
            )));
        }
        conn.execute_batch("PRAGMA journal_mode = DELETE; PRAGMA cache_size = -32000;")
            .map_err(sql_error)?;
        Ok(Self {
            conn,
            dir: dir.to_path_buf(),
        })
    }

    /// The folder of the index.
    #[must_use]
    pub fn dir(&self) -> &Path {
        &self.dir
    }

    /// Applies decisions: sets the decision and the canonical entry of every
    /// group, and which entries are merged into which.
    fn apply(&self, decisions: &Decisions) -> Result<(), LibraryError> {
        let conn = &self.conn;
        conn.execute_batch("BEGIN; UPDATE items SET canonical = NULL;")
            .map_err(sql_error)?;
        let groups: Vec<(i64, String, String)> = conn
            .prepare("SELECT id, key, class FROM groups ORDER BY id")
            .and_then(|mut stmt| {
                stmt.query_map([], |row| Ok((row.get(0)?, row.get(1)?, row.get(2)?)))?
                    .collect()
            })
            .map_err(sql_error)?;
        let mut member_stmt = conn
            .prepare(
                "SELECT m.item, i.library || ':' || i.name FROM members m
                 JOIN items i ON i.id = m.item WHERE m.group_id = ?1 ORDER BY m.rank",
            )
            .map_err(sql_error)?;
        for (id, key, class) in groups {
            let class = Class::parse(&class).unwrap_or(Class::Near);
            let choice = decisions.groups.get(&key);
            let decision = choice.map_or_else(|| class.default_decision(), |c| c.decision);
            let members: Vec<(i64, String)> = member_stmt
                .query_map([id], |row| Ok((row.get(0)?, row.get(1)?)))
                .and_then(Iterator::collect)
                .map_err(sql_error)?;
            let canonical = choice
                .and_then(|c| c.canonical.as_ref())
                .and_then(|name| members.iter().find(|(_, n)| n == name))
                .or_else(|| members.first())
                .map_or(0, |(item, _)| *item);
            conn.execute(
                "UPDATE groups SET decision = ?2, canonical = ?3 WHERE id = ?1",
                params![id, decision.as_str(), canonical],
            )
            .map_err(sql_error)?;
            if decision == Decision::Merge {
                // Merge into the canonical entry, or into the entry it was
                // merged into by an earlier group.
                let target: i64 = conn
                    .query_row(
                        "SELECT coalesce(canonical, id) FROM items WHERE id = ?1",
                        [canonical],
                        |row| row.get(0),
                    )
                    .map_err(sql_error)?;
                for (item, _) in &members {
                    if *item != target {
                        conn.execute(
                            "UPDATE items SET canonical = ?2 WHERE id = ?1 AND canonical IS NULL",
                            params![item, target],
                        )
                        .map_err(sql_error)?;
                    }
                }
            }
        }
        conn.execute_batch("COMMIT;").map_err(sql_error)
    }

    /// Records a review decision (also in `decisions.json`) and applies it.
    ///
    /// # Errors
    ///
    /// Fails if there is no such group or the decision cannot be saved.
    pub fn decide(
        &self,
        key: &str,
        decision: Decision,
        canonical: Option<i64>,
    ) -> Result<GroupInfo, LibraryError> {
        let group = self
            .group(key)?
            .ok_or_else(|| LibraryError::NotFound(format!("duplicate group {key}")))?;
        let canonical_name = match canonical {
            Some(id) => Some(
                group
                    .members
                    .iter()
                    .find(|m| m.id == id)
                    .map(|m| m.name.clone())
                    .ok_or_else(|| LibraryError::NotFound(format!("entry {id} in group {key}")))?,
            ),
            None => None,
        };
        let mut decisions = Decisions::load(&self.dir)?;
        decisions.version = 1;
        decisions.groups.insert(
            key.to_owned(),
            Choice {
                decision,
                canonical: canonical_name,
            },
        );
        decisions.save(&self.dir)?;
        self.apply(&decisions)?;
        self.group(key)?
            .ok_or_else(|| LibraryError::NotFound(format!("duplicate group {key}")))
    }

    /// Searches the index.
    ///
    /// # Errors
    ///
    /// Fails on database errors.
    pub fn search(&self, query: &Query) -> Result<SearchResult, LibraryError> {
        let started = Instant::now();
        let limit = query.limit.clamp(1, 500);
        let mut filters = Vec::new();
        let mut values: Vec<rusqlite::types::Value> = Vec::new();
        if let Some(kind) = query.kind {
            values.push(kind.code().into());
            filters.push(format!("i.kind = ?{}", values.len()));
        }
        if !query.libraries.is_empty() {
            let mut slots = Vec::new();
            for library in &query.libraries {
                values.push(library.clone().into());
                slots.push(format!("?{}", values.len()));
            }
            filters.push(format!("i.library IN ({})", slots.join(", ")));
        }
        if let Some(mounting) = &query.mounting {
            values.push(mounting.clone().into());
            filters.push(format!("i.mounting = ?{}", values.len()));
        }
        if query.with_model {
            filters.push("i.model IS NOT NULL".to_owned());
        }
        if query.duplicates {
            filters.push("EXISTS (SELECT 1 FROM members m WHERE m.item = i.id)".to_owned());
        }
        if !query.include_merged {
            filters.push("i.canonical IS NULL".to_owned());
        }
        let columns = "i.id, i.kind, i.library, i.name, i.description, i.value, i.package, i.pins, \
                       i.mounting, i.canonical, (SELECT count(*) FROM members m WHERE m.item = i.id)";
        let (from, order) = match fts_query(&query.text) {
            Some(fts) => {
                values.push(fts.into());
                filters.push(format!("search MATCH ?{}", values.len()));
                (
                    "search JOIN items i ON i.id = search.rowid",
                    format!("bm25(search, {WEIGHTS}), length(i.name), i.name, i.id"),
                )
            }
            None => ("items i", "i.library, i.name, i.id".to_owned()),
        };
        let filter = if filters.is_empty() {
            String::new()
        } else {
            format!("WHERE {}", filters.join(" AND "))
        };
        let total: i64 = self
            .conn
            .prepare_cached(&format!("SELECT count(*) FROM {from} {filter}"))
            .and_then(|mut stmt| {
                stmt.query_row(rusqlite::params_from_iter(values.iter()), |row| row.get(0))
            })
            .map_err(sql_error)?;
        let sql = format!(
            "SELECT {columns} FROM {from} {filter} ORDER BY {order} LIMIT {limit} OFFSET {}",
            query.offset
        );
        let items = self
            .conn
            .prepare_cached(&sql)
            .and_then(|mut stmt| {
                stmt.query_map(rusqlite::params_from_iter(values.iter()), summary_row)?
                    .collect::<Result<Vec<_>, _>>()
            })
            .map_err(sql_error)?;
        Ok(SearchResult {
            total: u64::try_from(total).unwrap_or(0),
            items,
            elapsed_ms: started.elapsed().as_secs_f64() * 1000.0,
        })
    }

    /// Libraries of a kind with their number of visible entries.
    ///
    /// # Errors
    ///
    /// Fails on database errors.
    pub fn facets(&self, kind: Kind) -> Result<Vec<Facet>, LibraryError> {
        self.conn
            .prepare_cached(
                "SELECT library, count(*) FROM items WHERE kind = ?1 AND canonical IS NULL
                 GROUP BY library ORDER BY library",
            )
            .and_then(|mut stmt| {
                stmt.query_map([kind.code()], |row| {
                    Ok(Facet {
                        library: row.get(0)?,
                        count: u64::try_from(row.get::<_, i64>(1)?).unwrap_or(0),
                    })
                })?
                .collect()
            })
            .map_err(sql_error)
    }

    /// Everything about one entry.
    ///
    /// # Errors
    ///
    /// Fails if there is no such entry.
    #[allow(
        clippy::too_many_lines,
        reason = "one query and the parts of the reply"
    )]
    pub fn get(&self, id: i64) -> Result<Details, LibraryError> {
        let conn = &self.conn;
        let row = conn
            .query_row(
                "SELECT i.id, i.kind, i.library, i.name, i.description, i.value, i.package, i.pins,
                        i.mounting, i.canonical,
                        (SELECT count(*) FROM members m WHERE m.item = i.id),
                        i.keywords, i.mpn, i.manufacturer, i.datasheet, i.params, i.units,
                        i.width, i.depth, i.height, i.model, i.file, i.mesh, i.data, i.source,
                        i.library || ':' || i.name
                 FROM items i WHERE i.id = ?1",
                [id],
                |row| {
                    let summary = summary_row(row)?;
                    let width: Option<f64> = row.get(17)?;
                    let size = width.map(|w| {
                        [
                            w,
                            row.get::<_, Option<f64>>(18).ok().flatten().unwrap_or(0.0),
                            row.get::<_, Option<f64>>(19).ok().flatten().unwrap_or(0.0),
                        ]
                    });
                    Ok((
                        summary,
                        (
                            row.get::<_, String>(11)?,
                            row.get::<_, String>(12)?,
                            row.get::<_, String>(13)?,
                            row.get::<_, String>(14)?,
                            row.get::<_, String>(15)?,
                            row.get::<_, i64>(16)?,
                        ),
                        size,
                        (
                            row.get::<_, Option<String>>(20)?,
                            row.get::<_, Option<String>>(21)?,
                            row.get::<_, Option<String>>(22)?,
                            row.get::<_, Option<i64>>(23)?,
                            row.get::<_, i64>(24)?,
                            row.get::<_, String>(25)?,
                        ),
                    ))
                },
            )
            .optional()
            .map_err(sql_error)?
            .ok_or_else(|| LibraryError::NotFound(format!("library entry {id}")))?;
        let (
            summary,
            (keywords, mpn, manufacturer, datasheet, params, units),
            size,
            (model, file, mesh, data, source_id, full_name),
        ) = row;
        let model_id = match &model {
            Some(key) => self.model_id(key)?,
            None => None,
        };
        let definition = match data {
            Some(blob) => conn
                .query_row("SELECT data FROM blobs WHERE id = ?1", [blob], |row| {
                    row.get::<_, Vec<u8>>(0)
                })
                .optional()
                .map_err(sql_error)?
                .and_then(|bytes| decompress(&bytes))
                .and_then(|text| serde_json::from_str(&text).ok())
                .map(|mut value| {
                    crate::entry::restore_ids(summary.kind, &full_name, &mut value);
                    value
                }),
            None => None,
        };
        let aliases = conn
            .prepare_cached("SELECT library || ':' || name FROM items WHERE canonical = ?1 ORDER BY library, name")
            .and_then(|mut stmt| stmt.query_map([id], |row| row.get(0))?.collect())
            .map_err(sql_error)?;
        let keys: Vec<String> = conn
            .prepare_cached(
                "SELECT g.key FROM members m JOIN groups g ON g.id = m.group_id
                 WHERE m.item = ?1 ORDER BY g.id",
            )
            .and_then(|mut stmt| stmt.query_map([id], |row| row.get(0))?.collect())
            .map_err(sql_error)?;
        let mut groups = Vec::new();
        for key in keys {
            groups.extend(self.group(&key)?);
        }
        let source = conn
            .query_row(
                "SELECT name, version, license, url FROM sources WHERE id = ?1",
                [source_id],
                |row| {
                    Ok(SourceInfo {
                        name: row.get(0)?,
                        version: row.get(1)?,
                        license: row.get(2)?,
                        url: row.get(3)?,
                    })
                },
            )
            .map_err(sql_error)?;
        let source_file = match &file {
            Some(file) => self.meta(&format!("models:{source_id}"))?.and_then(|dir| {
                let path = Path::new(&dir).join(file);
                path.is_file().then(|| path.display().to_string())
            }),
            None => None,
        };
        Ok(Details {
            summary,
            source_file,
            keywords,
            mpn,
            manufacturer,
            datasheet,
            params,
            units: u32::try_from(units).unwrap_or(0),
            size,
            model,
            model_id,
            file,
            mesh,
            aliases,
            duplicate_groups: groups,
            source,
            definition,
        })
    }

    /// Stores a value in the index metadata.
    ///
    /// # Errors
    ///
    /// Fails on database errors.
    pub fn set_meta(&self, key: &str, value: &str) -> Result<(), LibraryError> {
        self.conn
            .execute(
                "INSERT INTO meta(key, value) VALUES (?1, ?2)
                 ON CONFLICT(key) DO UPDATE SET value = excluded.value",
                params![key, value],
            )
            .map(drop)
            .map_err(sql_error)
    }

    fn meta(&self, key: &str) -> Result<Option<String>, LibraryError> {
        self.conn
            .query_row("SELECT value FROM meta WHERE key = ?1", [key], |row| {
                row.get(0)
            })
            .optional()
            .map_err(sql_error)
    }

    /// The id of a model by its key (`Resistor_SMD/R_0805_2012Metric`).
    ///
    /// # Errors
    ///
    /// Fails on database errors.
    pub fn model_id(&self, key: &str) -> Result<Option<i64>, LibraryError> {
        let Some((library, name)) = key.split_once('/') else {
            return Ok(None);
        };
        self.conn
            .query_row(
                "SELECT id FROM items WHERE kind = 2 AND library = ?1 AND name = ?2
                 ORDER BY source LIMIT 1",
                params![library, name],
                |row| row.get(0),
            )
            .optional()
            .map_err(sql_error)
    }

    /// The entry id of `library:name` of a kind.
    ///
    /// # Errors
    ///
    /// Fails on database errors.
    pub fn find(&self, kind: Kind, full_name: &str) -> Result<Option<i64>, LibraryError> {
        let (library, name) = crate::entry::split_name(full_name);
        self.conn
            .query_row(
                "SELECT id FROM items WHERE kind = ?1 AND library = ?2 AND name = ?3
                 ORDER BY source LIMIT 1",
                params![kind.code(), library, name],
                |row| row.get(0),
            )
            .optional()
            .map_err(sql_error)
    }

    /// Visible entries of a kind with a number of pins (footprints: distinct
    /// pad numbers) and their definitions: the candidates when parts from
    /// outside the library are checked for duplicates. At most `limit`.
    ///
    /// # Errors
    ///
    /// Fails on database errors.
    pub fn with_pins(
        &self,
        kind: Kind,
        pins: u32,
        limit: usize,
    ) -> Result<Vec<(i64, String, serde_json::Value)>, LibraryError> {
        let mut statement = self
            .conn
            .prepare(
                "SELECT i.id, i.library || ':' || i.name, b.data FROM items i
                 JOIN blobs b ON b.id = i.data
                 WHERE i.kind = ?1 AND i.pins = ?2 AND i.canonical IS NULL
                 ORDER BY i.library, i.name LIMIT ?3",
            )
            .map_err(sql_error)?;
        let rows = statement
            .query_map(
                params![kind.code(), pins, i64::try_from(limit).unwrap_or(i64::MAX)],
                |row| {
                    Ok((
                        row.get::<_, i64>(0)?,
                        row.get::<_, String>(1)?,
                        row.get::<_, Vec<u8>>(2)?,
                    ))
                },
            )
            .map_err(sql_error)?;
        let mut out = Vec::new();
        for row in rows {
            let (id, full_name, data) = row.map_err(sql_error)?;
            let Some(mut value) =
                decompress(&data).and_then(|text| serde_json::from_str(&text).ok())
            else {
                continue;
            };
            crate::entry::restore_ids(kind, &full_name, &mut value);
            out.push((id, full_name, value));
        }
        Ok(out)
    }

    /// One duplicate group by key.
    ///
    /// # Errors
    ///
    /// Fails on database errors.
    pub fn group(&self, key: &str) -> Result<Option<GroupInfo>, LibraryError> {
        let head = self
            .conn
            .query_row(
                "SELECT id, kind, class, reason, decision, canonical FROM groups WHERE key = ?1",
                [key],
                |row| {
                    Ok((
                        row.get::<_, i64>(0)?,
                        row.get::<_, i64>(1)?,
                        row.get::<_, String>(2)?,
                        row.get::<_, String>(3)?,
                        row.get::<_, String>(4)?,
                        row.get::<_, i64>(5)?,
                    ))
                },
            )
            .optional()
            .map_err(sql_error)?;
        let Some((id, kind, class, reason, decision, canonical)) = head else {
            return Ok(None);
        };
        let members = self
            .conn
            .prepare_cached(
                "SELECT m.item, i.library || ':' || i.name FROM members m
                 JOIN items i ON i.id = m.item WHERE m.group_id = ?1 ORDER BY m.rank",
            )
            .and_then(|mut stmt| {
                stmt.query_map([id], |row| {
                    Ok(Member {
                        id: row.get(0)?,
                        name: row.get(1)?,
                    })
                })?
                .collect()
            })
            .map_err(sql_error)?;
        Ok(Some(GroupInfo {
            key: key.to_owned(),
            kind: Kind::from_code(kind).unwrap_or(Kind::Symbol),
            class: Class::parse(&class).unwrap_or(Class::Near),
            reason,
            decision: Decision::parse(&decision).unwrap_or(Decision::Pending),
            canonical,
            members,
        }))
    }

    /// A page of duplicate groups.
    ///
    /// # Errors
    ///
    /// Fails on database errors.
    pub fn groups(
        &self,
        kind: Option<Kind>,
        class: Option<Class>,
        decision: Option<Decision>,
        offset: u32,
        limit: u32,
    ) -> Result<GroupPage, LibraryError> {
        let filter = "WHERE (?1 IS NULL OR kind = ?1) AND (?2 IS NULL OR class = ?2)
                      AND (?3 IS NULL OR decision = ?3)";
        let args = params![
            kind.map(Kind::code),
            class.map(Class::as_str),
            decision.map(Decision::as_str)
        ];
        let total: i64 = self
            .conn
            .query_row(
                &format!("SELECT count(*) FROM groups {filter}"),
                args,
                |row| row.get(0),
            )
            .map_err(sql_error)?;
        let keys: Vec<String> = self
            .conn
            .prepare(&format!(
                "SELECT key FROM groups {filter} ORDER BY id LIMIT {} OFFSET {offset}",
                limit.clamp(1, 500)
            ))
            .and_then(|mut stmt| stmt.query_map(args, |row| row.get(0))?.collect())
            .map_err(sql_error)?;
        let mut groups = Vec::with_capacity(keys.len());
        for key in keys {
            groups.extend(self.group(&key)?);
        }
        Ok(GroupPage {
            total: u64::try_from(total).unwrap_or(0),
            groups,
        })
    }

    /// The deduplication report with the current decisions.
    ///
    /// # Errors
    ///
    /// Fails on database errors.
    pub fn report(&self) -> Result<Report, LibraryError> {
        let tolerance: i64 = self
            .conn
            .query_row(
                "SELECT value FROM meta WHERE key = 'tolerance'",
                [],
                |row| row.get::<_, String>(0),
            )
            .map_err(sql_error)?
            .parse()
            .unwrap_or(fingerprint::TOLERANCE_UM);
        let mut kinds = Vec::new();
        for kind in Kind::ALL {
            let count = |sql: &str| -> Result<usize, LibraryError> {
                self.conn
                    .query_row(sql, [kind.code()], |row| row.get::<_, i64>(0))
                    .map(|n| usize::try_from(n).unwrap_or(0))
                    .map_err(sql_error)
            };
            kinds.push(KindSummary {
                kind: Some(kind),
                entries: count("SELECT count(*) FROM items WHERE kind = ?1")?,
                exact_groups: count("SELECT count(*) FROM groups WHERE kind = ?1 AND class = 'exact'")?,
                near_groups: count("SELECT count(*) FROM groups WHERE kind = ?1 AND class = 'near'")?,
                alias_groups: count("SELECT count(*) FROM groups WHERE kind = ?1 AND class = 'alias'")?,
                merged: count("SELECT count(*) FROM items WHERE kind = ?1 AND canonical IS NOT NULL")?,
                pending: count(
                    "SELECT count(*) FROM groups WHERE kind = ?1 AND class = 'near' AND decision = 'pending'",
                )?,
            });
        }
        let page = self.groups(None, None, None, 0, u32::MAX)?;
        let mut groups = page.groups;
        let mut offset = groups.len();
        while (offset as u64) < page.total {
            let next = self.groups(
                None,
                None,
                None,
                u32::try_from(offset).unwrap_or(u32::MAX),
                500,
            )?;
            if next.groups.is_empty() {
                break;
            }
            offset += next.groups.len();
            groups.extend(next.groups);
        }
        Ok(Report {
            tolerance: format!("{:.3}", tolerance as f64 / 1000.0),
            kinds,
            groups: groups
                .into_iter()
                .map(|group| {
                    let canonical = group
                        .members
                        .iter()
                        .find(|m| m.id == group.canonical)
                        .map(|m| m.name.clone())
                        .unwrap_or_default();
                    GroupReport {
                        key: group.key,
                        kind: group.kind,
                        class: group.class,
                        reason: group.reason,
                        members: group.members.into_iter().map(|m| m.name).collect(),
                        canonical,
                        decision: group.decision,
                    }
                })
                .collect(),
        })
    }

    /// Numbers of the index.
    ///
    /// # Errors
    ///
    /// Fails on database errors.
    pub fn status(&self) -> Result<Status, LibraryError> {
        let conn = &self.conn;
        let mut counts = BTreeMap::new();
        let mut visible = BTreeMap::new();
        for kind in Kind::ALL {
            let all: i64 = conn
                .query_row(
                    "SELECT count(*) FROM items WHERE kind = ?1",
                    [kind.code()],
                    |r| r.get(0),
                )
                .map_err(sql_error)?;
            let shown: i64 = conn
                .query_row(
                    "SELECT count(*) FROM items WHERE kind = ?1 AND canonical IS NULL",
                    [kind.code()],
                    |r| r.get(0),
                )
                .map_err(sql_error)?;
            counts.insert(kind.as_str().to_owned(), u64::try_from(all).unwrap_or(0));
            visible.insert(kind.as_str().to_owned(), u64::try_from(shown).unwrap_or(0));
        }
        let mut groups = BTreeMap::new();
        for class in [Class::Exact, Class::Near, Class::Alias] {
            let n: i64 = conn
                .query_row(
                    "SELECT count(*) FROM groups WHERE class = ?1",
                    [class.as_str()],
                    |r| r.get(0),
                )
                .map_err(sql_error)?;
            groups.insert(class.as_str().to_owned(), u64::try_from(n).unwrap_or(0));
        }
        let pending: i64 = conn
            .query_row(
                "SELECT count(*) FROM groups WHERE class = 'near' AND decision = 'pending'",
                [],
                |r| r.get(0),
            )
            .map_err(sql_error)?;
        let meshes: i64 = conn
            .query_row(
                "SELECT count(*) FROM items WHERE kind = 2 AND mesh IS NOT NULL",
                [],
                |r| r.get(0),
            )
            .map_err(sql_error)?;
        let sources = conn
            .prepare("SELECT name, version, license, url FROM sources ORDER BY id")
            .and_then(|mut stmt| {
                stmt.query_map([], |row| {
                    Ok(SourceInfo {
                        name: row.get(0)?,
                        version: row.get(1)?,
                        license: row.get(2)?,
                        url: row.get(3)?,
                    })
                })?
                .collect()
            })
            .map_err(sql_error)?;
        Ok(Status {
            path: self.dir.display().to_string(),
            format: FORMAT,
            sources,
            counts,
            visible,
            groups,
            pending: u64::try_from(pending).unwrap_or(0),
            meshes: u64::try_from(meshes).unwrap_or(0),
            bytes: std::fs::metadata(self.dir.join(INDEX_FILE)).map_or(0, |m| m.len()),
            release: self.meta("release")?,
        })
    }

    /// Mesh hashes of the models of a 3D library (`Resistor_SMD`), with
    /// their model names.
    ///
    /// # Errors
    ///
    /// Fails on database errors.
    pub fn meshes(&self) -> Result<Vec<(String, String)>, LibraryError> {
        self.conn
            .prepare("SELECT library, mesh FROM items WHERE kind = 2 AND mesh IS NOT NULL ORDER BY library, mesh")
            .and_then(|mut stmt| stmt.query_map([], |row| Ok((row.get(0)?, row.get(1)?)))?.collect())
            .map_err(sql_error)
    }
}

fn summary_row(row: &rusqlite::Row<'_>) -> rusqlite::Result<Summary> {
    Ok(Summary {
        id: row.get(0)?,
        kind: Kind::from_code(row.get(1)?).unwrap_or(Kind::Symbol),
        library: row.get(2)?,
        name: row.get(3)?,
        description: row.get(4)?,
        value: row.get(5)?,
        package: row.get(6)?,
        pins: u32::try_from(row.get::<_, i64>(7)?).unwrap_or(0),
        mounting: row.get(8)?,
        merged_into: row.get(9)?,
        groups: u32::try_from(row.get::<_, i64>(10)?).unwrap_or(0),
    })
}

fn insert_entries(
    tx: &rusqlite::Transaction<'_>,
    entries: &[Entry],
    groups: &[dedup::Group],
) -> Result<(), LibraryError> {
    // Names of the other members of exact groups, so a merged entry is
    // still found by its own name.
    let mut aliases: Vec<Vec<String>> = vec![Vec::new(); entries.len()];
    for group in groups.iter().filter(|g| g.class == Class::Exact) {
        for &a in &group.members {
            for &b in &group.members {
                if a != b {
                    aliases[a].push(entries[b].name.clone());
                }
            }
        }
    }
    let mut item = tx
        .prepare(
            "INSERT INTO items(id, kind, source, library, name, description, keywords, value,
               package, mpn, manufacturer, datasheet, params, pins, units, mounting, width, depth,
               height, model, file, mesh, data)
             VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10, ?11, ?12, ?13, ?14, ?15, ?16, ?17,
               ?18, ?19, ?20, ?21, ?22, ?23)",
        )
        .map_err(sql_error)?;
    let mut blob = tx
        .prepare("INSERT INTO blobs(id, hash, data) VALUES (?1, ?2, ?3)")
        .map_err(sql_error)?;
    let mut blob_ids: std::collections::HashMap<String, i64> = std::collections::HashMap::new();
    let mut search = tx
        .prepare(
            "INSERT INTO search(rowid, name, library, description, keywords, value, package, mpn,
               params, aliases) VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10)",
        )
        .map_err(sql_error)?;
    for (index, entry) in entries.iter().enumerate() {
        let id = index as i64 + 1;
        let data = match &entry.data {
            Some(json) => {
                let hash = fingerprint::hash(json);
                let next = blob_ids.len() as i64 + 1;
                let id = *blob_ids.entry(hash.clone()).or_insert(next);
                if id == next {
                    blob.execute(params![id, hash, compress(json)])
                        .map_err(sql_error)?;
                }
                Some(id)
            }
            None => None,
        };
        let size = entry.size.unwrap_or([0.0; 3]);
        item.execute(params![
            id,
            entry.kind.code(),
            entry.source as i64,
            entry.library,
            entry.name,
            entry.description,
            entry.keywords,
            entry.value,
            entry.package,
            entry.mpn,
            entry.manufacturer,
            entry.datasheet,
            entry.params,
            i64::from(entry.pins),
            i64::from(entry.units),
            entry.mounting,
            entry.size.map(|_| size[0]),
            entry.size.map(|_| size[1]),
            entry.size.map(|_| size[2]),
            entry.model,
            entry.file,
            entry.mesh,
            data,
        ])
        .map_err(sql_error)?;
        // Names are searched as written and with their parts
        // (`R_0805_2012Metric` → `R 0805 2012Metric`).
        search
            .execute(params![
                id,
                entry.name,
                entry.library,
                entry.description,
                entry.keywords,
                entry.value,
                entry.package,
                format!("{} {}", entry.mpn, entry.manufacturer).trim(),
                entry.params,
                aliases[index].join(" ")
            ])
            .map_err(sql_error)?;
    }
    Ok(())
}

fn insert_groups(
    tx: &rusqlite::Transaction<'_>,
    entries: &[Entry],
    groups: &[dedup::Group],
) -> Result<(), LibraryError> {
    let _ = entries;
    let mut group_stmt = tx
        .prepare(
            "INSERT INTO groups(id, key, kind, class, reason, decision, canonical)
             VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7)",
        )
        .map_err(sql_error)?;
    let mut member = tx
        .prepare("INSERT OR IGNORE INTO members(group_id, item, rank) VALUES (?1, ?2, ?3)")
        .map_err(sql_error)?;
    for (index, group) in groups.iter().enumerate() {
        let id = index as i64 + 1;
        group_stmt
            .execute(params![
                id,
                group.key,
                group.kind.code(),
                group.class.as_str(),
                group.reason,
                group.class.default_decision().as_str(),
                group.members[0] as i64 + 1
            ])
            .map_err(sql_error)?;
        for (rank, &item) in group.members.iter().enumerate() {
            member
                .execute(params![id, item as i64 + 1, rank as i64])
                .map_err(sql_error)?;
        }
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn user_input_becomes_prefix_phrases() {
        assert_eq!(fts_query("r 0805").as_deref(), Some("\"r\"* \"0805\"*"));
        assert_eq!(fts_query("SOT-23").as_deref(), Some("\"sot 23\"*"));
        assert_eq!(fts_query("  \"' ").as_deref(), None);
        assert_eq!(fts_query("Ü-Bahn").as_deref(), Some("\"ü bahn\"*"));
    }
}
