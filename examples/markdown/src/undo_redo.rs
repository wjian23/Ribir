//! Undo-Redo stack for markdown editor
//!
//! Captures text replacement operations and allows
//! replaying them in reverse (undo) or forward (redo).
//!
//! Simplified design: all edits are text replacements at a position.
//! - Insert: old_text is empty
//! - Delete: new_text is empty
//! - Replace: both have content

/// Represents a text replacement operation
#[derive(Clone, Debug)]
pub struct EditOp {
  /// Byte position where the replacement starts
  pub position: usize,
  /// The text that was replaced (needed for undo)
  pub old_text: String,
  /// The new text that replaced it (needed for redo)
  pub new_text: String,
}

impl EditOp {
  /// Apply inverse of this operation (undo)
  pub fn undo(&self, text: &mut String) {
    let end = self.position + self.new_text.len();
    let end = end.min(text.len());
    if self.position < text.len() {
      text.replace_range(self.position..end, &self.old_text);
    } else {
      text.push_str(&self.old_text);
    }
  }

  /// Re-apply this operation (redo)
  pub fn redo(&self, text: &mut String) {
    let end = self.position + self.old_text.len();
    let end = end.min(text.len());
    if self.position < text.len() || !self.old_text.is_empty() {
      text.replace_range(self.position..end, &self.new_text);
    } else {
      text.push_str(&self.new_text);
    }
  }

  /// Check if this is a pure insertion (old_text is empty)
  pub fn is_insert(&self) -> bool { self.old_text.is_empty() && !self.new_text.is_empty() }

  /// Check if this is a pure deletion (new_text is empty)
  pub fn is_delete(&self) -> bool { !self.old_text.is_empty() && self.new_text.is_empty() }
}

/// Undo-Redo stack that stores edit operations
pub struct UndoRedoStack {
  /// Stack of operations that can be undone
  undo_stack: Vec<EditOp>,
  /// Stack of operations that can be redone (cleared on new edits)
  redo_stack: Vec<EditOp>,
  /// Maximum stack size to prevent memory growth (0 = unlimited)
  max_size: usize,
}

impl UndoRedoStack {
  pub fn new() -> Self {
    Self {
      undo_stack: Vec::new(),
      redo_stack: Vec::new(),
      max_size: 1000, // Default limit
    }
  }

  /// Create a new stack with custom max size
  pub fn with_max_size(max_size: usize) -> Self {
    Self { undo_stack: Vec::new(), redo_stack: Vec::new(), max_size }
  }

  /// Push a new operation onto the undo stack
  /// This clears the redo stack (as new edits invalidate redo history)
  pub fn push(&mut self, operation: EditOp) {
    self.undo_stack.push(operation);
    self.redo_stack.clear();

    // Enforce max size limit
    if self.max_size > 0 && self.undo_stack.len() > self.max_size {
      let remove_count = self.undo_stack.len() - self.max_size;
      self.undo_stack.drain(0..remove_count);
    }
  }

  /// Undo the last operation, returning it
  pub fn undo(&mut self, text: &mut String) -> Option<EditOp> {
    if let Some(op) = self.undo_stack.pop() {
      op.undo(text);
      self.redo_stack.push(op.clone());
      Some(op)
    } else {
      None
    }
  }

  /// Redo the last undone operation
  pub fn redo(&mut self, text: &mut String) -> Option<EditOp> {
    if let Some(op) = self.redo_stack.pop() {
      op.redo(text);
      self.undo_stack.push(op.clone());
      Some(op)
    } else {
      None
    }
  }

  /// Check if there are operations to undo
  pub fn can_undo(&self) -> bool { !self.undo_stack.is_empty() }

  /// Check if there are operations to redo
  pub fn can_redo(&self) -> bool { !self.redo_stack.is_empty() }

  /// Clear all operations
  pub fn clear(&mut self) {
    self.undo_stack.clear();
    self.redo_stack.clear();
  }

  /// Get the number of operations in the undo stack
  pub fn undo_len(&self) -> usize { self.undo_stack.len() }

  /// Get the number of operations in the redo stack
  pub fn redo_len(&self) -> usize { self.redo_stack.len() }
}

impl Default for UndoRedoStack {
  fn default() -> Self { Self::new() }
}

/// Compute edit operation between two strings
/// Returns a single EditOp that transforms old → new
pub fn compute_edit_op(old: &str, new: &str) -> Option<EditOp> {
  if old == new {
    return None;
  }

  // Find the common prefix length
  let prefix_len = old
    .bytes()
    .zip(new.bytes())
    .take_while(|(a, b)| a == b)
    .count();

  // Find the common suffix length
  let suffix_len = old.as_bytes()[prefix_len..]
    .iter()
    .rev()
    .zip(new.as_bytes()[prefix_len..].iter().rev())
    .take_while(|(a, b)| a == b)
    .count();

  // Extract the replaced ranges
  let old_end = old.len() - suffix_len;
  let new_end = new.len() - suffix_len;

  let old_text = &old[prefix_len..old_end];
  let new_text = &new[prefix_len..new_end];

  Some(EditOp {
    position: prefix_len,
    old_text: old_text.to_string(),
    new_text: new_text.to_string(),
  })
}

#[cfg(test)]
mod tests {
  use super::*;

  #[test]
  fn test_insert() {
    let old = "Hello World";
    let new = "Hello New World";

    let op = compute_edit_op(old, new).unwrap();
    assert!(op.is_insert());
    assert_eq!(op.position, 6);
    assert_eq!(op.old_text, "");
    assert_eq!(op.new_text, "New ");

    // Verify undo
    let mut text = new.to_string();
    op.undo(&mut text);
    assert_eq!(text, old);

    // Verify redo
    let mut text2 = old.to_string();
    op.redo(&mut text2);
    assert_eq!(text2, new);
  }

  #[test]
  fn test_delete() {
    let old = "Hello Beautiful World";
    let new = "Hello World";

    let op = compute_edit_op(old, new).unwrap();
    assert!(op.is_delete());
    assert_eq!(op.position, 6);
    assert_eq!(op.old_text, "Beautiful ");
    assert_eq!(op.new_text, "");

    // Verify undo
    let mut text = new.to_string();
    op.undo(&mut text);
    assert_eq!(text, old);

    // Verify redo
    let mut text2 = old.to_string();
    op.redo(&mut text2);
    assert_eq!(text2, new);
  }

  #[test]
  fn test_replace() {
    let old = "Hello World";
    let new = "Hello Rust World";

    let op = compute_edit_op(old, new).unwrap();
    // This is actually an insert since "Hello World" → "Hello Rust World"
    // inserts "Rust " before "World", old_text is empty
    assert!(op.is_insert());
    assert_eq!(op.position, 6);
    assert_eq!(op.old_text, "");

    // "Hello World" vs "Hello Rust World"
    // prefix = "Hello " (6 bytes)
    // old[6..] = "World"
    // new[6..] = "Rust World"
    // suffix = "World" (5 bytes)
    // old_text = old[6..6] = "" (11-5=6)
    // new_text = new[6..10] = "Rust" (15-5=10)

    assert_eq!(op.new_text, "Rust ");

    // Verify undo
    let mut text = new.to_string();
    op.undo(&mut text);
    assert_eq!(text, old);

    // Verify redo
    let mut text2 = old.to_string();
    op.redo(&mut text2);
    assert_eq!(text2, new);
  }

  #[test]
  fn test_undo_redo_stack() {
    let mut stack = UndoRedoStack::new();
    let mut text = "Hello".to_string();

    // Simulate: "Hello" -> "Hello World" (insert " World" at 5)
    stack.push(EditOp { position: 5, old_text: String::new(), new_text: " World".to_string() });

    assert_eq!(text, "Hello");
    assert!(stack.can_undo());
    assert!(!stack.can_redo());

    // Undo: "Hello World" -> "Hello"
    stack.undo(&mut text);
    assert_eq!(text, "Hello");
    assert!(!stack.can_undo());
    assert!(stack.can_redo());

    // Redo: "Hello" -> "Hello World"
    stack.redo(&mut text);
    assert_eq!(text, "Hello World");
    assert!(stack.can_undo());
    assert!(!stack.can_redo());
  }

  #[test]
  fn test_new_edit_clears_redo() {
    let mut stack = UndoRedoStack::new();

    stack.push(EditOp { position: 0, old_text: String::new(), new_text: "A".to_string() });
    stack.push(EditOp { position: 1, old_text: String::new(), new_text: "B".to_string() });

    assert_eq!(stack.undo_len(), 2);
    assert_eq!(stack.redo_len(), 0);

    let mut text = String::new();
    stack.undo(&mut text);
    assert_eq!(stack.redo_len(), 1);

    // New edit clears redo
    stack.push(EditOp { position: 0, old_text: String::new(), new_text: "C".to_string() });
    assert_eq!(stack.redo_len(), 0);
  }

  #[test]
  fn test_multiple_operations() {
    let mut stack = UndoRedoStack::new();

    // Start with empty string
    let mut text = String::new();

    // Type "Hello"
    stack.push(EditOp { position: 0, old_text: String::new(), new_text: "Hello".to_string() });
    text = "Hello".to_string();

    // Add " World"
    stack.push(EditOp { position: 5, old_text: String::new(), new_text: " World".to_string() });
    text = "Hello World".to_string();

    // Undo twice
    stack.undo(&mut text);
    assert_eq!(text, "Hello");
    stack.undo(&mut text);
    assert_eq!(text, "");

    // Redo twice
    stack.redo(&mut text);
    assert_eq!(text, "Hello");
    stack.redo(&mut text);
    assert_eq!(text, "Hello World");
  }
}
