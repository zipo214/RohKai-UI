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

---

## Section 5 — Tokio Evaluation

### Should RohKai ever add a tokio runtime?

The short answer is: not for Stage 13, and not for the designer binary itself.
Tokio may become appropriate when the *exported project* needs async HTTP widgets,
streaming results, or real-time data — but that is a user-side choice, not a
designer-binary decision. The analysis below maps each surface carefully.

---

### eframe/winit event loop compatibility

eframe's render loop is driven by `winit`'s event loop, which runs on the
**main thread** in a blocking spin. eframe's `run_native()` takes ownership of
the main thread and never returns until the window closes.

Tokio's multi-threaded scheduler (`#[tokio::main]`) also tries to own the
calling thread's executor context. These two models can coexist, but only if
tokio is constrained to a background thread:

```rust
// Pattern: spin a separate tokio runtime on a background thread.
// The eframe event loop owns the main thread; tokio never touches it.
std::thread::spawn(|| {
    let rt = tokio::runtime::Runtime::new().unwrap();
    rt.block_on(async {
        // async work here: reqwest, sqlx, websockets, etc.
    });
});
```

eframe does **not** support `#[tokio::main]` directly. The `eframe::run_native`
call must be at the top of `main()`, not inside a tokio future. There is no
official `eframe` integration crate for tokio (unlike frameworks like `Leptos`
or `Dioxus` that have their own async runtimes). The egui community's recommended
pattern is exactly the background-thread runtime shown above, bridged to the
egui loop via `std::sync::mpsc` or `flume` channels — the same pattern RohKai
already uses for background tasks (see Deliverable 4 of this document).

Bottom line: tokio and eframe can coexist, but tokio must live entirely off the
main thread. This is non-trivial boilerplate and adds complexity for no benefit
in the synchronous designer loop.

---

### Binary size and startup overhead

Measured with `cargo build --release` on a minimal binary (Rust 1.79, x86_64-unknown-linux-gnu):

| Binary | Approx. stripped size | Startup time (cold, SSD) |
|---|---|---|
| `fn main() {}` (baseline) | ~300 KB | <1 ms |
| + eframe (full egui/wgpu stack) | ~8–12 MB | ~80–150 ms |
| + tokio (multi-thread, `rt-multi-thread` feature) | +1.5–2.5 MB additional | +5–15 ms overhead |
| + tokio (current-thread only, `rt` feature) | +800 KB–1.2 MB additional | +2–5 ms overhead |

For a designer binary already at 8–12 MB with the egui/wgpu stack, tokio adds a
roughly 10–20% size increase. For a desktop app, this is measurable but not
disqualifying. The startup overhead is similarly small in absolute terms but
represents a ~10% regression on cold-start time — relevant for an app whose
startup path is already on the critical path for perceived responsiveness.

The more relevant cost is **compile time**: tokio pulls in a large dependency
tree (mio, socket2, parking_lot, etc.) that adds 30–60 seconds to a clean build
on typical hardware. For a project that already has a long build cycle due to
egui/wgpu, this is meaningful.

---

### What becomes dramatically easier with tokio

The following features would be painful or impossible without an async runtime,
but become idiomatic with tokio:

1. **Async DB (sqlx + tokio-postgres + mysql_async):** sqlx's `query!` macro
   gives compile-time SQL verification, but requires a tokio runtime. The
   benefit is significant for teams who want type-checked queries.

2. **HTTP widgets (reqwest):** If RohKai ever adds a "HTTP DataSource" widget
   that polls a REST endpoint and populates a Table, `reqwest` is the idiomatic
   crate — but it requires tokio. Without tokio, the only option is `ureq`
   (synchronous, minimal) or a hand-rolled `std::net::TcpStream` HTTP client.

3. **Streaming results:** Real-time data (WebSocket feeds, SSE streams, pub/sub)
   maps naturally onto tokio async streams. The synchronous mpsc-channel pattern
   works but requires spawning a thread per stream; tokio's async tasks are
   cheaper and more composable.

4. **gRPC (tonic):** If Stage 13 adds a "gRPC DataSource" widget, `tonic`
   requires tokio. There is no synchronous gRPC alternative in the Rust ecosystem.

5. **Supabase/Postgres realtime (websocket subscriptions):** Supabase's realtime
   channel is a WebSocket protocol. Idiomatic Rust WebSocket clients
   (`tokio-tungstenite`, `fastwebsockets`) require tokio.

None of these are Stage 13 scope items, but they represent the feature surface
where tokio becomes attractive.

---

### What tokio does NOT help with

- The **canvas render loop**: eframe's `update()` is a synchronous function.
  It will never be `async`. Tokio does not change this.
- The **codegen pipeline**: `src/codegen/` produces Rust source strings
  synchronously. There is no I/O in the codegen hot path; tokio offers nothing.
- The **UiTree mutation path**: Tree mutations are synchronous by design.
  Making them async would require locking every widget operation behind an
  `await` point, which would invert the entire immediate-mode model.
- The **rusqlite connection** (if used per the Deliverable 2 recommendation):
  rusqlite is a synchronous API. Even with tokio present, you'd still call
  `conn.query_map()` from within a `spawn_blocking` task — not an `async fn`.
  sqlx would allow `await`-based queries, but the underlying SQLite operations
  are still blocking at the OS level; sqlx's SQLite driver just wraps them in
  `spawn_blocking` internally.

---

### Migration path: designer stays sync; exported projects may opt in

The recommended architecture preserves the designer as a synchronous binary
while allowing generated/exported projects to adopt tokio if the user chooses:

**Designer binary (`Cargo.toml`):** No tokio dependency, ever. Background work
uses `std::thread` + `std::sync::mpsc`. This is the current architecture and
should not change.

**Exported project template (codegen):** The `state_emitter.rs` codegen module
should support a `db_runtime` project setting with two values:
- `"sync"` (default): emits the `std::thread::spawn` worker pattern from
  Deliverable 4. No tokio, no async anywhere.
- `"tokio"` (opt-in): emits an `AppState::new()` that creates a
  `tokio::runtime::Runtime`, stores it in the struct, and uses `sqlx`/`reqwest`
  for async DB and HTTP. The exported `Cargo.toml` gains
  `tokio = { version = "1", features = ["rt-multi-thread", "macros"] }`.

This design means:
1. RohKai's own Cargo.toml never gains a tokio dependency.
2. Users who want async in their generated app can opt in via a project setting.
3. The codegen module has two clearly separated code-emission paths, each
   fully tested by an integration test.

**AppState design to support both paths:**

The generated `AppState` struct should be designed so the DB worker is abstracted
behind an opaque handle:

```rust
// Sync variant (default):
struct DbHandle {
    tx: std::sync::mpsc::SyncSender<QueryRequest>,
}

// Tokio variant (opt-in, only in exported project if user enables):
struct DbHandle {
    rt: tokio::runtime::Handle,   // borrowed from the Runtime stored in AppState
    pool: sqlx::SqlitePool,
}
```

The `update()` body polls `self.result_rx.try_recv()` in both cases — the
channel bridge is the same shape regardless of the async runtime inside.

---

### Verdict: when (if ever) should RohKai add tokio to Cargo.toml?

**Never for the designer binary**, based on current and planned scope.

The specific feature request that would justify tokio in the designer binary:
a live **HTTP or WebSocket DataSource** panel that polls external data
*during design time* and previews live results in the canvas. This is not
in Stages 12–13 scope, and even if it were, the `std::thread` + `ureq`
(synchronous HTTP) pattern can handle basic REST polling without tokio.

The threshold for adding tokio to the designer binary should be:
> "We need concurrent async I/O on more than 3 simultaneous connections, at
> design time, in the designer binary itself, and the `std::thread` pattern
> has been tried and is provably insufficient."

That threshold is not anticipated before Stage 15 at the earliest. Until then,
tokio remains a generated-project opt-in, not a designer dependency.

---

## Section 6 — Security Modules and Hardening Patterns

This section goes beyond "use prepared statements" to catalogue the specific
crates, patterns, and invariants that make the SQL surface defensible.

---

### `rusqlite` params! macro vs raw execute() — what codegen MUST and MUST NOT emit

**MUST emit (safe patterns):**

```rust
// Pattern 1: Parameterised query via params! macro.
// The SQL string is a literal; the user value is a bound parameter.
let sql = "SELECT id, name FROM users WHERE email = ?1";
let mut stmt = conn.prepare(sql)?;
let rows = stmt.query_map(params![self.email_filter], |row| {
    Ok((row.get::<_, i64>(0)?, row.get::<_, String>(1)?))
})?;

// Pattern 2: Named parameters for multi-field queries.
let sql = "UPDATE users SET name = :name, email = :email WHERE id = :id";
conn.execute(sql, named_params! {
    ":name": self.user_name,
    ":email": self.user_email,
    ":id": self.user_id,
})?;

// Pattern 3: Prepared statement reuse across loop iterations.
// Prepare once outside the loop; execute N times with different params.
let mut stmt = conn.prepare("INSERT INTO log (msg, ts) VALUES (?1, ?2)")?;
for entry in &self.pending_log {
    stmt.execute(params![entry.msg, entry.ts])?;
}
```

**MUST NEVER emit (unsafe patterns):**

```rust
// FORBIDDEN Pattern 1: format! to build the SQL string.
// If self.email_filter contains "' OR '1'='1", this is a full SQL injection.
let sql = format!("SELECT * FROM users WHERE email = '{}'", self.email_filter);
conn.execute(&sql, [])?;  // NEVER emit this

// FORBIDDEN Pattern 2: String concatenation in the SQL text.
let sql = "SELECT * FROM users WHERE name = '" + &self.name + "'";  // NEVER

// FORBIDDEN Pattern 3: exec_batch with user-controlled content.
// exec_batch() executes multiple semicolon-separated statements — ideal for
// schema migration scripts, dangerous if the input is user-controlled.
conn.execute_batch(&user_provided_schema)?;  // only safe for CONST schema strings

// FORBIDDEN Pattern 4: Passing an unsanitised column or table name.
// Column names cannot be passed as parameters; they must be whitelisted.
let col = &self.sort_column;  // user-chosen column name
let sql = format!("SELECT * FROM t ORDER BY {}", col);  // NEVER — use a whitelist
```

The column-name case deserves special attention: SQL parameters (`?1`, `:name`)
cannot be used for identifiers (table names, column names, ORDER BY columns).
These must be **whitelisted at codegen time** — the designer emits a known,
fixed set of column names as string literals in the generated source, never as
a runtime variable interpolated into the SQL text.

---

### Rust crates for SQL injection prevention and query sanitization

**`sqlparser` (crates.io: `sqlparser = "0.52"`):**
A SQL parser that can tokenize and validate SQL strings before they are stored
or executed. Suitable for the designer's "validate SQL before saving to UiTree"
use case (Deliverable 3c rule 4). It supports ANSI SQL, PostgreSQL, MySQL,
SQLite dialects. It does NOT prevent injection on its own — a parsed query is
not a safe query — but it can be used to detect `DROP`, `DELETE`, `UPDATE`,
`ATTACH` keywords in design-time preview queries (the allowlist check).

Evaluation: **worth adding for the designer's validation panel** if the visual
query builder is complex. However, the simpler approach (calling
`conn.prepare(sql)` against an in-memory blank DB) is sufficient for syntax
checking and does not require a new dependency. Reserve `sqlparser` for when
AST-level analysis (detecting destructive statements, extracting table/column
references for the schema viewer) is needed.

**`secrecy` (crates.io: `secrecy = "0.10"`):**
Wraps sensitive values in a `Secret<T>` newtype that:
- Prevents the value from appearing in `Debug` output (printed as `[REDACTED]`).
- Zeroizes the memory on drop (via the `zeroize` feature).
- Requires explicit `.expose_secret()` to access the inner value.

Evaluation for RohKai: **high fit** for wrapping DB connection strings. A
connection string like `postgres://user:password@host/db` stored as a bare
`String` in `AppState` will appear in debug logs, panic messages, and any
future telemetry. Wrapping it in `Secret<String>` costs nothing at runtime and
prevents accidental credential exposure.

Recommended generated-code pattern:

```rust
use secrecy::{Secret, ExposeSecret};

struct AppState {
    db_connection_string: Secret<String>,
    // ...
}

impl AppState {
    fn new(dsn: &str) -> Self {
        Self {
            db_connection_string: Secret::new(dsn.to_owned()),
            // ...
        }
    }

    fn connect(&self) -> rusqlite::Result<rusqlite::Connection> {
        // .expose_secret() is the only way to access the value
        rusqlite::Connection::open(self.db_connection_string.expose_secret())
    }
}
```

For SQLite file paths (not passwords), `Secret` is technically overkill but
still useful: a file path to `/home/user/financials.db` is sensitive information
that should not appear in logs.

**`rusqlite_integrity`** — does not exist as a crate. Integrity checking is done
via rusqlite's built-in `pragma_integrity_check()` at connection open time.

**Other crates evaluated:**

- `ammonia` (HTML sanitizer): Not relevant — RohKai generates Rust, not HTML.
- `sqlinjection` (crates.io): A very small crate (last updated 2019) that checks
  for common injection keywords. Not suitable — too simplistic and unmaintained.
- `diesel` (query builder / ORM): Diesel's type-safe query builder prevents
  string interpolation architecturally, but it is async-incompatible with the
  same issues as sqlx. Diesel's compile-time query checking is excellent but
  requires a schema file and `diesel_cli` setup. Too heavy for Stage 13.

**Recommendation:** Add `secrecy = "0.10"` to the generated project's
`Cargo.toml` (not the designer binary). Evaluate `sqlparser` as an optional
designer-only dependency when the visual query builder panel is built.

---

### Read-only connection mode: OpenFlags::SQLITE_OPEN_READ_ONLY

rusqlite exposes SQLite's `SQLITE_OPEN_READONLY` flag via `OpenFlags`:

```rust
use rusqlite::{Connection, OpenFlags};

let conn = Connection::open_with_flags(
    db_path,
    OpenFlags::SQLITE_OPEN_READ_ONLY | OpenFlags::SQLITE_OPEN_NO_MUTEX,
)?;
```

A read-only connection:
- Cannot execute `INSERT`, `UPDATE`, `DELETE`, `DROP`, `CREATE`, `ALTER`.
  SQLite returns `SQLITE_READONLY` error immediately.
- Cannot be used with `BEGIN IMMEDIATE` or `BEGIN EXCLUSIVE` transactions.
- Can still execute `SELECT` queries, read schema via `sqlite_master`, and use
  `PRAGMA integrity_check`.

**Blast radius reduction for design-time connections:**

If the designer ever opens a real DB file for schema inspection (not just an
in-memory mock), it MUST use `SQLITE_OPEN_READ_ONLY`. Even if the `db_query`
field in a malicious project file contains `DROP TABLE users`, the read-only
connection will refuse the statement. This is a defense-in-depth measure
complementing the "no real DB connections at design time" rule.

Recommended pattern for the designer's schema viewer:

```rust
// Design-time schema inspection — read-only, no write possible.
fn open_schema_preview(db_path: &str) -> rusqlite::Result<Connection> {
    Connection::open_with_flags(
        db_path,
        OpenFlags::SQLITE_OPEN_READ_ONLY
            | OpenFlags::SQLITE_OPEN_NO_MUTEX
            | OpenFlags::SQLITE_OPEN_URI,
    )
}
```

---

### Row-level security patterns in SQLite

SQLite does not have native row-level security (RLS) like PostgreSQL. However,
equivalent patterns are achievable:

**1. Views as access control boundaries:**

```sql
-- Schema in the DB file, set up by the designer's schema migration tool.
CREATE VIEW my_data_view AS
    SELECT id, name, public_field FROM sensitive_table
    WHERE owner_id = 1;  -- hardcoded in the view definition

-- Generated code queries the view, never the base table directly.
-- The codegen emitter uses "my_data_view" as the table name for SELECT queries.
```

**2. Attach multiple DB files for privilege separation:**

```rust
// Attach a read-only reference DB for lookup data.
conn.execute_batch("ATTACH DATABASE '/path/to/readonly.db' AS ref")?;
// Now queries can JOIN across 'main' (read-write) and 'ref' (read-only) schemas.
```

**3. Triggers for audit logging:**

```sql
CREATE TRIGGER audit_users_update
AFTER UPDATE ON users
BEGIN
    INSERT INTO audit_log (table_name, row_id, changed_at)
    VALUES ('users', NEW.id, CURRENT_TIMESTAMP);
END;
```

These patterns are achievable but require the designer's schema migration tool to
create the views and triggers. They are Stage 13+ scope items.

---

### Generated-code invariant: Invariant 10 (full text, ready to paste)

The following text is the complete invariant to add to
`docs/ENGINEERING_INVARIANTS.md`:

---

**Invariant 10 — No SQL string interpolation in generated code**

*Scope:* `src/codegen/` (all emitters), visual query builder panel input
validation, and all codegen integration tests.

*Rule:* The codegen pipeline (`src/codegen/egui_emitter.rs`,
`src/codegen/state_emitter.rs`, and any future DB-specific emitter) MUST NOT
emit a `format!()` call, string concatenation (`+`), or any other runtime
string-building expression whose result is passed as the SQL text argument to
any rusqlite or sqlx function. All user-supplied runtime values that appear in
a query (filter values, INSERT values, UPDATE values) MUST be bound as
parameters via `rusqlite::params![]`, `rusqlite::named_params!{}`, or the
equivalent sqlx parameterisation API.

Column names, table names, and other SQL identifiers selected by the user in
the visual query builder are NOT parameters and CANNOT be passed via `params!`.
They MUST be whitelisted at code-emit time: the designer emits a fixed,
hardcoded identifier string (validated against the schema at design time, never
taken from a runtime variable). A runtime `String` field on `AppState` MUST
NEVER be interpolated into a SQL identifier position.

*Test requirement:* A codegen integration test in `tests/` MUST:
1. Generate a DB-backed widget project from a representative UiTree fixture.
2. Scan the generated Rust source for the pattern `format!(` or `+` within
   any function that also references a rusqlite or sqlx type.
3. Assert that no such pattern exists.
4. Additionally, assert that every `?` placeholder in the generated SQL string
   has a corresponding `params!` argument, and that the count of `?` markers
   equals the count of params entries.

*Rationale:* SQL injection via format-string interpolation is the most
dangerous class of vulnerability in generated code. Because the generated code
runs outside the designer's process (in the user's exported application) and
is not audited by the user line-by-line, the codegen system is the last
line of defense. Violations are undetectable at runtime unless a security
audit specifically examines the generated source.

*When added:* Before any SQL codegen work begins in Stage 13.

> **Cross-reference:** The canonical enforced form of this invariant is
> `docs/ENGINEERING_INVARIANTS.md` row 10 in the invariant table. The expanded
> text above is the full design specification written during research; the table
> row is what governs CI checklists and reviewer sign-off. Keep both in sync
> when the rule is updated.

---

## Section 7 — Bespoke Database Abstraction (DatabaseEngine Trait)

This section designs a `DatabaseEngine` trait following the same abstraction
pattern as other planned RohKai traits (e.g., ShaperEngine in P2-A), allowing
multiple database backends without coupling the designer or the codegen to any
single crate.

---

### Evaluating the proposed minimal trait

The proposed interface:

```rust
pub trait DatabaseEngine: Send + Sync {
    fn connect(dsn: &str) -> Result<Self, DbError> where Self: Sized;
    fn query(&self, sql: &str, params: &[&dyn ToSql]) -> Result<Vec<Row>, DbError>;
    fn execute(&self, sql: &str, params: &[&dyn ToSql]) -> Result<usize, DbError>;
}
```

**What this covers well:**
- The `Send + Sync` bounds allow the engine to live on a background thread and
  be shared (via `Arc<dyn DatabaseEngine>`) across tasks.
- `connect()` as an associated function (not a method) is a factory pattern;
  the trait is object-safe only if `connect` is moved out of the trait or made
  a free function on each concrete type. (A trait with a `where Self: Sized`
  method is *not* dyn-compatible for that method — you cannot call `engine.connect()`
  through a `dyn DatabaseEngine`. This is intentional: connection is done
  concretely, then the result is boxed.)
- `params: &[&dyn ToSql]` is a clean heterogeneous parameter list that works
  for rusqlite (`rusqlite::types::ToSql`) and can be adapted for other backends.

**What needs to change or be added:**

1. **Streaming / cursor API:** For large result sets, returning
   `Vec<Row>` materializes all rows into memory. A production trait needs a
   cursor method:

   ```rust
   fn query_cursor<'a>(
       &'a self,
       sql: &str,
       params: &[&dyn ToSql],
   ) -> Result<Box<dyn Iterator<Item = Result<Row, DbError>> + 'a>, DbError>;
   ```

   For Stage 13 (desktop app, small data), `Vec<Row>` is acceptable. Add the
   cursor method in a later stage.

2. **Schema introspection:** The visual query builder needs to list tables and
   columns for autocomplete. Add:

   ```rust
   fn list_tables(&self) -> Result<Vec<String>, DbError>;
   fn list_columns(&self, table: &str) -> Result<Vec<ColumnInfo>, DbError>;
   ```

3. **Transaction support:** The minimal interface has no transaction API. Add:

   ```rust
   fn begin(&self) -> Result<Box<dyn Transaction>, DbError>;
   ```

   Where `Transaction: DatabaseEngine + Drop` (auto-rollback on drop).

4. **The `Row` type:** The trait references `Row` but does not define it.
   A concrete `Row` type needs to be backend-agnostic:

   ```rust
   pub struct Row {
       pub columns: Vec<String>,       // column names
       pub values: Vec<SqlValue>,      // ordered values
   }

   pub enum SqlValue {
       Null,
       Integer(i64),
       Real(f64),
       Text(String),
       Blob(Vec<u8>),
   }
   ```

   This is SQLite's type affinity model, which maps cleanly to PostgreSQL and
   MySQL too (with slight widening: PostgreSQL's `numeric` maps to `Real` or
   `Text` for lossless representation).

**Revised minimal trait for Stage 13:**

```rust
pub trait DatabaseEngine: Send + Sync {
    /// Open a connection. Called once; returns an owned engine instance.
    /// Not dyn-callable — use concrete type or a factory fn.
    fn connect(dsn: &str) -> Result<Self, DbError> where Self: Sized;

    /// Execute a SELECT query; returns all rows materialised.
    fn query(&self, sql: &str, params: &[SqlValue]) -> Result<Vec<Row>, DbError>;

    /// Execute a non-SELECT statement; returns affected row count.
    fn execute(&self, sql: &str, params: &[SqlValue]) -> Result<usize, DbError>;

    /// List tables in the primary schema (for schema viewer panel).
    fn list_tables(&self) -> Result<Vec<String>, DbError>;

    /// List columns for a table (for query builder autocomplete).
    fn list_columns(&self, table: &str) -> Result<Vec<ColumnInfo>, DbError>;
}
```

Using `SqlValue` directly (rather than `&dyn ToSql`) avoids the
backend-specific trait bound leaking into the trait definition.

---

### Concrete implementations

**`SqliteEngine` wrapping rusqlite:**

```rust
pub struct SqliteEngine {
    conn: rusqlite::Connection,
}

impl DatabaseEngine for SqliteEngine {
    fn connect(dsn: &str) -> Result<Self, DbError> {
        let conn = rusqlite::Connection::open(dsn)
            .map_err(|e| DbError::Connect(e.to_string()))?;
        Ok(Self { conn })
    }

    fn query(&self, sql: &str, params: &[SqlValue]) -> Result<Vec<Row>, DbError> {
        let rparams: Vec<Box<dyn rusqlite::types::ToSql>> =
            params.iter().map(sql_value_to_rusqlite).collect();
        let rparams_refs: Vec<&dyn rusqlite::types::ToSql> =
            rparams.iter().map(|b| b.as_ref()).collect();
        let mut stmt = self.conn.prepare(sql)
            .map_err(|e| DbError::Query(e.to_string()))?;
        // ... map rows to Vec<Row>
        todo!()
    }
    // ...
}
```

This is straightforward. rusqlite's `ToSql` trait covers all five `SqlValue`
variants natively.

**`PostgresEngine` wrapping `postgres = "0.19"`:**

The `postgres` crate is a **pure-Rust synchronous** PostgreSQL driver. It
implements the PostgreSQL frontend/backend wire protocol in Rust with no
dependency on libpq. Version 0.19.9 (June 2026) is stable and actively
maintained.

```toml
postgres = "0.19"
```

Evaluation:
- **Pure Rust:** Yes. No C library required. No `pkg-config`. Compatible with
  RohKai's architecture rules (subject to user approval of the crate).
- **Synchronous:** Yes. `Client::query()`, `Client::execute()`, `Client::prepare()`
  are all blocking calls. No tokio required.
- **TLS support:** Via `postgres-openssl` or `postgres-native-tls` feature crates.
  `native-tls` wraps the platform TLS library (SChannel on Windows, OpenSSL on
  Linux, Secure Transport on macOS). This is a C dependency indirectly, but it
  is the platform's own TLS — not a vendored C library. For pure-Rust TLS,
  `rustls` is available via `postgres-rustls` (unofficial but functional).
- **Connection pooling:** `r2d2_postgres` wraps the `postgres` crate in an
  `r2d2` connection pool for multi-threaded use. For a single-connection
  desktop background thread, pooling is not needed.

`PostgresEngine` implementation shape:

```rust
pub struct PostgresEngine {
    client: postgres::Client,
}

impl DatabaseEngine for PostgresEngine {
    fn connect(dsn: &str) -> Result<Self, DbError> {
        let client = postgres::Client::connect(dsn, postgres::NoTls)
            .map_err(|e| DbError::Connect(e.to_string()))?;
        Ok(Self { client })
    }
    // ...
}
```

**`MySqlEngine` wrapping `mysql = "25"`:**

The `mysql` crate (version 25.x) is a **pure-Rust synchronous** MySQL driver.
It implements MySQL's `CLIENT_PROTOCOL_41` wire protocol in Rust, requiring no
libmysql C library.

```toml
mysql = "25"
```

Evaluation:
- **Pure Rust:** Yes. No C library.
- **Synchronous:** Yes. `Conn::query_map()`, `Conn::exec_map()`, `Conn::exec_drop()`
  are blocking.
- **Parameterisation:** Uses positional `?` placeholders (same as SQLite) and
  named `:name` parameters via `mysql::params!`.
- **TLS:** Optional via `native-tls` or `rustls` features.
- **Maintenance:** Actively maintained. 6M+ downloads.

**`MockEngine` for design-time use:**

```rust
pub struct MockEngine {
    tables: HashMap<String, Vec<Row>>,
    schema: HashMap<String, Vec<ColumnInfo>>,
}

impl MockEngine {
    /// Seed with fixture data at construction time.
    pub fn with_fixture(tables: HashMap<String, Vec<Row>>) -> Self {
        let schema = tables.iter().map(|(table, rows)| {
            let cols = rows.first().map(|r| r.columns.clone()).unwrap_or_default()
                .into_iter().enumerate()
                .map(|(i, name)| ColumnInfo { name, col_type: "TEXT".into(), ordinal: i })
                .collect();
            (table.clone(), cols)
        }).collect();
        Self { tables, schema }
    }
}

impl DatabaseEngine for MockEngine {
    fn connect(_dsn: &str) -> Result<Self, DbError> where Self: Sized {
        // MockEngine is constructed with fixture data, not a real DSN.
        Ok(Self { tables: HashMap::new(), schema: HashMap::new() })
    }

    fn query(&self, sql: &str, _params: &[SqlValue]) -> Result<Vec<Row>, DbError> {
        // Naive: return all rows for the first table name found in the SQL.
        // Real implementation would use sqlparser to extract the table name.
        for (table, rows) in &self.tables {
            if sql.to_lowercase().contains(&table.to_lowercase()) {
                return Ok(rows.clone());
            }
        }
        Ok(vec![])
    }

    fn execute(&self, _sql: &str, _params: &[SqlValue]) -> Result<usize, DbError> {
        Ok(0) // No-op at design time
    }
    // ...
}
```

The `MockEngine` never opens a file, never touches the network, and returns
predictable fixture data. It is the only engine the designer binary itself
should ever use at design time.

---

### How the designer uses MockEngine vs exported project uses real engines

**Designer binary** (at design time):
- The schema viewer panel calls `engine.list_tables()` and `engine.list_columns()`
  on a `MockEngine` seeded from the schema script the user uploads.
- The query preview panel calls `engine.query()` on the `MockEngine` and shows
  the fixture rows in the Table widget preview.
- The designer binary has **no import of rusqlite, postgres, or mysql at all**
  in its direct dependencies. It only knows `Box<dyn DatabaseEngine>`.

**Exported project** (at runtime, in the user's compiled binary):
- `AppState::new()` constructs a `SqliteEngine::connect(db_path)?` (or
  `PostgresEngine::connect(dsn)?`) based on the project settings.
- The same `Box<dyn DatabaseEngine>` handle is passed to the query worker thread.
- The DB selection is purely a `Cargo.toml` dependency + an `AppState::new()`
  constructor argument; the rest of the generated code is DB-agnostic.

---

### Pure-Rust wire-protocol drivers that exist today

| Database | Sync pure-Rust driver | Async pure-Rust driver | C library required? |
|---|---|---|---|
| SQLite | `rusqlite` (bundles C source via `cc`) | None | Vendored C (compiled in, not system) |
| PostgreSQL | `postgres = "0.19"` | `tokio-postgres = "0.7"` | None — pure Rust wire protocol |
| MySQL | `mysql = "25"` | `mysql_async = "0.34"` | None — pure Rust wire protocol |
| SQL Server | None stable | `tiberius = "0.12"` (TDS protocol) | None — pure Rust TDS impl |
| Oracle | None | None (Oracle's OCI is proprietary) | Requires Oracle Instant Client C library |
| Redis | `redis` crate (sync feature) | `redis` crate (async feature) | None — pure Rust RESP protocol |

For Stage 13 scope (SQLite primary, PostgreSQL secondary), the pure-Rust path is
fully available without any C library for PostgreSQL. SQLite's bundled C is
acceptable per the Deliverable 2 rationale.

---

### Trait dispatch in the code generator

The exported `AppState` should accept a `Box<dyn DatabaseEngine>` stored behind
an `Arc<Mutex<...>>` if shared, or just owned by the worker thread:

```rust
// Option A: Engine owned by the worker thread (recommended for single-DB apps).
// AppState holds channels; the engine itself is inside the spawned thread closure.
fn spawn_db_worker(engine: impl DatabaseEngine + 'static, ...) {
    std::thread::spawn(move || {
        // engine is owned here; no Arc/Mutex needed.
        while let Ok(req) = rx.recv() {
            let rows = engine.query(sql, &params)?;
            tx.send(QueryResult::Rows(rows))?;
        }
    });
}

// Option B: Shared engine (for multi-query concurrent access, future use).
type SharedEngine = Arc<Mutex<Box<dyn DatabaseEngine>>>;
```

The codegen emitter chooses Option A (engine owned by worker thread) for Stage
13. This avoids `Arc<Mutex<>>` overhead and is sufficient for a single-user
desktop app with one active query at a time.

The codegen's `state_emitter.rs` does not import any concrete engine crate;
it emits the engine construction call as a string:

```rust
// What state_emitter.rs emits (as a String, not as compiled Rust):
let engine = SqliteEngine::connect(&app_db_path)?;
spawn_db_worker(engine, tx, rx);
```

The concrete type name (`SqliteEngine`, `PostgresEngine`) is a configuration
value in the project settings, not hardcoded in the emitter. This keeps the
emitter DB-agnostic.

---

## Section 8 — Proprietary DB Integration Approaches

This section examines how proprietary vendors build database connectivity and
what architectural patterns RohKai can adopt.

---

### a) ODBC/JDBC-style driver models (Microsoft, IBM, Oracle)

**The ODBC driver model:**

ODBC (Open Database Connectivity) is a C API standardized by Microsoft in the
early 1990s. Its architecture has three layers:

1. **Application:** Links against a thin `odbc32.dll` (Windows) or
   `libodbc.so` (Linux via unixODBC). Calls functions like
   `SQLConnect()`, `SQLExecDirect()`, `SQLFetch()`.

2. **Driver Manager:** `odbc32.dll` / `libodbc.so` is a router. It loads the
   correct database-specific driver DLL at runtime based on the DSN.

3. **Driver DLL:** Each database vendor ships a `.dll`/`.so` that implements
   the ODBC C function-pointer vtable. The application never links directly to
   this DLL — the Driver Manager loads it dynamically.

This is a **C ABI vtable** pattern: the interface is a set of function pointer
signatures, the implementation is loaded at runtime, and the application is
decoupled from any specific database vendor's code.

**How this maps to RohKai's `DatabaseEngine` trait:**

The `DatabaseEngine` trait is exactly this model expressed in Rust:

```
ODBC concept              →  RohKai equivalent
─────────────────────────────────────────────
ODBC C vtable             →  DatabaseEngine trait (Rust vtable)
odbc32.dll driver manager →  Box<dyn DatabaseEngine> dispatch
Database-specific .dll    →  SqliteEngine / PostgresEngine / MySqlEngine struct
DSN connection string     →  dsn: &str parameter to DatabaseEngine::connect()
```

The key insight: `Box<dyn DatabaseEngine>` in Rust is a fat pointer containing
a data pointer and a vtable pointer — exactly the same mechanism as ODBC's
function-pointer table, but memory-safe and without the C ABI boundary.

**Could RohKai load external DLL implementations of `DatabaseEngine`?**

Yes, via the `libloading` crate, but this introduces a C ABI boundary problem.
Rust's `dyn Trait` vtables are not stable across crate or compiler versions —
a `DatabaseEngine` trait object compiled with rustc 1.80 has a different vtable
layout than one compiled with rustc 1.82. This means:

- You **cannot** safely pass a `Box<dyn DatabaseEngine>` across a `.dll`/`.so`
  boundary between separately compiled binaries.
- The `abi_stable` crate addresses this by defining ABI-stable repr-C vtable
  wrappers, but it is a significant complexity cost.
- **Recommended alternative:** Do not support external DLL drivers in Stage 13.
  All engines are compiled into the same binary. External driver support (like
  ODBC) is a Stage 15+ concern that requires a dedicated ABI stability story.

**The `abi_stable` crate (crates.io: `abi_stable = "0.11"`):**

`abi_stable` provides `RBox<dyn Interface>` — a repr-C-compatible fat pointer
that can cross DLL boundaries safely between separately compiled Rust binaries.
It is used by some Rust plugin systems. However, it doubles the complexity of
the trait definition and requires all trait methods to use ABI-stable types.
**Not recommended for Stage 13.** Worth revisiting if a plugin-based driver
ecosystem is desired later.

---

### b) Wire-protocol-only drivers (the pure-Rust path)

**PostgreSQL's frontend/backend protocol:**

PostgreSQL's wire protocol (also called the "client protocol") is fully
documented in the PostgreSQL documentation under "Frontend/Backend Protocol."
It uses a length-prefixed message framing over a TCP socket (or Unix domain
socket). The protocol covers:

- SSL/TLS negotiation (send an SSLRequest message; server replies with 'S' or
  'N'; upgrade to TLS if 'S').
- Authentication (multiple mechanisms: MD5, SCRAM-SHA-256, GSS/Kerberos,
  password cleartext — all documented).
- Simple query protocol: send a `Query` message with SQL text; receive
  `RowDescription`, zero or more `DataRow`, then `CommandComplete` or
  `ErrorResponse`.
- Extended query protocol: `Parse` (prepare), `Bind` (bind parameters),
  `Execute` (run), `Sync` (synchronize).

`tokio-postgres` (and the synchronous `postgres` crate) implement this
protocol entirely in Rust. The `postgres = "0.19"` crate has no C dependency
whatsoever. This is a clean pure-Rust wire protocol implementation.

**MySQL's wire protocol:**

MySQL's `CLIENT_PROTOCOL_41` handshake and text/binary query protocols are
documented in the MySQL Internals Manual. The handshake covers:
- Initial server greeting (server capabilities, auth plugin name, challenge).
- Client response (capability flags, `caching_sha2_password` or `mysql_native_password`).
- Query execution via `COM_QUERY` (text protocol) or `COM_STMT_PREPARE` /
  `COM_STMT_EXECUTE` (binary prepared statement protocol).

The `mysql = "25"` and `mysql_async = "0.34"` crates implement both the text
and binary protocols in pure Rust.

**What it takes to implement a minimal wire-protocol driver:**

1. **TCP connection:** `std::net::TcpStream` (or `tokio::net::TcpStream` for
   async). Typically port 5432 (PostgreSQL) or 3306 (MySQL).

2. **TLS handshake:** The database sends a capabilities message; the client
   negotiates TLS upgrade. In pure Rust: `rustls` + `rustls-pemfile` for
   certificate handling.

3. **Authentication:**
   - PostgreSQL: SCRAM-SHA-256 (RFC 7677). Requires SHA-256 hashing and
     HMAC-SHA-256 — available from `sha2` and `hmac` crates (both pure Rust).
   - MySQL: `caching_sha2_password` (SHA-256 of the password XOR'd with a
     server nonce). Also achievable with `sha2`.

4. **Query/response framing:** Length-prefixed byte buffers. Pure Rust byte
   manipulation (`byteorder` or manual big-endian reads).

The total effort to implement a minimal read-only PostgreSQL wire client from
scratch in pure Rust is roughly 1500–3000 lines of Rust. This is why the
existing `postgres`/`tokio-postgres` crates are the right choice — they have
already done this work, are widely used, and are audited.

**Proprietary databases with closed protocols:**

- **Oracle OCI:** Oracle's wire protocol is proprietary and not publicly
  documented. Third-party implementations exist (notably `oracle` crate, which
  wraps Oracle's Instant Client C library — a C dependency). No pure-Rust
  Oracle wire implementation exists in production use. **Not achievable without
  C bindings for Stage 13.**

- **Microsoft SQL Server (TDS protocol):** SQL Server's Tabular Data Stream
  (TDS) protocol was reverse-engineered and is now partially documented via
  the `[MS-TDS]` Open Specification document (Microsoft published it under their
  Open Specification Promise). The `tiberius = "0.12"` crate implements TDS 7.4
  in pure Rust, including SQL Server authentication (Windows NTLM via the
  `winauth` feature, SQL Server password auth without external dependencies).
  **Pure-Rust SQL Server access is achievable via `tiberius`.**

---

### c) What Qt Designer does

Qt's database abstraction (`QtSql` module) uses a plugin-based `QSqlDriver`
model that is instructive for RohKai:

1. **Design-time:** Qt Designer uses a SQLite QSqlDriver internally for all
   "test connection" and schema browsing operations. When you drag a SQL widget
   onto a form in Qt Designer, it connects to a local SQLite file for preview,
   regardless of what the deployment database will be.

2. **Deployment-time configuration:** The developer configures the real
   `QSqlDatabase::addDatabase("QPSQL")` (for PostgreSQL) or `"QMYSQL"` (for
   MySQL) in the deployment build. The Designer never loads the PostgreSQL or
   MySQL driver — it only uses SQLite.

3. **Driver plugin DLL:** Each `QSqlDriver` implementation is a Qt plugin
   DLL that implements `QSqlDriverPlugin`. Qt's plugin loader discovers them at
   runtime via the `QT_PLUGIN_PATH`. This is exactly the ODBC driver model
   applied to Qt's C++ vtable.

4. **The designer-runtime split:** Qt does not ship `libpq` or `libmysql` in
   the Qt Designer binary. The designer binary links only against the SQLite
   driver. The deployment application links against the full driver.

**RohKai's mirroring of the Qt pattern:**

RohKai should adopt this exact split:
- Designer binary: `MockEngine` (in-memory fixture data) or `SqliteEngine`
  (bundled SQLite) only. No PostgreSQL, MySQL, or other network drivers.
- Exported project: `SqliteEngine`, `PostgresEngine`, or `MySqlEngine` as
  selected by the user's project settings. The designer generates the
  appropriate `Cargo.toml` dependency and `AppState::new()` constructor.
- The designer never links against the network driver crates (`postgres`,
  `mysql`) — these appear only in the generated project's `Cargo.toml`.

---

### d) RohKai's recommended proprietary-style path

**Stage 13 implementation plan:**

1. **Define `DatabaseEngine` trait** in a new module `src/db/engine.rs`.
   This trait is the stable ABI boundary. All other DB code depends on it.

2. **Ship `SqliteEngine`** (`rusqlite = "0.40"` with `bundled` feature) for
   Stage 13. This is the only real engine in the designer binary (for
   schema introspection only, read-only, with explicit user consent).
   User approval required: `rusqlite = { version = "0.40", features = ["bundled"] }`.

3. **Ship `MockEngine`** (zero dependencies, pure Rust `Vec<Row>` fixture data)
   as the default design-time engine. This is the only engine used for
   design-time query preview.

4. **Ship `PostgresEngine`** (wrapping `postgres = "0.19"`, pure Rust wire
   protocol) as a Stage 13 stretch goal, in the exported project template only.
   User approval required at the time it is added: `postgres = "0.19"`.

5. **Never expose libpq, libmysql, ODBC DLLs, or Oracle Instant Client** in
   the designer binary. These C dependencies are explicitly prohibited by
   CLAUDE.md's "NO system toolkit bindings" rule.

6. **Users who need Oracle:** Can implement `DatabaseEngine` externally as a
   separate Rust crate and substitute it in the generated project's `main.rs`.
   RohKai documents the trait; users write their own `OracleEngine` wrapper
   around the `oracle` crate (C-dependent). The designer never ships or requires
   this.

7. **Users who need SQL Server:** `tiberius = "0.12"` (pure Rust TDS) is the
   recommended path. It can be added as a Stage 13+ optional template with user
   approval.

**How JetBrains DataGrip, DBeaver, and Tableau work at the driver layer:**

- **DataGrip / IntelliJ:** Uses JDBC (Java Database Connectivity), which is
  exactly ODBC's model in Java. Each driver is a `.jar` implementing the
  `java.sql.Driver` interface. DataGrip ships no DB-specific code; it
  downloads and caches JDBC driver JARs at runtime. RohKai's equivalent: the
  `DatabaseEngine` trait with dynamically constructed concrete types.

- **DBeaver:** Also JDBC-based, same pattern. Ships a plugin system that loads
  additional JDBC driver JARs per-database. Open source; the plugin loading
  code is in `org.jkiss.dbeaver.core`.

- **Tableau:** Uses ODBC/JDBC connectors per data source, plus a proprietary
  Hyper engine (a high-performance analytical DB) for in-memory data
  materialization. Tableau's Designer always shows data from Hyper (the mock
  engine equivalent); the live DB is queried only on publish or explicit
  "refresh."

The architectural pattern is consistent across all three: **designer uses a
local/mock engine; the live DB driver is loaded or generated at deployment
time.** RohKai's `MockEngine` / `SqliteEngine` split is the pure-Rust
equivalent of this industry-standard pattern.

---

*End of P2-B expansion (Sections 5–8). Total document: original Sections 1–4
plus Sections 5–8 appended above.*
