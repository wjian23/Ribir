//! Core markdown document model with command queue
//!
//! Simplified architecture:
//! - All edits unified as SetText commands
//! - Consecutive SetText coalesced (keep only last)
//! - Debounce 200ms to consume and generate EditOp
//! - Undo/Redo forces queue consumption first
//! - Version tracking for external dirty state comparison
//! - Save accepts path externally (doc doesn't store path)

use std::{cell::RefCell, collections::VecDeque, rc::Rc};

use crate::{
  DocumentNode, MarkdownDocument,
  incremental_parser::IncrementalParser,
  undo_redo::{EditOp, UndoRedoStack, compute_edit_op},
};

/// Pending command in the queue
#[derive(Clone, Debug)]
pub enum PendingCommand {
  /// Set text (unified for all edits)
  /// Multiple consecutive SetText are coalesced - only last is kept
  SetText { text: String },
  /// Undo operation
  Undo,
  /// Redo operation
  Redo,
}

/// Command queue with SetText coalescing
#[derive(Clone, Debug)]
pub struct CommandQueue {
  commands: VecDeque<PendingCommand>,
}

impl CommandQueue {
  pub fn new() -> Self { Self { commands: VecDeque::new() } }

  /// Push a command to the queue
  /// For SetText: if last is SetText, replace it; otherwise push
  pub fn push(&mut self, cmd: PendingCommand) {
    match &cmd {
      PendingCommand::SetText { .. } => {
        // If last is SetText, replace it
        if let Some(last) = self.commands.back_mut()
          && matches!(last, PendingCommand::SetText { .. })
        {
          *last = cmd;
          return;
        }
        self.commands.push_back(cmd);
      }
      PendingCommand::Undo | PendingCommand::Redo => {
        self.commands.push_back(cmd);
      }
    }
  }

  /// Pop the next command from queue
  pub fn pop(&mut self) -> Option<PendingCommand> { self.commands.pop_front() }

  /// Check if queue is empty
  pub fn is_empty(&self) -> bool { self.commands.is_empty() }

  /// Get queue length
  pub fn len(&self) -> usize { self.commands.len() }

  /// Clear all commands
  pub fn clear(&mut self) { self.commands.clear(); }

  /// Check if last command is SetText
  pub fn last_is_settext(&self) -> bool {
    self
      .commands
      .back()
      .map(|c| matches!(c, PendingCommand::SetText { .. }))
      .unwrap_or(false)
  }
}

impl Default for CommandQueue {
  fn default() -> Self { Self::new() }
}

/// Core markdown document with command queue
pub struct MarkdownCoreDoc {
  /// Raw markdown source (committed state)
  source: String,
  /// Parsed document structure
  document: MarkdownDocument,
  /// Undo/redo history
  undo_redo: UndoRedoStack,
  /// Incremental parser for efficient re-parsing
  incremental_parser: IncrementalParser,
  /// Version number that increments with each committed edit
  version: u64,
  /// Pending commands queue
  command_queue: CommandQueue,
  /// Pending text (current text being edited, not yet committed)
  pending_text: Rc<RefCell<String>>,
  /// Editing version - increments on every push_set_text, used for dirty
  /// detection
  editing_version: u64,
}

impl MarkdownCoreDoc {
  /// Create a new document from markdown text
  pub fn new(markdown: impl Into<String>) -> Self {
    let source = markdown.into();
    let (nodes, incremental_parser) = IncrementalParser::new(&source);
    let document = MarkdownDocument { nodes };
    Self {
      source: source.clone(),
      document,
      undo_redo: UndoRedoStack::new(),
      incremental_parser,
      version: 0,
      command_queue: CommandQueue::new(),
      pending_text: Rc::new(RefCell::new(source)),
      editing_version: 0,
    }
  }

  /// Get the committed source text
  pub fn source(&self) -> &str { &self.source }

  /// Get pending text (current editing state)
  pub fn pending_text(&self) -> String { self.pending_text.borrow().clone() }

  /// Set pending text (called by edit stream)
  pub fn set_pending_text(&self, text: String) { *self.pending_text.borrow_mut() = text; }

  /// Get parsed document
  pub fn document(&self) -> &MarkdownDocument { &self.document }

  /// Get current version (committed)
  pub fn version(&self) -> u64 { self.version }

  /// Get editing version (increments on every push_set_text)
  /// Used for immediate dirty detection before debounce consume
  pub fn editing_version(&self) -> u64 { self.editing_version }

  /// Check if there are pending SetText commands
  pub fn has_pending(&self) -> bool { self.command_queue.last_is_settext() }

  /// Push SetText command (coalesced)
  pub fn push_set_text(&mut self, text: String) {
    self
      .command_queue
      .push(PendingCommand::SetText { text: text.clone() });
    *self.pending_text.borrow_mut() = text;
    self.editing_version += 1;
  }

  /// Push Undo command
  pub fn push_undo(&mut self) { self.command_queue.push(PendingCommand::Undo); }

  /// Push Redo command
  pub fn push_redo(&mut self) { self.command_queue.push(PendingCommand::Redo); }

  /// Consume pending SetText and commit to document
  /// Returns the EditOp that was generated and pushed to undo stack
  pub fn consume_set_text(&mut self) -> Option<EditOp> {
    let Some(PendingCommand::SetText { .. }) = self.command_queue.commands.front() else {
      return None;
    };
    let text = match self.command_queue.pop() {
      Some(PendingCommand::SetText { text }) => text,
      Some(PendingCommand::Undo | PendingCommand::Redo) | None => unreachable!(),
    };

    // Generate EditOp from old to new text
    let old_text = self.source.clone();
    if old_text == text {
      return None;
    }

    let op = compute_edit_op(&old_text, &text)?;

    // Push to undo stack
    self.undo_redo.push(op.clone());

    // Update source and version
    self.source = text.clone();
    self.version += 1;

    // Incremental parse
    let source = self.source.clone();
    self.incremental_parse_with_op(&source, &op);

    Some(op)
  }

  /// Force consume all pending commands before Undo/Redo
  fn force_consume_pending(&mut self) -> Option<EditOp> { self.consume_set_text() }

  /// Undo: force consume pending, then undo
  /// Returns EditOp for caret positioning
  pub fn undo(&mut self) -> Option<EditOp> {
    // Force consume any pending SetText first
    self.force_consume_pending();

    let mut text = self.source.clone();
    let op = self.undo_redo.undo(&mut text)?;
    let inverse_op = inverse_edit_op(&op);

    self.source = text.clone();
    self.version = self.version.saturating_sub(1);

    self.incremental_parse_with_op(&text, &inverse_op);

    // Update pending text to match committed
    *self.pending_text.borrow_mut() = text;

    Some(op)
  }

  /// Redo: force consume pending, then redo
  /// Returns EditOp for caret positioning
  pub fn redo(&mut self) -> Option<EditOp> {
    // Force consume any pending SetText first
    self.force_consume_pending();

    let mut text = self.source.clone();
    let op = self.undo_redo.redo(&mut text)?;

    self.source = text.clone();
    self.version += 1;

    self.incremental_parse_with_op(&text, &op);

    // Update pending text to match committed
    *self.pending_text.borrow_mut() = text;

    Some(op)
  }

  /// Save to path: consume pending, write file
  /// Returns the version after save (for external tracking)
  pub fn save(&mut self, path: &str) -> std::io::Result<u64> {
    // Consume any pending edits
    self.force_consume_pending();

    std::fs::write(path, &self.source)?;
    Ok(self.version)
  }

  /// Parse incrementally with EditOp
  fn incremental_parse_with_op(&mut self, text: &str, edit_op: &EditOp) {
    let nodes = self
      .incremental_parser
      .parse_with_op(text, edit_op);
    self.document = MarkdownDocument { nodes };
  }

  /// Get document nodes for rendering
  pub fn document_nodes(&self) -> &Vec<DocumentNode> { &self.document.nodes }

  /// Check if undo is available
  pub fn can_undo(&self) -> bool { self.undo_redo.can_undo() }

  /// Check if redo is available
  pub fn can_redo(&self) -> bool { self.undo_redo.can_redo() }
}

impl Default for MarkdownCoreDoc {
  fn default() -> Self { Self::new("") }
}

fn inverse_edit_op(edit_op: &EditOp) -> EditOp {
  EditOp {
    position: edit_op.position,
    old_text: edit_op.new_text.clone(),
    new_text: edit_op.old_text.clone(),
  }
}

#[cfg(test)]
mod tests {
  use super::*;
  use crate::{MarkdownNode, parser::parse_full};

  fn summarize(nodes: &[DocumentNode]) -> Vec<(std::ops::Range<usize>, MarkdownNode)> {
    nodes
      .iter()
      .map(|node| (node.source_range.clone(), node.node.clone()))
      .collect()
  }

  #[test]
  fn test_command_queue_coalescing() {
    let mut queue = CommandQueue::new();

    queue.push(PendingCommand::SetText { text: "first".to_string() });
    assert_eq!(queue.len(), 1);

    queue.push(PendingCommand::SetText { text: "second".to_string() });
    assert_eq!(queue.len(), 1); // Replaced

    queue.push(PendingCommand::SetText { text: "third".to_string() });
    assert_eq!(queue.len(), 1); // Replaced

    let cmd = queue.pop();
    assert!(matches!(cmd, Some(PendingCommand::SetText { text }) if text == "third"));
    assert!(queue.is_empty());
  }

  #[test]
  fn test_command_queue_undo_not_coalesced() {
    let mut queue = CommandQueue::new();

    queue.push(PendingCommand::SetText { text: "a".to_string() });
    queue.push(PendingCommand::Undo);
    queue.push(PendingCommand::SetText { text: "b".to_string() }); // New SetText after Undo

    assert_eq!(queue.len(), 3);

    let cmd1 = queue.pop();
    assert!(matches!(cmd1, Some(PendingCommand::SetText { .. })));

    let cmd2 = queue.pop();
    assert!(matches!(cmd2, Some(PendingCommand::Undo)));

    let cmd3 = queue.pop();
    assert!(matches!(cmd3, Some(PendingCommand::SetText { text }) if text == "b"));
  }

  #[test]
  fn test_doc_consume_set_text() {
    let mut doc = MarkdownCoreDoc::new("initial");

    doc.push_set_text("modified".to_string());
    assert!(doc.has_pending());

    let op = doc.consume_set_text();
    assert!(op.is_some());
    assert_eq!(doc.source(), "modified");
    assert_eq!(doc.version(), 1);
    assert!(!doc.has_pending());
  }

  #[test]
  fn test_doc_consume_set_text_preserves_queued_undo_redo() {
    let mut doc = MarkdownCoreDoc::new("initial");
    doc.push_undo();
    doc.push_set_text("modified".to_string());

    assert!(doc.consume_set_text().is_none());
    assert_eq!(doc.command_queue.len(), 2);
    assert!(matches!(doc.command_queue.pop(), Some(PendingCommand::Undo)));
    assert!(matches!(
      doc.command_queue.pop(),
      Some(PendingCommand::SetText { text }) if text == "modified"
    ));
  }

  #[test]
  fn test_doc_undo_consumes_pending() {
    let mut doc = MarkdownCoreDoc::new("initial");

    // First edit
    doc.push_set_text("first".to_string());
    doc.consume_set_text();
    assert_eq!(doc.source(), "first");

    // Second edit (pending)
    doc.push_set_text("second".to_string());
    assert!(doc.has_pending());

    // Undo: consumes pending ("second"), then undo ("first" → "second")
    // Result: back to "first"
    let op = doc.undo();
    assert!(op.is_some());
    assert_eq!(doc.source(), "first");
    assert!(!doc.has_pending());

    // Another undo: back to "initial"
    let op2 = doc.undo();
    assert!(op2.is_some());
    assert_eq!(doc.source(), "initial");
  }

  #[test]
  fn test_version_tracking() {
    let mut doc = MarkdownCoreDoc::new("initial");
    assert_eq!(doc.version(), 0);

    doc.push_set_text("modified".to_string());
    // Version unchanged until consumed
    assert_eq!(doc.version(), 0);

    doc.consume_set_text();
    assert_eq!(doc.version(), 1);

    // Another edit
    doc.push_set_text("edited again".to_string());
    doc.consume_set_text();
    assert_eq!(doc.version(), 2);

    // Undo decrements version
    doc.undo();
    assert_eq!(doc.version(), 1);
  }

  #[test]
  fn test_undo_redo_reparse_matches_full_parse() {
    let old = "aaa\n\n# Heading\n";
    let new = "aa\n\n# Heading\n";
    let mut doc = MarkdownCoreDoc::new(old);

    doc.push_set_text(new.to_string());
    doc.consume_set_text();
    assert_eq!(summarize(doc.document_nodes()), summarize(&parse_full(new)));

    doc.undo();
    assert_eq!(doc.source(), old);
    assert_eq!(summarize(doc.document_nodes()), summarize(&parse_full(old)));

    doc.redo();
    assert_eq!(doc.source(), new);
    assert_eq!(summarize(doc.document_nodes()), summarize(&parse_full(new)));
  }

  #[test]
  fn test_save_to_path() {
    let mut doc = MarkdownCoreDoc::new("test content");
    doc.push_set_text("modified".to_string());

    // Save with pending should consume first
    let version = doc.save("/tmp/test_doc.md").unwrap();
    assert_eq!(doc.source(), "modified"); // Pending consumed
    assert_eq!(version, 1); // Returns version for external tracking
  }
}
