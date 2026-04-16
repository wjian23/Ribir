//! Markdown parser that converts markdown text to document tree
//!
//! Provides full parsing and range-based parsing for incremental updates.

use std::mem;

use pulldown_cmark::{Alignment, Event, Options, Parser, Tag, TagEnd};

use crate::{
  DocumentNode, NodeId, allocate_node_id,
  components::{
    InlineNode, InlineStyle, MarkdownNode, MarkdownTableData, TableAlign, TableCell, TableRow,
    TodoItem, TodoState,
  },
};

/// Parse full markdown text into DocumentNodes with source ranges
pub fn parse_full(text: &str) -> Vec<DocumentNode> { parse_range(text, 0) }

/// Parse markdown text range into DocumentNodes with offset-adjusted source
/// ranges
pub fn parse_range(text: &str, base_offset: usize) -> Vec<DocumentNode> {
  let mut nodes = Vec::new();
  let mut current_inlines: Vec<InlineNode> = Vec::new();
  let mut current_style = InlineStyle::default();
  let mut block_start: Option<usize> = None;
  let mut list_state = ListState::default();
  let mut heading_level: Option<u8> = None;
  let mut table_state = TableState::default();

  let mut pending_image_url: Option<String> = None;
  let mut pending_image_alt: String = String::new();

  let mut in_code_block = false;
  let mut code_block_language = String::new();
  let mut code_block_content = String::new();
  let mut code_block_start: Option<usize> = None;

  let mut options = Options::empty();
  options.insert(Options::ENABLE_TABLES);
  let parser = Parser::new_ext(text, options).into_offset_iter();

  for (event, range) in parser {
    let event_start = base_offset + range.start;
    let event_end = base_offset + range.end;

    match event {
      Event::Start(tag) => match tag {
        Tag::Paragraph => {
          if table_state.is_active() {
            continue;
          }
          list_state.flush_into(&mut nodes, event_start);
          block_start = Some(event_start);
          current_inlines.clear();
          current_style = InlineStyle::default();
        }
        Tag::Heading { level, .. } => {
          list_state.flush_into(&mut nodes, event_start);
          block_start = Some(event_start);
          current_inlines.clear();
          current_style = InlineStyle::default();
          heading_level = Some(match level {
            pulldown_cmark::HeadingLevel::H1 => 1,
            pulldown_cmark::HeadingLevel::H2 => 2,
            pulldown_cmark::HeadingLevel::H3 => 3,
            pulldown_cmark::HeadingLevel::H4 => 4,
            pulldown_cmark::HeadingLevel::H5 => 5,
            pulldown_cmark::HeadingLevel::H6 => 6,
          });
        }
        Tag::List(list_type) => {
          list_state.flush_into(&mut nodes, event_start);
          list_state.begin(list_type.is_some(), event_start);
        }
        Tag::Item => {
          current_inlines.clear();
          current_style = InlineStyle::default();
        }
        Tag::Table(alignments) => {
          list_state.flush_into(&mut nodes, event_start);
          table_state.begin(
            alignments
              .into_iter()
              .map(map_table_alignment)
              .collect(),
            event_start,
          );
        }
        Tag::TableHead => {
          table_state.begin_header();
        }
        Tag::TableRow => {
          table_state.begin_row();
          current_inlines.clear();
          current_style = InlineStyle::default();
        }
        Tag::TableCell => {
          current_inlines.clear();
          current_style = InlineStyle::default();
        }
        Tag::Link { dest_url, .. } => {
          if block_start.is_none() {
            block_start = Some(event_start);
          }
          current_style.link_url = Some(dest_url.to_string());
        }
        Tag::Image { dest_url, .. } => {
          if block_start.is_none() {
            block_start = Some(event_start);
          }
          pending_image_url = Some(dest_url.to_string());
          pending_image_alt.clear();
        }
        Tag::CodeBlock(lang) => {
          block_start = Some(event_start);
          code_block_start = Some(event_start);
          in_code_block = true;
          code_block_language = match &lang {
            pulldown_cmark::CodeBlockKind::Fenced(l) => l.to_string(),
            _ => String::new(),
          };
          code_block_content.clear();
        }
        Tag::Strong => {
          current_style.bold = true;
        }
        Tag::Emphasis => {
          current_style.italic = true;
        }
        _ => {}
      },
      Event::Rule => {
        nodes.push(make_node(
          allocate_node_id,
          MarkdownNode::HorizontalRule,
          event_start,
          event_end,
        ));
      }
      Event::End(tag) => match tag {
        TagEnd::Paragraph => {
          if table_state.is_active() {
            continue;
          }
          let start = block_start.unwrap_or(event_start);
          if !current_inlines.is_empty() && !list_state.is_active() {
            flush_paragraph_by_images(&mut current_inlines, &mut nodes, start, event_end);
          }
          block_start = None;
          current_inlines.clear();
          current_style = InlineStyle::default();
        }
        TagEnd::Heading(_) => {
          let start = block_start.unwrap_or(event_start);
          if !current_inlines.is_empty() {
            let level = heading_level.unwrap_or(1);
            nodes.push(make_node(
              allocate_node_id,
              MarkdownNode::Heading { level, inlines: current_inlines.clone() },
              start,
              event_end,
            ));
          }
          block_start = None;
          current_inlines.clear();
          current_style = InlineStyle::default();
          heading_level = None;
        }
        TagEnd::List(_) => {
          list_state.flush_into(&mut nodes, event_end);
        }
        TagEnd::Item => {
          if list_state.is_active() {
            list_state.push_item(&current_inlines);
            current_inlines.clear();
            current_style = InlineStyle::default();
          }
        }
        TagEnd::Table => {
          if let Some((start, table)) = table_state.finish() {
            nodes.push(make_node(
              allocate_node_id,
              MarkdownNode::Table { table },
              start,
              event_end,
            ));
          }
        }
        TagEnd::TableHead => {
          table_state.end_header();
        }
        TagEnd::TableRow => {
          table_state.finish_row();
        }
        TagEnd::TableCell => {
          table_state.push_cell(mem::take(&mut current_inlines));
          current_style = InlineStyle::default();
        }
        TagEnd::Link => {
          current_style.link_url = None;
        }
        TagEnd::Image => {
          if let Some(url) = pending_image_url.take() {
            current_inlines.push(InlineNode::Image { url, alt: pending_image_alt.clone() });
            pending_image_alt.clear();
          }
        }
        TagEnd::CodeBlock => {
          let start = code_block_start.unwrap_or(event_start);
          if in_code_block {
            nodes.push(make_node(
              allocate_node_id,
              MarkdownNode::CodeBlock {
                language: code_block_language.clone(),
                code: code_block_content.clone(),
              },
              start,
              event_end,
            ));
            in_code_block = false;
          }
          block_start = None;
          code_block_start = None;
        }
        TagEnd::Strong => {
          current_style.bold = false;
        }
        TagEnd::Emphasis => {
          current_style.italic = false;
        }
        _ => {}
      },
      Event::Text(t) => {
        if in_code_block {
          code_block_content.push_str(&t);
        } else if pending_image_url.is_some() {
          pending_image_alt.push_str(&t);
        } else {
          flush_text(&t, &current_style, &mut current_inlines);
        }
      }
      Event::Code(c) => {
        let mut code_style = current_style.clone();
        code_style.code = true;
        flush_text(&c, &code_style, &mut current_inlines);
      }
      Event::SoftBreak | Event::HardBreak => {
        flush_text(" ", &current_style, &mut current_inlines);
      }
      _ => {}
    }
  }

  nodes
}

fn make_node(
  id_fn: impl FnOnce() -> NodeId, node: MarkdownNode, start: usize, end: usize,
) -> DocumentNode {
  DocumentNode { id: id_fn(), node, source_range: start..end }
}

fn flush_paragraph_by_images(
  inlines: &mut Vec<InlineNode>, nodes: &mut Vec<DocumentNode>, start: usize, end: usize,
) {
  let mut text_inlines: Vec<InlineNode> = Vec::new();

  for inline in inlines.drain(..) {
    match inline {
      InlineNode::Text { .. } => {
        text_inlines.push(inline);
      }
      InlineNode::Image { url, alt } => {
        if !text_inlines.is_empty() {
          nodes.push(make_node(
            allocate_node_id,
            MarkdownNode::Paragraph { inlines: std::mem::take(&mut text_inlines) },
            start,
            end,
          ));
        }
        nodes.push(make_node(allocate_node_id, MarkdownNode::Image { url, alt }, start, end));
      }
      InlineNode::TodoMarker { .. } => {
        text_inlines.push(inline);
      }
    }
  }

  if !text_inlines.is_empty() {
    nodes.push(make_node(
      allocate_node_id,
      MarkdownNode::Paragraph { inlines: text_inlines },
      start,
      end,
    ));
  }
}

fn flush_text(text: &str, style: &InlineStyle, inlines: &mut Vec<InlineNode>) {
  if !text.is_empty() {
    inlines.push(InlineNode::Text { content: text.to_string(), style: style.clone() });
  }
}

fn flatten_inline_text(inlines: &[InlineNode]) -> String {
  inlines
    .iter()
    .map(|inline| match inline {
      InlineNode::Text { content, .. } => content.clone(),
      InlineNode::Image { alt, .. } => format!("[{}]", alt),
      InlineNode::TodoMarker { .. } => String::new(),
    })
    .collect()
}

fn parse_checkbox(text: &str) -> Option<(TodoState, String)> {
  let trimmed = text.trim_start();
  if let Some(rest) = trimmed.strip_prefix("[ ] ") {
    Some((TodoState::Unchecked, rest.to_string()))
  } else if trimmed == "[ ]" {
    Some((TodoState::Unchecked, String::new()))
  } else if let Some(rest) = trimmed
    .strip_prefix("[x] ")
    .or_else(|| trimmed.strip_prefix("[X] "))
  {
    Some((TodoState::Checked, rest.to_string()))
  } else if trimmed == "[x]" || trimmed == "[X]" {
    Some((TodoState::Checked, String::new()))
  } else if let Some(rest) = trimmed.strip_prefix("[-] ") {
    Some((TodoState::Indeterminate, rest.to_string()))
  } else if trimmed == "[-]" {
    Some((TodoState::Indeterminate, String::new()))
  } else {
    None
  }
}

fn strip_inline_prefix(inlines: &[InlineNode], prefix_len: usize) -> Vec<InlineNode> {
  let mut remaining = prefix_len;
  let mut stripped = Vec::with_capacity(inlines.len());

  for inline in inlines {
    match inline {
      InlineNode::Text { content, style } if remaining > 0 => {
        if remaining >= content.len() {
          remaining -= content.len();
          continue;
        }

        stripped.push(InlineNode::Text {
          content: content[remaining..].to_string(),
          style: style.clone(),
        });
        remaining = 0;
      }
      _ => stripped.push(inline.clone()),
    }
  }

  stripped
}

fn map_table_alignment(alignment: Alignment) -> TableAlign {
  match alignment {
    Alignment::None | Alignment::Left => TableAlign::Start,
    Alignment::Center => TableAlign::Center,
    Alignment::Right => TableAlign::End,
  }
}

#[derive(Default)]
struct ListState {
  kind: Option<ListKind>,
  start: Option<usize>,
  items: Vec<Vec<InlineNode>>,
  todo_items: Vec<TodoItem>,
}

#[derive(Clone, Copy, Default)]
enum ListKind {
  #[default]
  Unordered,
  Ordered,
  Todo,
}

impl ListState {
  fn is_active(&self) -> bool { self.kind.is_some() }

  fn begin(&mut self, ordered: bool, start: usize) {
    self.kind = Some(if ordered { ListKind::Ordered } else { ListKind::Unordered });
    self.start = Some(start);
    self.items.clear();
    self.todo_items.clear();
  }

  fn push_item(&mut self, inlines: &[InlineNode]) {
    let text = flatten_inline_text(inlines);
    if let Some((state, todo_text)) = parse_checkbox(&text) {
      let prefix_len = text.len().saturating_sub(todo_text.len());
      self.kind = Some(ListKind::Todo);
      self
        .todo_items
        .push(TodoItem { state, inlines: strip_inline_prefix(inlines, prefix_len) });
    } else {
      self.items.push(inlines.to_vec());
    }
  }

  fn flush_into(&mut self, nodes: &mut Vec<DocumentNode>, end: usize) {
    let Some(kind) = self.kind.take() else {
      return;
    };
    let start = self.start.take().unwrap_or(end);
    let node = match kind {
      ListKind::Todo => MarkdownNode::TodoList { items: mem::take(&mut self.todo_items) },
      ListKind::Ordered => MarkdownNode::OrderedList { items: mem::take(&mut self.items) },
      ListKind::Unordered => MarkdownNode::UnorderedList { items: mem::take(&mut self.items) },
    };
    nodes.push(make_node(allocate_node_id, node, start, end));
  }
}

#[derive(Default)]
struct TableState {
  active: bool,
  start: Option<usize>,
  alignments: Vec<TableAlign>,
  header: Option<TableRow>,
  rows: Vec<TableRow>,
  current_row: Vec<TableCell>,
  in_header: bool,
}

impl TableState {
  fn is_active(&self) -> bool { self.active }

  fn begin(&mut self, alignments: Vec<TableAlign>, start: usize) {
    self.active = true;
    self.start = Some(start);
    self.alignments = alignments;
    self.header = None;
    self.rows.clear();
    self.current_row.clear();
    self.in_header = false;
  }

  fn begin_header(&mut self) { self.in_header = true; }

  fn end_header(&mut self) {
    if self.header.is_none() && !self.current_row.is_empty() {
      self.header = Some(TableRow { cells: mem::take(&mut self.current_row) });
    }
    self.in_header = false;
  }

  fn begin_row(&mut self) { self.current_row.clear(); }

  fn push_cell(&mut self, inlines: Vec<InlineNode>) {
    self.current_row.push(TableCell { inlines });
  }

  fn finish_row(&mut self) {
    if self.current_row.is_empty() {
      return;
    }
    let row = TableRow { cells: mem::take(&mut self.current_row) };
    if self.in_header && self.header.is_none() {
      self.header = Some(row);
    } else {
      self.rows.push(row);
    }
  }

  fn finish(&mut self) -> Option<(usize, MarkdownTableData)> {
    if !self.active {
      return None;
    }

    self.active = false;
    self.in_header = false;
    Some((
      self.start.take().unwrap_or_default(),
      MarkdownTableData {
        alignments: mem::take(&mut self.alignments),
        header: self.header.take().unwrap_or_default(),
        rows: mem::take(&mut self.rows),
      },
    ))
  }
}

#[cfg(test)]
mod tests {
  use super::*;

  #[test]
  fn parse_markdown_table_with_inline_cells() {
    let nodes = parse_full(
      "| name | value |\n| :--- | ---: |\n| **bold** | ![alt](https://example.com/a.png) |\n",
    );

    assert_eq!(nodes.len(), 1);
    let MarkdownNode::Table { table } = &nodes[0].node else {
      panic!("expected table node");
    };

    assert_eq!(table.alignments, vec![TableAlign::Start, TableAlign::End]);
    assert_eq!(table.header.cells.len(), 2);
    assert_eq!(table.rows.len(), 1);
    assert!(matches!(table.rows[0].cells[0].inlines[0], InlineNode::Text { .. }));
    assert!(matches!(table.rows[0].cells[1].inlines[0], InlineNode::Image { .. }));
  }

  #[test]
  fn parse_checkbox_accepts_uppercase_and_lowercase_checked_markers() {
    assert_eq!(parse_checkbox("[x] done"), Some((TodoState::Checked, "done".to_string())));
    assert_eq!(parse_checkbox("[X] done"), Some((TodoState::Checked, "done".to_string())));
  }

  #[test]
  fn todo_items_keep_inline_content_after_checkbox_prefix() {
    let nodes = parse_full("- [ ] **bold** ![alt](https://example.com/a.png)\n");

    assert_eq!(nodes.len(), 1);
    let MarkdownNode::TodoList { items } = &nodes[0].node else {
      panic!("expected todo list node");
    };

    assert_eq!(items.len(), 1);
    assert_eq!(items[0].state, TodoState::Unchecked);
    assert!(matches!(
      items[0].inlines.as_slice(),
      [
        InlineNode::Text { content, style },
        InlineNode::Text { content: gap, style: gap_style },
        InlineNode::Image { alt, .. }
      ] if content == "bold" && style.bold && gap == " " && !gap_style.bold && alt == "alt"
    ));
  }
}
