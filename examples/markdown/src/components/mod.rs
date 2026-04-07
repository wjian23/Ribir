//! Markdown rendering components

use ribir::{
  core::text::{FontStyle, FontWeight},
  prelude::*,
};

use crate::syntax_highlight;

/// Represents the checked state of a todo list item
#[derive(Clone, Debug, Hash, PartialEq, Eq)]
pub enum TodoState {
  Unchecked,
  Checked,
  Indeterminate,
}

/// Represents a single todo list item
#[derive(Clone, Debug, Hash, PartialEq, Eq)]
pub struct TodoItem {
  pub state: TodoState,
  pub text: String,
}

/// Represents inline formatting style for text spans
#[derive(Clone, Debug, Hash, PartialEq, Eq, Default)]
pub struct InlineStyle {
  pub bold: bool,
  pub italic: bool,
  pub link_url: Option<String>,
  pub code: bool,
}

/// Represents an inline element within text
#[derive(Clone, Debug, Hash, PartialEq, Eq)]
pub enum InlineNode {
  /// Plain text with optional styling
  Text { content: String, style: InlineStyle },
  /// Inline image that breaks text flow
  Image { url: String, alt: String },
}

/// Represents a node in the markdown document tree
#[derive(Clone, Debug, Hash, PartialEq, Eq)]
pub enum MarkdownNode {
  /// Paragraph with mixed inline content (text only, no images)
  Paragraph { inlines: Vec<InlineNode> },
  /// Heading with level (1-6) and mixed inline content
  Heading { level: u8, inlines: Vec<InlineNode> },
  /// Unordered list with items
  UnorderedList { items: Vec<Vec<InlineNode>> },
  /// Ordered list with items
  OrderedList { items: Vec<Vec<InlineNode>> },
  /// Todo list with checkbox items
  TodoList { items: Vec<TodoItem> },
  /// Standalone image block (not inline in text)
  Image { url: String, alt: String },
  /// Horizontal rule (---)
  HorizontalRule,
  /// Code block with language and code
  CodeBlock { language: String, code: String },
}

/// Render a markdown block node to widget
pub fn render_block(node: MarkdownNode) -> Widget<'static> {
  match node {
    MarkdownNode::Paragraph { inlines } => render_paragraph_with_inlines(inlines),
    MarkdownNode::Heading { level, inlines } => render_heading(level, inlines),
    MarkdownNode::UnorderedList { items } => render_unordered_list(items),
    MarkdownNode::OrderedList { items } => render_ordered_list(items),
    MarkdownNode::TodoList { items } => render_todo_list(items),
    MarkdownNode::Image { url, alt } => render_image_block(&url, &alt),
    MarkdownNode::HorizontalRule => render_horizontal_rule(),
    MarkdownNode::CodeBlock { language, code } => render_code_block(&language, &code),
  }
}

/// Render a paragraph with mixed inline content (text only, no images)
fn render_paragraph_with_inlines(inlines: Vec<InlineNode>) -> Widget<'static> {
  let children: Vec<RichTextChild> = build_span_children(&inlines);
  let stateful = Stateful::new(RichText::default());
  ComposeChild::compose_child(stateful, children)
}

/// Render a standalone image block
fn render_image_block(url: &str, alt: &str) -> Widget<'static> {
  let url = url.to_string();
  let alt = alt.to_string();

  fn_widget! {
    @Column {
      align_items: Align::Center,
      padding: EdgeInsets::all(8.0),
      @ {
        crate::image_loader::render_markdown_image(InlineNode::Image { url, alt })
      }
    }
  }
  .into_widget()
}

/// Render a horizontal rule (---)
fn render_horizontal_rule() -> Widget<'static> {
  fn_widget! {
    @Container {
      width: f32::INFINITY,
      height: 2.0,
      margin: EdgeInsets::new(16.0, 0.0, 16.0, 16.0),
      background: Color::from_u32(0xFFE0E0E0),
    }
  }
  .into_widget()
}

/// Render a heading with mixed inline content
fn render_heading(level: u8, inlines: Vec<InlineNode>) -> Widget<'static> {
  let children = build_span_children(&inlines);
  // Use heading font sizes based on level
  let font_size = match level {
    1 => 32.0,
    2 => 28.0,
    3 => 24.0,
    4 => 20.0,
    5 => 18.0,
    6 => 16.0,
    _ => 16.0,
  };

  let stateful = Stateful::new(RichText::default());
  let widget = ComposeChild::compose_child(stateful, children);

  // Wrap with font_size using FatObj
  fn_widget! {
    let font_size = font_size;
    @FatObj {
      font_size,
      @ { widget }
    }
  }
  .into_widget()
}

/// Build RichText children from inline nodes
fn build_span_children(inlines: &[InlineNode]) -> Vec<RichTextChild> {
  let mut children = Vec::new();
  let mut current_text = String::new();
  let mut current_style = InlineStyle::default();

  for inline in inlines {
    match inline {
      InlineNode::Text { content, style } => {
        if style == &current_style || current_text.is_empty() {
          current_text.push_str(content);
          current_style = style.clone();
        } else {
          // Push accumulated text with previous style
          if !current_text.is_empty() {
            children.push(create_span_child(&current_text, &current_style));
            current_text.clear();
          }
          current_text = content.clone();
          current_style = style.clone();
        }
      }
      InlineNode::Image { url: _, alt } => {
        // Flush current text
        if !current_text.is_empty() {
          children.push(create_span_child(&current_text, &current_style));
          current_text.clear();
        }
        // Add image widget - we'll need to handle this specially
        // For now, add a placeholder text with the alt
        children.push(RichTextChild::Text(PipeValue::Value(CowArc::from(format!("[{}]", alt)))));
      }
    }
  }

  if !current_text.is_empty() {
    children.push(create_span_child(&current_text, &current_style));
  }

  children
}

/// Create a RichTextChild from text and style
fn create_span_child(text: &str, style: &InlineStyle) -> RichTextChild {
  if style.link_url.is_some() {
    // Link styled text
    let _url = style.link_url.clone().unwrap();
    let text_str = text.to_string();
    RichTextChild::Span(Box::new(Span {
      text: PipeValue::Value(CowArc::from(text_str)),
      foreground: Some(PipeValue::Value(Brush::from(Color::from_u32(0xFF0066CC)))),
      text_decoration: Some(PipeValue::Value(TextDecorationStyle {
        decoration: TextDecoration::UNDERLINE,
        decoration_color: Some(Color::from_u32(0xFF0066CC)),
      })),
      ..Default::default()
    }))
  } else if style.bold || style.italic {
    // Bold/italic styled text
    let mut font_face = FontFace::default();
    if style.bold {
      font_face.weight = FontWeight::BOLD;
    }
    if style.italic {
      font_face.style = FontStyle::Italic;
    }
    RichTextChild::Span(Box::new(Span {
      text: PipeValue::Value(CowArc::from(text.to_string())),
      font: Some(PipeValue::Value(font_face)),
      ..Default::default()
    }))
  } else if style.code {
    // Code styled text
    RichTextChild::Span(Box::new(Span {
      text: PipeValue::Value(CowArc::from(text.to_string())),
      font: Some(PipeValue::Value(FontFace {
        families: Box::new([FontFamily::Monospace]),
        ..Default::default()
      })),
      foreground: Some(PipeValue::Value(Brush::from(Color::from_u32(0xFF8B0000)))),
      ..Default::default()
    }))
  } else {
    // Plain text
    RichTextChild::Text(PipeValue::Value(CowArc::from(text.to_string())))
  }
}

fn render_unordered_list(items: Vec<Vec<InlineNode>>) -> Widget<'static> {
  fn_widget! {
    @Column {
      @ {
        items.into_iter().map(|item_inlines| {
          let text = flatten_inlines(&item_inlines);
          fn_widget! {
            @Text { text: format!("• {}", text) }
          }
          .into_widget()
        }).collect::<Vec<_>>()
      }
    }
  }
  .into_widget()
}

fn render_ordered_list(items: Vec<Vec<InlineNode>>) -> Widget<'static> {
  fn_widget! {
    @Column {
      @ {
        items.into_iter().enumerate().map(|(i, item_inlines)| {
          let text = flatten_inlines(&item_inlines);
          fn_widget! {
            @Text { text: format!("{}. {}", i + 1, text) }
          }
          .into_widget()
        }).collect::<Vec<_>>()
      }
    }
  }
  .into_widget()
}

fn render_todo_list(items: Vec<TodoItem>) -> Widget<'static> {
  fn_widget! {
    @Column {
      @ {
        items.into_iter().map(|todo_item| {
          let text = todo_item.text.clone();
          let is_checked = matches!(todo_item.state, TodoState::Checked);
          let is_indeterminate = matches!(todo_item.state, TodoState::Indeterminate);

          fn_widget! {
            @ListItem {
              @Checkbox {
                checked: is_checked,
                indeterminate: is_indeterminate,
              }
              @ListItemHeadline { @ { text } }
            }
          }
          .into_widget()
        }).collect::<Vec<_>>()
      }
    }
  }
  .into_widget()
}

fn render_code_block(language: &str, code: &str) -> Widget<'static> {
  syntax_highlight::create_code_widget(language, code)
}

/// Flatten inline nodes to a simple string for list items
fn flatten_inlines(inlines: &[InlineNode]) -> String {
  inlines
    .iter()
    .filter_map(|inline| match inline {
      InlineNode::Text { content, .. } => Some(content.clone()),
      InlineNode::Image { alt, .. } => Some(format!("[{}]", alt)),
    })
    .collect()
}
