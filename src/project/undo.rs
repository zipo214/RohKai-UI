//! Undo/redo stack for the `UiTree`.
//!
//! Snapshots are serialized `.rohkai.json` strings (the same canonical form used
//! for dirty tracking and save), so a snapshot round-trips exactly through
//! `io::deserialize`.  The stack is capped at `CAP` steps.
//!
//! Commit boundaries are decided by the caller: `record()` is called once per
//! meaningful, settled change (e.g. after a drag releases, a widget is added,
//! or a property edit lands).  In-progress drags are coalesced because the
//! caller defers `record()` until the pointer is up.

const CAP: usize = 50;

/// Undo/redo history of serialized `UiTree` snapshots.
pub struct UndoStack {
    /// Past states, oldest first; the last entry is the most recent committed snapshot.
    undo: Vec<String>,
    /// Future states (populated by undo), most-recently-undone last.
    redo: Vec<String>,
    /// The snapshot currently reflected on the canvas (the "present").
    current: Option<String>,
}

impl Default for UndoStack {
    fn default() -> Self {
        Self::new()
    }
}

impl UndoStack {
    pub fn new() -> Self {
        Self {
            undo: Vec::new(),
            redo: Vec::new(),
            current: None,
        }
    }

    /// Re-seed the stack to a single baseline state, clearing history.
    /// Called on new project / open / load.
    pub fn reset(&mut self, baseline_json: String) {
        self.undo.clear();
        self.redo.clear();
        self.current = Some(baseline_json);
    }

    /// Update the "present" snapshot without disturbing history.
    /// Used after an undo/redo restore so the next `record()` diffs against the
    /// state actually on the canvas.
    pub fn sync_current(&mut self, json: String) {
        self.current = Some(json);
    }

    /// Record a new committed snapshot.  No-op if unchanged from `current`.
    /// Pushes the previous `current` onto the undo history and clears redo.
    pub fn record(&mut self, new_json: String) {
        if self.current.as_deref() == Some(new_json.as_str()) {
            return; // nothing changed
        }
        if let Some(prev) = self.current.take() {
            self.undo.push(prev);
            if self.undo.len() > CAP {
                self.undo.remove(0);
            }
        }
        self.redo.clear();
        self.current = Some(new_json);
    }

    pub fn can_undo(&self) -> bool {
        !self.undo.is_empty()
    }

    pub fn can_redo(&self) -> bool {
        !self.redo.is_empty()
    }

    /// Step back one state.  Returns the JSON to restore, or `None` if at the
    /// oldest state.  The current state moves onto the redo stack.
    pub fn undo(&mut self) -> Option<String> {
        let prev = self.undo.pop()?;
        if let Some(cur) = self.current.take() {
            self.redo.push(cur);
        }
        self.current = Some(prev.clone());
        Some(prev)
    }

    /// Step forward one state.  Returns the JSON to restore, or `None` if at the
    /// newest state.  The current state moves back onto the undo stack.
    pub fn redo(&mut self) -> Option<String> {
        let next = self.redo.pop()?;
        if let Some(cur) = self.current.take() {
            self.undo.push(cur);
        }
        self.current = Some(next.clone());
        Some(next)
    }
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn record_pushes_history_and_clears_redo() {
        let mut s = UndoStack::new();
        s.reset("A".to_owned());
        s.record("B".to_owned());
        s.record("C".to_owned());
        assert!(s.can_undo());
        assert!(!s.can_redo());
    }

    #[test]
    fn record_noop_when_unchanged() {
        let mut s = UndoStack::new();
        s.reset("A".to_owned());
        s.record("A".to_owned());
        assert!(!s.can_undo(), "identical snapshot must not create history");
    }

    #[test]
    fn undo_then_redo_round_trips() {
        let mut s = UndoStack::new();
        s.reset("A".to_owned());
        s.record("B".to_owned());
        s.record("C".to_owned());

        assert_eq!(s.undo(), Some("B".to_owned()));
        assert_eq!(s.undo(), Some("A".to_owned()));
        assert_eq!(s.undo(), None, "cannot undo past oldest");

        assert_eq!(s.redo(), Some("B".to_owned()));
        assert_eq!(s.redo(), Some("C".to_owned()));
        assert_eq!(s.redo(), None, "cannot redo past newest");
    }

    #[test]
    fn record_after_undo_clears_redo_branch() {
        let mut s = UndoStack::new();
        s.reset("A".to_owned());
        s.record("B".to_owned());
        s.undo(); // back to A
        assert!(s.can_redo());
        s.record("D".to_owned()); // new branch
        assert!(!s.can_redo(), "new edit must drop the redo branch");
        assert_eq!(s.undo(), Some("A".to_owned()));
    }

    #[test]
    fn history_is_capped() {
        let mut s = UndoStack::new();
        s.reset("0".to_owned());
        for i in 1..=(CAP + 10) {
            s.record(i.to_string());
        }
        // Undo as far as possible; should not exceed CAP steps.
        let mut steps = 0;
        while s.undo().is_some() {
            steps += 1;
        }
        assert!(steps <= CAP, "undo depth {steps} exceeded cap {CAP}");
    }
}
