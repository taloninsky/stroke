//! Session-local stroke history for undo and redo.
//!
//! History deliberately wraps only committed strokes and is never serialized.
//! The document remains the source of truth for the currently visible stroke
//! list; the redo stack is transient UI state for the current editing session.

use crate::document::{Document, Stroke};

/// Session-local undo/redo state for committed strokes.
#[derive(Debug, Default)]
pub struct StrokeHistory {
    redo_stack: Vec<Stroke>,
}

impl StrokeHistory {
    /// Create an empty history stack.
    pub fn new() -> Self {
        Self::default()
    }

    /// Record that a new user stroke was committed.
    ///
    /// Drawing after undo makes the old redo path invalid, so the redo stack
    /// is cleared whenever capture commits a fresh stroke.
    pub fn record_new_stroke(&mut self) {
        self.redo_stack.clear();
    }

    /// Remove the most recent visible stroke and make it redoable.
    ///
    /// Returns `true` when the document changed and `false` for an empty
    /// document no-op.
    pub fn undo(&mut self, document: &mut Document) -> bool {
        let Some(stroke) = document.strokes.pop() else {
            return false;
        };
        self.redo_stack.push(stroke);
        true
    }

    /// Restore the most recently undone stroke.
    ///
    /// Returns `true` when the document changed and `false` for an empty redo
    /// stack no-op.
    pub fn redo(&mut self, document: &mut Document) -> bool {
        let Some(stroke) = self.redo_stack.pop() else {
            return false;
        };
        document.strokes.push(stroke);
        true
    }

    /// Clear all transient history, used after Open and New.
    pub fn clear(&mut self) {
        self.redo_stack.clear();
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::document::{Canvas, Point, PointerType};

    fn stroke(id: u32) -> Stroke {
        Stroke {
            id,
            author_id: "local-user".to_string(),
            tool: "pen".to_string(),
            color: "#000000".to_string(),
            width: 2.0,
            started_at: chrono::Utc::now(),
            pointer_type: PointerType::Mouse,
            points: vec![
                Point {
                    x: id as f64,
                    y: 1.0,
                    t: 0.0,
                },
                Point {
                    x: id as f64 + 1.0,
                    y: 2.0,
                    t: 16.0,
                },
            ],
        }
    }

    fn document_with_strokes(count: u32) -> Document {
        let mut document = Document::new(1, Canvas::css_px(800.0, 600.0));
        document.strokes = (1..=count).map(stroke).collect();
        document.next_id = count + 1;
        document
    }

    #[test]
    fn undo_removes_latest_stroke_and_redo_restores_it() {
        let mut document = document_with_strokes(3);
        let mut history = StrokeHistory::new();

        assert!(history.undo(&mut document));
        assert_eq!(
            document.strokes.iter().map(|s| s.id).collect::<Vec<_>>(),
            vec![1, 2]
        );

        assert!(history.redo(&mut document));
        assert_eq!(
            document.strokes.iter().map(|s| s.id).collect::<Vec<_>>(),
            vec![1, 2, 3]
        );
    }

    #[test]
    fn empty_undo_and_redo_are_noops() {
        let mut document = document_with_strokes(0);
        let mut history = StrokeHistory::new();

        assert!(!history.undo(&mut document));
        assert!(!history.redo(&mut document));
        assert!(document.strokes.is_empty());
    }

    #[test]
    fn drawing_after_undo_clears_redo_stack() {
        let mut document = document_with_strokes(2);
        let mut history = StrokeHistory::new();

        assert!(history.undo(&mut document));
        document.strokes.push(stroke(3));
        history.record_new_stroke();

        assert!(!history.redo(&mut document));
        assert_eq!(
            document.strokes.iter().map(|s| s.id).collect::<Vec<_>>(),
            vec![1, 3]
        );
    }

    #[test]
    fn clear_discards_redo_stack() {
        let mut document = document_with_strokes(1);
        let mut history = StrokeHistory::new();

        assert!(history.undo(&mut document));
        history.clear();

        assert!(!history.redo(&mut document));
    }
}
