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
    let old_range = edit_op.position..edit_op.position + edit_op.old_text.len();
    let new_range = edit_op.position..edit_op.position + edit_op.new_text.len();
    self.reparse(new_text, &old_range, &new_range)
  }

  /// Parse incrementally (fallback that computes edit range internally)
  pub fn parse(&mut self, new_text: &str) -> Vec<DocumentNode> {
    if self.last_text.is_empty() {
      let nodes = parse_full(new_text);
      self.cached_nodes = nodes.clone();
      self.last_text = new_text.to_string();
      return self.cached_nodes.clone();
    }

    let Some((old_range, new_range)) = find_edit_range(&self.last_text, new_text) else {
      return self.cached_nodes.clone();
    };

    self.reparse(new_text, &old_range, &new_range)
  }

  fn reparse(
    &mut self, new_text: &str, old_range: &Range<usize>, new_range: &Range<usize>,
  ) -> Vec<DocumentNode> {
    let length_delta = new_range.len() as isize - old_range.len() as isize;
    let affected = self.find_affected_nodes(old_range);
    let reparse_region =
      self.compute_reparse_region(new_text, &affected, old_range, new_range, length_delta);

    let new_nodes = parse_reparse_region(new_text, &reparse_region);
    let result = self.merge_nodes(&affected, new_nodes, length_delta);

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
    if self.cached_nodes.is_empty() {
      return AffectedNodes { prefix_end: 0, suffix_start: 0 };
    }

    let replace_start = self
      .cached_nodes
      .iter()
      .position(|n| n.source_range.end >= old_range.start)
      .unwrap_or(self.cached_nodes.len());

    let replace_end = self
      .cached_nodes
      .iter()
      .rposition(|n| n.source_range.start <= old_range.end)
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
    &self, new_text: &str, affected: &AffectedNodes, _old_range: &Range<usize>,
    new_range: &Range<usize>, length_delta: isize,
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
    let reparse_start = text_start
      .min(new_range.start)
      .min(new_text.len());
    let reparse_end = text_end.max(new_range.end).min(new_text.len());

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
  if old == new {
    return None;
  }

  let old_bytes = old.as_bytes();
  let new_bytes = new.as_bytes();

  // Find common prefix length
  let prefix_len = old_bytes
    .iter()
    .zip(new_bytes)
    .take_while(|(a, b)| a == b)
    .count();

  // Find common suffix length (from end)
  let suffix_len = old_bytes[prefix_len..]
    .iter()
    .rev()
    .zip(new_bytes[prefix_len..].iter().rev())
    .take_while(|(a, b)| a == b)
    .count();

  let old_end = old_bytes.len() - suffix_len;
  let new_end = new_bytes.len() - suffix_len;

  Some((prefix_len..old_end, prefix_len..new_end))
}

fn parse_reparse_region(text: &str, reparse_region: &Range<usize>) -> Vec<DocumentNode> {
  if reparse_region.start < reparse_region.end {
    parse_range(&text[reparse_region.start..reparse_region.end], reparse_region.start)
  } else {
    Vec::new()
  }
}

#[cfg(test)]
mod tests {
  use super::*;
  use crate::{MarkdownNode, undo_redo::compute_edit_op};

  fn summarize(nodes: &[DocumentNode]) -> Vec<(Range<usize>, MarkdownNode)> {
    nodes
      .iter()
      .map(|node| (node.source_range.clone(), node.node.clone()))
      .collect()
  }

  fn assert_incremental_matches_full(old: &str, new: &str) {
    let full = summarize(&parse_full(new));

    let mut parser = IncrementalParser::new(old).1;
    assert_eq!(summarize(&parser.parse(new)), full);

    let mut parser = IncrementalParser::new(old).1;
    let op = compute_edit_op(old, new).expect("expected an edit operation");
    assert_eq!(summarize(&parser.parse_with_op(new, &op)), full);
  }

  #[test]
  fn find_edit_range_handles_repeated_bytes_without_overlap() {
    assert_eq!(find_edit_range("aaa", "aa"), Some((2..3, 2..2)));
    assert_eq!(find_edit_range("aab", "ab"), Some((1..2, 1..1)));
    assert_eq!(find_edit_range("abc", "abbc"), Some((2..2, 2..3)));
  }

  #[test]
  fn parse_matches_full_for_repeated_character_edits() {
    assert_incremental_matches_full("abc", "abbc");
    assert_incremental_matches_full("abbc", "abc");
  }

  #[test]
  fn parse_matches_full_for_block_boundary_merges_and_splits() {
    assert_incremental_matches_full("- a\n\npara\n", "- a\npara\n");
    assert_incremental_matches_full("- a\npara\n", "- a\n\npara\n");
    assert_incremental_matches_full("a\n\nb\n", "a\nb\n");
    assert_incremental_matches_full("a\nb\n", "a\n\nb\n");
  }
}
