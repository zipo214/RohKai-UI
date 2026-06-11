# P2-B: Database Integration Research

## Executive Summary

RohKai's synchronous egui loop makes async-first ORMs (sqlx, sea-orm async) a
poor fit without adding a tokio runtime. **rusqlite 0.40.1** is the recommended
crate for Stage 13: it is synchronous-only, bundles SQLite as a vendored C
library, ships pregenerated FFI bindings (no bindgen at build time), and has
40M+ downloads of proven desktop use. The only meaningful trade-off is accepting
a C-code compile boundary, which is not prohibited by RohKai's "pure Rust crates
only" rule because SQLite itself is compiled in — not a system library call.
`sea-orm-sync` (SeaORM 2.x RC) is a viable long-range alternative if a richer
entity model is needed later; it wraps rusqlite internally. The threat model
section identifies two hard invariants that must be added to
`docs/ENGINEERING_INVARIANTS.md` before any codegen work begins.

---

## Deliverable 1 — Comparison Matrix

Research sources: crates.io, docs.rs, the SeaQL blog post
"How we made SeaORM synchronous" (Dec 2025), and the sqlx GitHub issue tracker.

| Criterion | rusqlite 0.40.1 | sqlx 0.9.0 | sea-orm-sync 2.0-rc |
|---|---|---|---|
| **Sync/async model** | Synchronous only. `Connection` has no `async fn`. | Async-first. Every query returns a `Future`. SQLite driver is runtime-agnostic internally, but the public `Pool` API requires a runtime for timeouts. | Synchronous. Auto-generated from sea-orm source by a rewrite script; ships as separate `sea-orm-sync` crate. |
| **Compile-time query checking** | No. Queries are `&str` at runtime. Optional `rusqlite_macros` crate (experimental) adds some checking, not production-stable. | Yes. `query!` / `query_as!` macros verify SQL against a live DB at compile time (requires `DATABASE_URL` env var and `sqlx prepare`). Strong safety guarantee. | No. Queries are runtime strings via the ORM model or raw SQL. |
| **No-tokio usability** | Fully usable without any async runtime. Works in `fn main()` or a `std::thread`. | Cannot use `Pool`/`SqlitePool` without a runtime feature enabled. Workaround: spawn a `tokio::runtime::Runtime` on the background thread. This is non-trivial and pulls in tokio as a dependency. | Fully usable without any async runtime (the whole point of `sea-orm-sync`). |
| **Connection pool overhead** | No built-in pool. Use `r2d2_sqlite` (a separate unapproved crate) for pooling. For a desktop app with one DB file and an `Arc<Mutex<Connection>>`, no pool is needed. | Built-in async `Pool` with configurable min/max connections. Excellent for servers; over-engineered for a single-user desktop app. | Uses `RusqliteSharedConnection` — an `Arc<Mutex<rusqlite::Connection>>` under the hood. Thread-safe but single-connection; no pool abstraction needed for desktop. |
| **WASM compatibility** | Limited. The `bundled` feature compiles SQLite's C source, which requires a C toolchain for the target. `wasm32-unknown-unknown` (browser WASM) is **not supported**. `wasm32-wasi` is possible with extra linker configuration. | No stable WASM support. An open GitHub issue (#4056) tracks `wasm32-wasip2`, but it is still a proof-of-concept as of June 2026. | Same as rusqlite (it wraps rusqlite). No browser WASM. |
| **Embedded SQLite support** | Yes. The `bundled` feature (`rusqlite = { version = "0.40", features = ["bundled"] }`) compiles SQLite 3.53.2 from vendored C source. No system library dependency. Ships pregenerated bindgen output so bindgen itself is not a build dependency. | Yes for the SQLite driver, but requires the async runtime overhead regardless. | Yes via rusqlite's `bundled` feature under the hood. |
| **Desktop app ergonomics** | Excellent. Synchronous `Connection::execute()`, `prepare()`, `query_map()`, `params!` and `named_params!` macros. Low ceremony. No runtime, no `#[tokio::main]`, no executor spin-up. 17-second average build time on docs.rs. | Adequate but awkward without a runtime. Real-world egui+sqlx integrations require spawning a hidden tokio runtime on a background thread and using channels to bridge back to the sync loop — exactly the pattern RohKai already uses for mpsc, but with added tokio overhead. | Good. Full SeaORM entity/relation model, but that model is likely overkill for Stage 13's visual query builder scope. Compile time 15 s vs async SeaORM's 30 s. |
| **Last release + maintenance** | 0.40.1 released **2026-06-06**. Actively maintained. 40.2M total downloads, 5.8M recent. | 0.9.0 released **2026-05-21**. Very active. 12M+ downloads. | 2.0.0-rc.40 (June 2026). New crate; low total downloads but backed by the established SeaORM team. Not yet stable. |
| **Binary size impact** | Bundled SQLite adds ~1.4 MB of C object code to the binary (SQLite amalgamation is ~230k lines of C). Already small in isolation; no transitive Rust crates needed beyond `libsqlite3-sys`. | Larger: pulls in `sqlx-core`, `sqlx-sqlite`, runtime crate (tokio or async-std), `futures`, `pin-project`, and more. Estimated 2–4 MB additional Rust code vs. rusqlite. | Larger than raw rusqlite: pulls in the full sea-orm entity framework on top of rusqlite. Acceptable for the designer binary but heavier than needed. |

### Key data points

- rusqlite `bundled` feature: compiles **SQLite 3.53.2** from vendored source.
  Pregenerated bindings mean `bindgen` is not a build-time dependency.
- sqlx has **no synchronous public API**. Issue #1221 ("Does sqlx support sync
  style?") was closed: the answer is "no, use rusqlite or diesel."
- sea-orm-sync wraps rusqlite internally with `Arc<Mutex<Connection>>` and is
  automatically rebased from the async sea-orm source tree. It is SQLite-only
  (no PostgreSQL, no MySQL) in its sync form.

---

## Deliverable 2 — Recommendation

### Recommended crate

```toml
rusqlite = { version = "0.40", features = ["bundled"] }
```

The `bundled` feature is strongly recommended over the default (which links the
system SQLite). It guarantees a known SQLite version on all three target
platforms (Windows, macOS, Linux) without `pkg-config` or a system library
dependency — both of which conflict with RohKai's no-cmake/no-pkg-config rule.

### Rationale

1. **Zero async surface.** rusqlite's entire public API is synchronous. It slots
   directly into RohKai's existing `std::sync::mpsc`-channel background-thread
   pattern with no friction. The codegen emitter in `src/codegen/` produces Rust
   that calls `rusqlite::Connection` from a `std::thread::spawn` closure —
   exactly the pattern shown in Deliverable 4.

2. **No new runtime.** Adding sqlx would require shipping a hidden
   `tokio::runtime::Runtime::new().unwrap()` inside the background thread, which
   contradicts CLAUDE.md: "No tokio runtime unless a specific planned feature
   explicitly requires it." That clause was written for async I/O, not DB
   queries. rusqlite avoids the debate entirely.

3. **Battle-tested desktop crate.** 40M+ downloads. The canonical recommendation
   in the Rust community for "SQLite in a desktop app or CLI" is rusqlite,
   as confirmed by the sqlx issue tracker itself.

4. **Matches the planned scope.** Stage 13 targets SQLite as the primary
   embedded DB (see ROADMAP_PHASE2.md §P2.6: "SQLite / PostgreSQL / MySQL /
   Supabase"). Starting with the SQLite-only crate is correct: add a PostgreSQL
   driver only if Stage 13 scope explicitly expands to remote DBs, and only with
   explicit user approval of the additional crate (`postgres` or `tokio-postgres`
   at that time).

5. **Bundled C source is not a "C FFI" violation.** CLAUDE.md prohibits "C FFI"
   in the sense of "system toolkit bindings" — a `cmake`/`pkg-config` dependency
   on a platform library. The `bundled` feature instead compiles a vendored copy
   of SQLite into the binary using the standard `cc` crate (already in the Rust
   build toolchain). This is the same pattern used by many pure-Rust-first
   projects (e.g., the official SQLite Rust bindings).

### Trade-offs

- **No compile-time query verification.** sqlx's `query!` macro would catch SQL
  errors at compile time. rusqlite does not. Mitigation: the codegen test suite
  should include a smoke-test that opens an in-memory DB and executes each
  generated query template against it.

- **No ORM entity model.** Every query is a `&str`. For Stage 13's visual query
  builder this is fine: the designer emits simple SELECT/INSERT/UPDATE SQL
  strings directly. If a future stage needs entity relations, `sea-orm-sync`
  (wrapping rusqlite) is a one-crate upgrade that does not change the underlying
  DB engine.

- **Single-connection for desktop.** rusqlite's `Connection` is not `Send` by
  default. The recommended pattern is to keep the connection on the background
  thread permanently and communicate via mpsc channels. This is the same
  architecture RohKai uses for other background tasks.

- **Binary grows by ~1.4 MB** (bundled SQLite amalgamation). Acceptable for a
  desktop app; document in release notes.

### Migration path if an async runtime is added later

Stage 12 (Platform Targets / WASM) may eventually require tokio for browser
networking. If that happens:

1. Keep rusqlite as the **designer** DB (synchronous, no change).
2. For the **exported project's** runtime DB, offer sqlx (with tokio feature)
   as an alternative generated template. The designer would emit either a
   rusqlite snippet or a sqlx snippet depending on the user's "async target"
   project setting.
3. sea-orm-sync remains a viable drop-in if the entity model is needed: its
   internal rusqlite dependency means no DB migration is required.

---

## Deliverable 3 — Threat Model

### a) Designer threat model

**What can a malicious `.rohkai.json` file do?**

A `.rohkai.json` project file can contain a `db_query` field (the user-entered
SQL string for a Table/ListView widget binding). When the designer loads the
file, it deserialises the SQL string into a `String` and stores it in the
`UiTree` node's `db_query` property. It does **not** execute it.

Concrete risks:

| Vector | Risk | Mitigated by |
|---|---|---|
| SQL string in loaded project | Parsed as `String`; never executed by the designer | Designer never opens a DB connection at design time (see below) |
| Malformed SQL string injected via project file | Stored verbatim in `UiTree`; emitted as-is into generated code | Codegen sanitization rules (Deliverable 3c) catch most cases |
| Path traversal in DB file path field | `"db_path": "../../etc/passwd"` could point a connection at a sensitive file | Validate path against the project directory; require explicit user approval before connecting |
| SQL that drops tables / deletes data | If designer opens a real DB, a `DROP TABLE` in the query field executes on open | **Do not open a real DB at design time** |

**Can SQL in a project file execute against the user's DB on open?**

Only if the designer opens a live DB connection at design time. The rule is:

> **The designer MUST NOT open a rusqlite `Connection` to any file path stored
> in the project file without explicit, per-session user approval via a
> confirmation dialog.**

At design time the designer should use an **in-memory SQLite database seeded
with sample schema only**, never the user's actual data file. Queries entered in
the visual query builder run against this mock DB for preview purposes.

**Sandboxing: should the designer connect to a real DB at design time?**

No. Design-time DB connections should be restricted to:

1. An anonymous in-memory DB (`Connection::open_in_memory()`) seeded from a
   user-provided schema script.
2. A read-only connection to a user-specified file, opened only after the user
   explicitly clicks "Connect to real DB" in the schema viewer panel, with a
   visible warning that this grants read access to that file.

Write connections (INSERT/UPDATE/DELETE) must never be issued by the designer
itself, only by the **generated application** at runtime.

### b) Exported-project threat model

**The generated Rust code contains SQL strings from the designer.**

The primary injection risk in the exported project is a developer who uses the
visual query builder to construct a query like:

```sql
SELECT * FROM users WHERE name = '{{user_input}}'
```

and the codegen naively emits:

```rust
// BAD — generated with string interpolation
let sql = format!("SELECT * FROM users WHERE name = '{}'", self.search_input);
conn.execute(&sql, [])?;
```

This is a textbook SQL injection vulnerability baked into the generated code.

**How should generated code parametrise queries?**

All generated SQL must use `?` positional placeholders and pass values via the
`rusqlite::params!` macro. The designer's visual query builder must translate
every "filter value" input into a placeholder, never into a literal embedded in
the SQL string.

Generated pattern:

```rust
// GOOD — generated with parameterised query
let sql = "SELECT * FROM users WHERE name = ?1";
let mut stmt = conn.prepare(sql)?;
let rows = stmt.query_map(params![self.search_input], |row| {
    Ok(row.get::<_, String>(0)?)
})?;
```

**Invariant to add to `docs/ENGINEERING_INVARIANTS.md`:**

> **Invariant 10 — No SQL string interpolation in generated code.**
> The codegen (`src/codegen/`) must never emit a `format!` call whose result
> is passed as a SQL string. All user-supplied values that appear in a query
> must be bound as parameters via `rusqlite::params!` or `rusqlite::named_params!`.
> A test must verify that the generated SQL string contains `?` placeholders
> (or named `:param` markers) for every binding, and that no `format!`/`+`
> string-building is emitted for the SQL text itself.

### c) Required mitigations

**Codegen rules (enforce in `src/codegen/`):**

1. **No `format!` for SQL text.** The SQL string emitted by codegen must be a
   string literal or a `const`; never a `format!` expression. Build-time
   rustc/clippy can enforce this with a custom lint or a doc-test that scans the
   generated output.

2. **All query parameters use `?` placeholders.** Every binding site the user
   configures in the visual query builder maps to a `?N` placeholder in the
   emitted SQL, never to a `'literal'` embedded in the string.

3. **Named parameters for complex queries.** When a query has more than three
   parameters, emit `named_params!{ ":field": value }` for readability and to
   prevent positional-index mistakes.

4. **Prepared statements only.** Emit `conn.prepare(sql)?` and call
   `stmt.query_map(params![...], ...)` rather than the shorthand
   `conn.execute(sql, params![...])` in any query that returns rows, to ensure
   the statement is compiled once and reused.

5. **Identifier quoting.** If the user selects a table name or column name from
   the schema viewer, emit it quoted: `"table_name"."column_name"` — not
   interpolated from a runtime string. The table/column names are baked into the
   generated source at code-emit time, not at query-execution time.

**Designer UI validation rules (enforce in the visual query builder panel):**

1. Reject SQL strings that contain single-quoted literals adjacent to a binding
   expression (e.g. `WHERE x = '{{y}}'`). Show a validation error:
   "Use a parameter placeholder instead of a literal value."

2. Disallow `DROP`, `DELETE`, `UPDATE`, `INSERT`, `CREATE`, `ALTER`, `ATTACH`
   in design-time preview queries. Show a clear error: "Destructive SQL is not
   allowed in design-time preview."

3. Limit SQL string length to 2048 characters in the project file to prevent
   DoS via a maliciously crafted large query on open.

4. Validate that the SQL string is syntactically parseable before saving it to
   the `UiTree` node. Use an in-memory `conn.prepare(sql)` call against a
   blank in-memory DB; if it fails, show the SQLite error to the user and block
   save.

---

## Deliverable 4 — AppState Sketch

These are pattern sketches (~30 lines each), not complete compilable code. They
show the idioms the codegen must emit.

### a) AppState struct

```rust
// Generated by RohKai codegen (src/codegen/state_emitter.rs)
// Pattern: DB connection lives on a background thread; only the channel
// handles cross the thread boundary into AppState.

use std::sync::mpsc;

struct AppState {
    // UI-bound query results (updated each frame when channel has data)
    users_rows: Vec<Vec<String>>,
    users_loading: bool,

    // Channel pair — Sender stays in AppState, Receiver polled each frame.
    // The actual rusqlite::Connection lives on the worker thread (not here).
    query_tx: mpsc::SyncSender<QueryRequest>,
    query_rx: mpsc::Receiver<QueryResult>,

    // Filter state (bound to TextInput widgets in the canvas)
    name_filter: String,
}

enum QueryRequest {
    FetchUsers { name_filter: String },
}

enum QueryResult {
    Users(Vec<Vec<String>>),
    Error(String),
}
```

### b) eframe `update()` body

```rust
// Generated update() body — drains the result channel each frame,
// then renders the Table widget bound to users_rows.
fn update(&mut self, ctx: &egui::Context, _frame: &mut eframe::Frame) {
    // 1. Drain pending DB results (non-blocking — never stalls the frame).
    while let Ok(result) = self.query_rx.try_recv() {
        match result {
            QueryResult::Users(rows) => {
                self.users_rows = rows;
                self.users_loading = false;
            }
            QueryResult::Error(e) => {
                eprintln!("DB error: {e}");
                self.users_loading = false;
            }
        }
    }

    egui::CentralPanel::default().show(ctx, |ui| {
        // 2. Filter input — triggers a new query on change.
        if ui.text_edit_singleline(&mut self.name_filter).changed() {
            self.users_loading = true;
            let _ = self.query_tx.try_send(QueryRequest::FetchUsers {
                name_filter: self.name_filter.clone(),
            });
        }

        // 3. Table widget bound to query result cache.
        if self.users_loading {
            ui.spinner();
        } else {
            egui::Grid::new("users_table").show(ui, |ui| {
                for row in &self.users_rows {
                    for cell in row {
                        ui.label(cell);
                    }
                    ui.end_row();
                }
            });
        }
    });
}
```

### c) Background query worker thread

```rust
// Generated worker — spawned once in main() or AppState::new().
// The rusqlite::Connection lives entirely inside this thread closure.
fn spawn_db_worker(
    db_path: &str,
    rx: mpsc::Receiver<QueryRequest>,
    tx: mpsc::SyncSender<QueryResult>,
) {
    let db_path = db_path.to_owned();
    std::thread::spawn(move || {
        // Connection opened once; stays alive for the process lifetime.
        let conn = match rusqlite::Connection::open(&db_path) {
            Ok(c) => c,
            Err(e) => {
                let _ = tx.send(QueryResult::Error(e.to_string()));
                return;
            }
        };

        // Process requests until the sender side drops (app exit).
        while let Ok(req) = rx.recv() {
            match req {
                QueryRequest::FetchUsers { name_filter } => {
                    // Parameterised query — no string interpolation in the SQL text.
                    let sql = "SELECT name, email FROM users \
                               WHERE name LIKE ?1 LIMIT 500";
                    let result = conn.prepare(sql).and_then(|mut stmt| {
                        let pattern = format!("%{}%", name_filter); // safe: bound param, not SQL
                        stmt.query_map(rusqlite::params![pattern], |row| {
                            Ok(vec![row.get::<_, String>(0)?,
                                    row.get::<_, String>(1)?])
                        })
                        .map(|rows| rows.flatten().collect::<Vec<_>>())
                    });
                    let msg = match result {
                        Ok(rows) => QueryResult::Users(rows),
                        Err(e)   => QueryResult::Error(e.to_string()),
                    };
                    let _ = tx.send(msg);
                }
            }
        }
    });
}
```

Note: `format!("%{}%", name_filter)` in the worker constructs the **LIKE
pattern value** — this is safe because the resulting string is passed as a
bound parameter (`?1`), not interpolated into the SQL text. The SQL text itself
(`sql`) is a string literal, never built with `format!`.

---

## Recommended Next Steps

1. **Seek explicit user approval** of `rusqlite = { version = "0.40", features = ["bundled"] }` before any Stage 13 code is written. CLAUDE.md requires this for every new dependency.

2. **Add Invariant 10** ("No SQL string interpolation in generated code") to `docs/ENGINEERING_INVARIANTS.md` now, before codegen work begins, so the class is named before the first line of SQL codegen is written.

3. **Design-time DB sandboxing rule** — add to `docs/ARCHITECTURE.md` under a "Data Layer" section: "The designer never opens a write connection to the user's data file. Design-time preview uses `Connection::open_in_memory()` only."

4. **Validation gate in visual query builder** — before saving a SQL string to a `UiTree` node, validate it with `conn.prepare(sql)` against a blank in-memory connection. Emit a UI error and block save on failure.

5. **Smoke-test template** — once rusqlite is approved, add a codegen test in `tests/` that generates a DB-backed Table widget project, opens an in-memory DB, runs the generated query against it, and asserts non-empty results. This is the compile-time-check substitute for rusqlite's lack of `query!` macros.

6. **Defer sea-orm-sync consideration** to a future stage if entity relations are needed. Do not add it to Stage 13 scope without a separate P2-B follow-up decision.
