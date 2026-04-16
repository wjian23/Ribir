//! Markdown rendering components

mod mixed_inline;
mod table;

pub use mixed_inline::MixedInline;
use ribir::{
  core::text::{FontStyle, FontWeight},
  prelude::*,
};

use crate::syntax_highlight;

pub(super) const TODO_MARKER_SIZE: f32 = 18.0;
const TODO_MARKER_ICON_SIZE: f32 = 12.0;
const TODO_MARKER_RADIUS: f32 = 4.0;
const TODO_MARKER_GAP_TEXT: &str = " ";
const LIST_BLOCK_INDENT: f32 = 24.0;
const LIST_ITEM_SPACING: f32 = 6.0;
const TODO_INDETERMINATE_BAR_WIDTH: f32 = 8.0;
const TODO_INDETERMINATE_BAR_HEIGHT: f32 = 2.0;

/// Represents the checked state of a todo list item
#[derive(Clone, Debug, Hash, PartialEq, Eq)]
pub enum TodoState {
  Unchecked,
  Checked,
  Indeterminate,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum MarkdownTodoMarkerVisual {
  Unchecked,
  Checked,
  Indeterminate,
}

impl MarkdownTodoMarkerVisual {
  fn from_state(state: &TodoState) -> Self {
    match state {
      TodoState::Unchecked => Self::Unchecked,
      TodoState::Checked => Self::Checked,
      TodoState::Indeterminate => Self::Indeterminate,
    }
  }
}

/// Represents a single todo list item
#[derive(Clone, Debug, Hash, PartialEq, Eq)]
pub struct TodoItem {
  pub state: TodoState,
  pub inlines: Vec<InlineNode>,
}

#[derive(Clone, Debug, Hash, PartialEq, Eq, Default)]
pub enum TableAlign {
  #[default]
  Start,
  Center,
  End,
}

#[derive(Clone, Debug, Hash, PartialEq, Eq, Default)]
pub struct TableCell {
  pub inlines: Vec<InlineNode>,
}

#[derive(Clone, Debug, Hash, PartialEq, Eq, Default)]
pub struct TableRow {
  pub cells: Vec<TableCell>,
}

#[derive(Clone, Debug, Hash, PartialEq, Eq, Default)]
pub struct MarkdownTableData {
  pub alignments: Vec<TableAlign>,
  pub header: TableRow,
  pub rows: Vec<TableRow>,
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
  /// Markdown todo marker rendered inline with text layout
  TodoMarker { state: TodoState },
}

/// Represents a node in the markdown document tree
#[derive(Clone, Debug, Hash, PartialEq, Eq)]
pub enum MarkdownNode {
  /// Paragraph with mixed inline content
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
  /// Markdown table with header/body rows.
  Table { table: MarkdownTableData },
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
    MarkdownNode::Table { table } => table::render_table(table),
  }
}

/// Render a paragraph with mixed inline content
fn render_paragraph_with_inlines(inlines: Vec<InlineNode>) -> Widget<'static> {
  render_inline_content(inlines)
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
    let palette = Palette::of(BuildCtx::get());
    @Container {
      width: f32::INFINITY,
      height: 1.0,
      background: palette.outline_variant(),
    }
  }
  .into_widget()
}

/// Render a heading with mixed inline content
fn render_heading(level: u8, inlines: Vec<InlineNode>) -> Widget<'static> {
  let widget = render_inline_content(inlines);

  fn_widget! {
    let typography = TypographyTheme::of(BuildCtx::get());
    let palette = Palette::of(BuildCtx::get());
    let text_style = match level {
      1 => typography.headline_large.text.clone(),
      2 => typography.headline_medium.text.clone(),
      3 => typography.headline_small.text.clone(),
      4 => typography.title_large.text.clone(),
      5 => typography.title_medium.text.clone(),
      _ => typography.title_small.text.clone(),
    };
    @FatObj {
      text_style,
      foreground: palette.on_surface(),
      @ { widget }
    }
  }
  .into_widget()
}

fn render_inline_content(inlines: Vec<InlineNode>) -> Widget<'static> {
  if inlines.is_empty() {
    Void::default().into_widget()
  } else {
    MixedInline::new(inlines).into_widget()
  }
}

/// Create a RichTextChild from text and style
pub(super) fn create_span_child(text: &str, style: &InlineStyle) -> RichTextChild {
  if style.link_url.is_some() {
    let palette = Palette::of(BuildCtx::get());
    let text_str = text.to_string();
    RichTextChild::Span(Box::new(Span {
      text: PipeValue::Value(CowArc::from(text_str)),
      foreground: Some(PipeValue::Value(Brush::from(palette.primary()))),
      text_decoration: Some(PipeValue::Value(TextDecorationStyle {
        decoration: TextDecoration::UNDERLINE,
        decoration_color: Some(palette.primary()),
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
    let palette = Palette::of(BuildCtx::get());
    RichTextChild::Span(Box::new(Span {
      text: PipeValue::Value(CowArc::from(text.to_string())),
      font: Some(PipeValue::Value(FontFace {
        families: Box::new([FontFamily::Monospace]),
        ..Default::default()
      })),
      foreground: Some(PipeValue::Value(Brush::from(palette.on_surface()))),
      ..Default::default()
    }))
  } else {
    // Plain text
    RichTextChild::Text(PipeValue::Value(CowArc::from(text.to_string())))
  }
}

fn render_unordered_list(items: Vec<Vec<InlineNode>>) -> Widget<'static> {
  fn_widget! {
    @Flex {
      direction: Direction::Vertical,
      item_gap: LIST_ITEM_SPACING,
      padding: EdgeInsets::only_left(LIST_BLOCK_INDENT),
      @ {
        items.into_iter().map(|item_inlines| {
          render_prefixed_inline_item("• ".to_string(), item_inlines)
        }).collect::<Vec<_>>()
      }
    }
  }
  .into_widget()
}

fn render_ordered_list(items: Vec<Vec<InlineNode>>) -> Widget<'static> {
  fn_widget! {
    @Flex {
      direction: Direction::Vertical,
      item_gap: LIST_ITEM_SPACING,
      padding: EdgeInsets::only_left(LIST_BLOCK_INDENT),
      @ {
        items.into_iter().enumerate().map(|(i, item_inlines)| {
          render_prefixed_inline_item(format!("{}. ", i + 1), item_inlines)
        }).collect::<Vec<_>>()
      }
    }
  }
  .into_widget()
}

fn render_todo_list(items: Vec<TodoItem>) -> Widget<'static> {
  fn_widget! {
    @Flex {
      direction: Direction::Vertical,
      item_gap: LIST_ITEM_SPACING,
      padding: EdgeInsets::only_left(LIST_BLOCK_INDENT),
      @ {
        items.into_iter().map(|todo_item| {
          render_inline_content(todo_item_inlines(todo_item))
        }).collect::<Vec<_>>()
      }
    }
  }
  .into_widget()
}

fn render_code_block(language: &str, code: &str) -> Widget<'static> {
  syntax_highlight::create_code_widget(language, code)
}

fn todo_item_inlines(todo_item: TodoItem) -> Vec<InlineNode> {
  let mut inlines = Vec::with_capacity(todo_item.inlines.len() + 2);
  inlines.push(InlineNode::TodoMarker { state: todo_item.state });
  if !todo_item.inlines.is_empty() {
    inlines.push(InlineNode::Text {
      content: TODO_MARKER_GAP_TEXT.to_string(),
      style: InlineStyle::default(),
    });
    inlines.extend(todo_item.inlines);
  }
  inlines
}

fn render_todo_marker(state: TodoState) -> Widget<'static> {
  fn_widget! {
    let palette = Palette::of(BuildCtx::get());
    let visual = MarkdownTodoMarkerVisual::from_state(&state);
    let (background, border_color, icon_color) = match visual {
      MarkdownTodoMarkerVisual::Unchecked => (
        palette.surface(),
        palette.outline_variant(),
        palette.on_surface_variant(),
      ),
      MarkdownTodoMarkerVisual::Checked => (
        palette.primary(),
        palette.primary(),
        palette.on_primary(),
      ),
      MarkdownTodoMarkerVisual::Indeterminate => (
        palette.surface_container_highest(),
        palette.outline(),
        palette.on_surface_variant(),
      ),
    };

    @Container {
      clamp: BoxClamp::EXPAND_BOTH,
      background,
      radius: Radius::all(TODO_MARKER_RADIUS),
      border: Border::all(BorderSide::new(1., border_color.into())),
      @ {
        render_todo_marker_icon(visual, icon_color)
      }
    }
  }
  .into_widget()
}

fn render_todo_marker_icon(visual: MarkdownTodoMarkerVisual, color: Color) -> Widget<'static> {
  match visual {
    MarkdownTodoMarkerVisual::Unchecked => Void::default().into_widget(),
    MarkdownTodoMarkerVisual::Checked => fn_widget! {
      @Icon {
        x: AnchorX::center(),
        y: AnchorY::center(),
        text_line_height: TODO_MARKER_ICON_SIZE,
        foreground: color,
        @ { svg_registry::get_or_default("check") }
      }
    }
    .into_widget(),
    MarkdownTodoMarkerVisual::Indeterminate => fn_widget! {
      @Container {
        x: AnchorX::center(),
        y: AnchorY::center(),
        size: Size::new(TODO_INDETERMINATE_BAR_WIDTH, TODO_INDETERMINATE_BAR_HEIGHT),
        radius: Radius::all(TODO_INDETERMINATE_BAR_HEIGHT * 0.5),
        background: color,
      }
    }
    .into_widget(),
  }
}

fn render_prefixed_inline_item(prefix: String, inlines: Vec<InlineNode>) -> Widget<'static> {
  let mut prefixed = Vec::with_capacity(inlines.len() + 1);
  prefixed.push(InlineNode::Text { content: prefix, style: InlineStyle::default() });
  prefixed.extend(inlines);
  render_inline_content(prefixed)
}

#[cfg(test)]
mod tests {
  use super::*;

  #[test]
  fn markdown_todo_marker_visual_tracks_state() {
    assert_eq!(
      MarkdownTodoMarkerVisual::from_state(&TodoState::Unchecked),
      MarkdownTodoMarkerVisual::Unchecked
    );
    assert_eq!(
      MarkdownTodoMarkerVisual::from_state(&TodoState::Checked),
      MarkdownTodoMarkerVisual::Checked
    );
    assert_eq!(
      MarkdownTodoMarkerVisual::from_state(&TodoState::Indeterminate),
      MarkdownTodoMarkerVisual::Indeterminate
    );
  }

  #[test]
  fn todo_item_inlines_prefix_marker_and_preserve_content() {
    let inlines = todo_item_inlines(TodoItem {
      state: TodoState::Checked,
      inlines: vec![InlineNode::Text { content: "task".into(), style: InlineStyle::default() }],
    });

    assert!(matches!(inlines[0], InlineNode::TodoMarker { state: TodoState::Checked }));
    assert!(
      matches!(inlines[1], InlineNode::Text { ref content, .. } if content == TODO_MARKER_GAP_TEXT)
    );
    assert!(matches!(inlines[2], InlineNode::Text { ref content, .. } if content == "task"));
  }
}
