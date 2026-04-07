//! Markdown parser that converts markdown text to document tree
//!
//! Provides full parsing and range-based parsing for incremental updates.

use pulldown_cmark::{Event, Options, Parser, Tag, TagEnd};

use crate::{
  DocumentNode, NodeId, allocate_node_id,
  components::{InlineNode, InlineStyle, MarkdownNode, TodoItem, TodoState},
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
  let mut in_list = false;
  let mut is_ordered_list = false;
  let mut list_items: Vec<Vec<InlineNode>> = Vec::new();
  let mut todo_list_items: Vec<TodoItem> = Vec::new();
  let mut is_todo_list = false;
  let mut heading_level: Option<u8> = None;

  let mut pending_image_url: Option<String> = None;
  let mut pending_image_alt: String = String::new();

  let mut in_code_block = false;
  let mut code_block_language = String::new();
  let mut code_block_content = String::new();
  let mut code_block_start: Option<usize> = None;

  let mut list_start: Option<usize> = None;

  let parser = Parser::new_ext(text, Options::empty()).into_offset_iter();

  // Helper: parse checkbox syntax
  let parse_checkbox = |text: &str| -> Option<(TodoState, String)> {
    let trimmed = text.trim_start();
    if trimmed.starts_with("[ ] ") || trimmed == "[ ]" {
      Some((TodoState::Unchecked, trimmed[4..].to_string()))
    } else if trimmed.starts_with("[x] ") || trimmed == "[x]" {
      Some((TodoState::Checked, trimmed[4..].to_string()))
    } else if trimmed.starts_with("[X] ") || trimmed == "[X]" {
      Some((TodoState::Checked, trimmed[4..].to_string()))
    } else if trimmed.starts_with("[-] ") || trimmed == "[-]" {
      Some((TodoState::Indeterminate, trimmed[4..].to_string()))
    } else {
      None
    }
  };

  // Helper: flush text with style
  let flush_text = |text: &str, style: &InlineStyle, inlines: &mut Vec<InlineNode>| {
    if !text.is_empty() {
      inlines.push(InlineNode::Text { content: text.to_string(), style: style.clone() });
    }
  };

  for (event, range) in parser {
    let event_start = base_offset + range.start;
    let event_end = base_offset + range.end;

    match event {
      Event::Start(tag) => match tag {
        Tag::Paragraph => {
          if in_list {
            flush_list_node(
              &mut nodes,
              is_todo_list,
              is_ordered_list,
              &mut todo_list_items,
              &mut list_items,
              &mut in_list,
              list_start.unwrap_or(event_start),
              event_start,
            );
          }
          block_start = Some(event_start);
          current_inlines.clear();
          current_style = InlineStyle::default();
        }
        Tag::Heading { level, .. } => {
          if in_list {
            flush_list_node(
              &mut nodes,
              is_todo_list,
              is_ordered_list,
              &mut todo_list_items,
              &mut list_items,
              &mut in_list,
              list_start.unwrap_or(event_start),
              event_start,
            );
          }
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
          if in_list {
            flush_list_node(
              &mut nodes,
              is_todo_list,
              is_ordered_list,
              &mut todo_list_items,
              &mut list_items,
              &mut in_list,
              list_start.unwrap_or(event_start),
              event_start,
            );
          }
          in_list = true;
          is_ordered_list = list_type.is_some();
          is_todo_list = false;
          list_start = Some(event_start);
          list_items.clear();
          todo_list_items.clear();
        }
        Tag::Item => {
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
          let start = block_start.unwrap_or(event_start);
          if !current_inlines.is_empty() && !in_list {
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
          flush_list_node(
            &mut nodes,
            is_todo_list,
            is_ordered_list,
            &mut todo_list_items,
            &mut list_items,
            &mut in_list,
            list_start.unwrap_or(event_start),
            event_end,
          );
          list_start = None;
        }
        TagEnd::Item => {
          if in_list {
            let text = current_inlines
              .iter()
              .filter_map(|inline| match inline {
                InlineNode::Text { content, .. } => Some(content.clone()),
                InlineNode::Image { alt, .. } => Some(format!("[{}]", alt)),
              })
              .collect::<String>();

            if let Some((state, todo_text)) = parse_checkbox(&text) {
              is_todo_list = true;
              todo_list_items.push(TodoItem { state, text: todo_text });
            } else {
              list_items.push(current_inlines.clone());
            }
            current_inlines.clear();
            current_style = InlineStyle::default();
          }
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

fn flush_list_node(
  nodes: &mut Vec<DocumentNode>, is_todo_list: bool, is_ordered_list: bool,
  todo_list_items: &mut Vec<TodoItem>, list_items: &mut Vec<Vec<InlineNode>>, in_list: &mut bool,
  start: usize, end: usize,
) {
  let node = if is_todo_list {
    MarkdownNode::TodoList { items: std::mem::take(todo_list_items) }
  } else if is_ordered_list {
    MarkdownNode::OrderedList { items: std::mem::take(list_items) }
  } else {
    MarkdownNode::UnorderedList { items: std::mem::take(list_items) }
  };
  nodes.push(make_node(allocate_node_id, node, start, end));
  *in_list = false;
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
