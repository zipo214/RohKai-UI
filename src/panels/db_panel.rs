//! Stage 13 — Database panel.
//!
//! Provides a floating window for:
//!  - Connecting to a SQLite database file (or `:memory:` for testing).
//!  - Browsing the schema tree (tables → columns).
//!  - Previewing rows via a simple query builder (table + limit).
//!
//! All SQL is issued through the [`DatabaseEngine`] trait; this panel never
//! imports `rusqlite` directly.

use crate::project::db_engine::{ColumnInfo, DatabaseEngine, DbRow, SqliteEngine};

// ---------------------------------------------------------------------------
// Panel state (owned by RohKaiApp)
// ---------------------------------------------------------------------------

/// Persistent state for the DB panel window.
pub struct DbPanelState {
    /// Path text field contents.
    pub path_buf: String,
    /// Status message shown below the connect row.
    pub status: Option<(bool, String)>,
    /// Expanded table in the schema tree (if any).
    pub expanded_table: Option<String>,
    /// Table selected in the query builder.
    pub selected_table: String,
    /// Cached table list (repopulated after every successful connect/refresh).
    pub tables: Vec<String>,
    /// Cached column list for the selected table.
    pub columns: Vec<ColumnInfo>,
    /// Preview rows for the last query.
    pub preview_rows: Vec<DbRow>,
    /// Column headers for the preview grid.
    pub preview_headers: Vec<String>,
    /// Preview row limit (spinner).
    pub preview_limit: usize,
    /// True while the window is open.
    pub open: bool,
}

impl Default for DbPanelState {
    fn default() -> Self {
        Self {
            path_buf: String::new(),
            status: None,
            expanded_table: None,
            selected_table: String::new(),
            tables: Vec::new(),
            columns: Vec::new(),
            preview_rows: Vec::new(),
            preview_headers: Vec::new(),
            preview_limit: 10,
            open: false,
        }
    }
}

// ---------------------------------------------------------------------------
// Public entry point
// ---------------------------------------------------------------------------

/// Show the Database floating window.
///
/// `engine` is `Option<Box<dyn DatabaseEngine>>` stored on `RohKaiApp`.
/// Pass `&mut state.open` as the bool carried by `egui::Window`.
pub fn show_window(
    ctx: &egui::Context,
    state: &mut DbPanelState,
    engine: &mut Option<Box<dyn DatabaseEngine>>,
) {
    if !state.open {
        return;
    }

    let mut open = state.open;
    egui::Window::new("Database")
        .open(&mut open)
        .default_size([480.0, 420.0])
        .resizable(true)
        .show(ctx, |ui| {
            show_content(ui, state, engine);
        });
    state.open = open;
}

// ---------------------------------------------------------------------------
// Inner content
// ---------------------------------------------------------------------------

fn show_content(
    ui: &mut egui::Ui,
    state: &mut DbPanelState,
    engine: &mut Option<Box<dyn DatabaseEngine>>,
) {
    // --- Connection row ---
    ui.heading("Connection");
    ui.horizontal(|ui| {
        ui.label("Path:");
        ui.add(
            egui::TextEdit::singleline(&mut state.path_buf)
                .hint_text("path/to/file.db  or  :memory:")
                .desired_width(280.0),
        );
        if ui.button("Connect").clicked() {
            do_connect(state, engine);
        }
        if engine.is_some() && ui.button("Refresh").clicked() {
            refresh_schema(state, engine);
        }
    });

    if let Some((ok, ref msg)) = state.status {
        let color = if ok {
            egui::Color32::from_rgb(52, 211, 153)
        } else {
            egui::Color32::from_rgb(248, 113, 113)
        };
        ui.colored_label(color, msg);
    }

    if engine.is_none() {
        return;
    }

    ui.separator();

    // --- Schema tree ---
    ui.heading("Schema");
    egui::ScrollArea::vertical()
        .id_salt("db_schema_scroll")
        .max_height(160.0)
        .show(ui, |ui| {
            show_schema_tree(ui, state, engine);
        });

    ui.separator();

    // --- Query builder ---
    ui.heading("Query Builder");
    show_query_builder(ui, state, engine);
}

fn do_connect(state: &mut DbPanelState, engine: &mut Option<Box<dyn DatabaseEngine>>) {
    let path = state.path_buf.trim().to_owned();
    if path.is_empty() {
        state.status = Some((false, "Enter a database path first.".to_owned()));
        return;
    }
    let mut new_engine: Box<dyn DatabaseEngine> = Box::new(SqliteEngine::new());
    match new_engine.connect(&path) {
        Ok(()) => {
            *engine = Some(new_engine);
            state.status = Some((true, format!("Connected to: {path}")));
            refresh_schema(state, engine);
        }
        Err(e) => {
            state.status = Some((false, format!("Connection failed: {e}")));
            *engine = None;
        }
    }
}

fn refresh_schema(state: &mut DbPanelState, engine: &mut Option<Box<dyn DatabaseEngine>>) {
    let Some(eng) = engine.as_ref() else {
        return;
    };
    match eng.list_tables() {
        Ok(tables) => {
            state.tables = tables;
            if !state.tables.is_empty() && state.selected_table.is_empty() {
                state.selected_table = state.tables[0].clone();
            }
        }
        Err(e) => {
            state.status = Some((false, format!("Schema error: {e}")));
            state.tables.clear();
        }
    }
}

fn show_schema_tree(
    ui: &mut egui::Ui,
    state: &mut DbPanelState,
    engine: &mut Option<Box<dyn DatabaseEngine>>,
) {
    if state.tables.is_empty() {
        ui.weak("No tables found.");
        return;
    }

    let tables = state.tables.clone();
    for table in &tables {
        let expanded = state.expanded_table.as_deref() == Some(table.as_str());
        let header = egui::RichText::new(format!("  {table}")).monospace();
        let resp = ui.selectable_label(expanded, header);
        if resp.clicked() {
            if expanded {
                state.expanded_table = None;
            } else {
                // Load columns on expand
                if let Some(eng) = engine.as_ref() {
                    match eng.list_columns(table) {
                        Ok(cols) => {
                            state.columns = cols;
                            state.expanded_table = Some(table.clone());
                        }
                        Err(e) => {
                            state.status = Some((false, format!("Column error: {e}")));
                        }
                    }
                }
            }
        }

        if expanded {
            for col in &state.columns {
                let pk_mark = if col.pk { " PK" } else { "" };
                let nn_mark = if col.not_null { " NOT NULL" } else { "" };
                ui.indent("db_col_indent", |ui| {
                    ui.weak(format!(
                        "{} : {}{}{}",
                        col.name, col.type_name, pk_mark, nn_mark
                    ));
                });
            }
        }
    }
}

fn show_query_builder(
    ui: &mut egui::Ui,
    state: &mut DbPanelState,
    engine: &mut Option<Box<dyn DatabaseEngine>>,
) {
    ui.horizontal(|ui| {
        ui.label("Table:");
        let tables = state.tables.clone();
        let current = state.selected_table.clone();
        egui::ComboBox::from_id_salt("db_qb_table")
            .selected_text(&current)
            .show_ui(ui, |ui| {
                for t in &tables {
                    ui.selectable_value(&mut state.selected_table, t.clone(), t);
                }
            });

        ui.label("Limit:");
        let mut limit_str = state.preview_limit.to_string();
        if ui
            .add(
                egui::TextEdit::singleline(&mut limit_str)
                    .desired_width(48.0)
                    .hint_text("10"),
            )
            .changed()
        {
            if let Ok(n) = limit_str.trim().parse::<usize>() {
                state.preview_limit = n.clamp(1, 1000);
            }
        }

        if ui.button("Preview").clicked() {
            do_preview(state, engine);
        }
    });

    if state.preview_rows.is_empty() && state.preview_headers.is_empty() {
        return;
    }

    // --- Row preview grid ---
    ui.separator();
    let row_count = state.preview_rows.len();
    let col_count = state.preview_headers.len();
    if col_count == 0 {
        return;
    }

    egui::ScrollArea::both()
        .id_salt("db_preview_scroll")
        .max_height(180.0)
        .show(ui, |ui| {
            egui::Grid::new("db_preview_grid")
                .striped(true)
                .num_columns(col_count)
                .show(ui, |ui| {
                    // Header row
                    for h in &state.preview_headers {
                        ui.strong(h);
                    }
                    ui.end_row();

                    // Data rows
                    for row in &state.preview_rows {
                        for cell in &row.cells {
                            ui.monospace(cell);
                        }
                        ui.end_row();
                    }
                    // Guard against unused variable warning for row_count.
                    let _ = row_count;
                });
        });
}

fn do_preview(state: &mut DbPanelState, engine: &mut Option<Box<dyn DatabaseEngine>>) {
    let table = state.selected_table.trim().to_owned();
    if table.is_empty() {
        state.status = Some((false, "Select a table first.".to_owned()));
        return;
    }

    let Some(eng) = engine.as_ref() else {
        state.status = Some((false, "Not connected.".to_owned()));
        return;
    };

    // Fetch column names for header row
    match eng.list_columns(&table) {
        Ok(cols) => {
            state.preview_headers = cols.iter().map(|c| c.name.clone()).collect();
        }
        Err(e) => {
            state.status = Some((false, format!("Column error: {e}")));
            return;
        }
    }

    // Fetch rows
    match eng.preview_rows(&table, state.preview_limit) {
        Ok(rows) => {
            state.preview_rows = rows;
            state.status = Some((
                true,
                format!("Showing {} rows from '{table}'", state.preview_rows.len()),
            ));
        }
        Err(e) => {
            state.status = Some((false, format!("Query error: {e}")));
            state.preview_rows.clear();
        }
    }
}
