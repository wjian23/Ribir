//! Incremental markdown parser
//!
//! Strategy:
//! 1. Each DocumentNode tracks its source_range in the original text
//! 2. Find nodes overlapping with edit range
//! 3. Determine reparse region
//! 4. Call parser to re-parse that region
//! 5. Merge: prefix (unchanged) + new (re-parsed) + suffix (range-adjusted)

use std::ops::Range;

use crate::{
  DocumentNode,
  parser::{parse_full, parse_range},
  undo_redo::EditOp,
};

/// Incremental parser that caches DocumentNodes and handles incremental updates
#[derive(Clone)]
pub struct IncrementalParser {
  cached_nodes: Vec<DocumentNode>,
  last_text: String,
}

impl IncrementalParser {
  /// Create new parser with initial text
  pub fn new(initial_text: &str) -> (Vec<DocumentNode>, Self) {
    let nodes = parse_full(initial_text);
    let parser = Self { cached_nodes: nodes.clone(), last_text: initial_text.to_string() };
    (nodes, parser)
  }

  /// Get cached nodes
  pub fn cached_nodes(&self) -> &Vec<DocumentNode> { &self.cached_nodes }

  /// Parse with pre-computed edit operation
  pub fn parse_with_op(&mut self, new_text: &str, edit_op: &EditOp) -> Vec<DocumentNode> {
    // Determine ranges from EditOp
    let old_range = edit_op.position..edit_op.position + edit_op.old_text.len();
    let new_range = edit_op.position..edit_op.position + edit_op.new_text.len();
    let length_delta = edit_op.new_text.len() as isize - edit_op.old_text.len() as isize;

    // Find affected node indices
    let affected = self.find_affected_nodes(&old_range);

    // Determine reparse region
    let reparse_region =
      self.compute_reparse_region(&affected, &old_range, &new_range, length_delta);

    // Re-parse affected region using parser
    let new_nodes = if reparse_region.start < reparse_region.end {
      parse_range(&new_text[reparse_region.start..reparse_region.end], reparse_region.start)
    } else {
      Vec::new()
    };

    // Merge results
    let result = self.merge_nodes(&affected, new_nodes, length_delta);

    // Update cache
    self.cached_nodes = result.clone();
    self.last_text = new_text.to_string();

    result
  }

  /// Parse incrementally (fallback that computes edit range internally)
  pub fn parse(&mut self, new_text: &str) -> Vec<DocumentNode> {
    if self.last_text.is_empty() {
      let nodes = parse_full(new_text);
      self.cached_nodes = nodes.clone();
      self.last_text = new_text.to_string();
      return self.cached_nodes.clone();
    }

    // Find edit region by comparing texts
    let Some((old_range, new_range)) = find_edit_range(&self.last_text, new_text) else {
      return self.cached_nodes.clone();
    };

    let length_delta = new_range.len() as isize - old_range.len() as isize;

    // Find affected nodes
    let affected = self.find_affected_nodes(&old_range);

    // Compute reparse region
    let reparse_region =
      self.compute_reparse_region(&affected, &old_range, &new_range, length_delta);

    // Re-parse
    let new_nodes = if reparse_region.start < reparse_region.end {
      parse_range(&new_text[reparse_region.start..reparse_region.end], reparse_region.start)
    } else {
      Vec::new()
    };

    // Merge
    let result = self.merge_nodes(&affected, new_nodes, length_delta);

    // Update cache
    self.cached_nodes = result.clone();
    self.last_text = new_text.to_string();

    result
  }

  /// Clear cache
  pub fn clear(&mut self) {
    self.cached_nodes.clear();
    self.last_text.clear();
  }

  /// Find nodes overlapping with edit range
  fn find_affected_nodes(&self, old_range: &Range<usize>) -> AffectedNodes {
    let replace_start = self
      .cached_nodes
      .iter()
      .position(|n| n.source_range.end > old_range.start)
      .unwrap_or(self.cached_nodes.len());

    let replace_end = self
      .cached_nodes
      .iter()
      .rposition(|n| n.source_range.start < old_range.end)
      .map(|i| i + 1)
      .unwrap_or(0);

    // Expand to adjacent nodes when edit is in gap
    let (expand_start, expand_end) = if replace_start < replace_end {
      (replace_start, replace_end)
    } else {
      (replace_start.saturating_sub(1), (replace_start + 1).min(self.cached_nodes.len()))
    };

    AffectedNodes { prefix_end: expand_start, suffix_start: expand_end }
  }

  /// Compute the text region that needs re-parsing
  fn compute_reparse_region(
    &self, affected: &AffectedNodes, _old_range: &Range<usize>, new_range: &Range<usize>,
    length_delta: isize,
  ) -> Range<usize> {
    let text_start = if affected.prefix_end < self.cached_nodes.len() {
      self.cached_nodes[affected.prefix_end]
        .source_range
        .start
    } else {
      new_range.start
    };

    let text_end = if affected.suffix_start > 0 {
      let end = self.cached_nodes[affected.suffix_start - 1]
        .source_range
        .end;
      (end as isize + length_delta).max(0) as usize
    } else {
      new_range.end
    };

    // Merge with new_range to ensure full coverage
    let reparse_start = text_start.min(new_range.start);
    let reparse_end = text_end.max(new_range.end);

    reparse_start..reparse_end
  }

  /// Merge: prefix + new_nodes + suffix (with adjusted ranges)
  fn merge_nodes(
    &self, affected: &AffectedNodes, new_nodes: Vec<DocumentNode>, length_delta: isize,
  ) -> Vec<DocumentNode> {
    let mut result = Vec::with_capacity(
      affected.prefix_end
        + new_nodes.len()
        + self
          .cached_nodes
          .len()
          .saturating_sub(affected.suffix_start),
    );

    // Prefix: unchanged nodes
    result.extend(
      self.cached_nodes[..affected.prefix_end]
        .iter()
        .cloned(),
    );

    // New: re-parsed nodes
    result.extend(new_nodes);

    // Suffix: nodes with adjusted ranges
    for node in &self.cached_nodes[affected.suffix_start..] {
      let mut adjusted = node.clone();
      adjusted.source_range = (adjusted.source_range.start as isize + length_delta).max(0) as usize
        ..(adjusted.source_range.end as isize + length_delta).max(0) as usize;
      result.push(adjusted);
    }

    result
  }
}

/// Tracks which cached nodes are affected by an edit
struct AffectedNodes {
  /// End index of prefix (unchanged nodes before edit)
  prefix_end: usize,
  /// Start index of suffix (nodes after edit that need range adjustment)
  suffix_start: usize,
}

/// Find the edit range between old and new text by comparing from both ends
fn find_edit_range(old: &str, new: &str) -> Option<(Range<usize>, Range<usize>)> {
  let old_bytes = old.as_bytes();
  let new_bytes = new.as_bytes();

  // Find common prefix length
  let prefix_len = old_bytes
    .iter()
    .zip(new_bytes)
    .take_while(|(a, b)| a == b)
    .count();

  // Find common suffix length (from end)
  let suffix_len = old_bytes
    .iter()
    .rev()
    .zip(new_bytes.iter().rev())
    .take_while(|(a, b)| a == b)
    .count();

  let old_end = old_bytes.len().saturating_sub(suffix_len);
  let new_end = new_bytes.len().saturating_sub(suffix_len);
  let old_start = prefix_len.min(old_bytes.len());
  let new_start = prefix_len.min(new_bytes.len());

  Some((old_start..old_end.max(old_start), new_start..new_end.max(new_start)))
}
