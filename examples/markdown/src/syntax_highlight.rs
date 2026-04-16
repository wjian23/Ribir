//! Syntax highlighting for code blocks using syntect

use std::cell::{Ref, RefCell};

use ribir::prelude::*;
use syntect::{easy::HighlightLines, highlighting::ThemeSet, parsing::SyntaxSet};

/// Highlight code and return AttributedText with syntax colors
pub fn highlight_code_to_attributed_text(code: &str, language: &str) -> AttributedText {
  let ps = SyntaxSet::load_defaults_newlines();
  let ts = ThemeSet::load_defaults();

  // Find the syntax for the language
  let syntax = if language.is_empty() {
    ps.find_syntax_plain_text()
  } else {
    ps.find_syntax_by_token(language)
      .unwrap_or_else(|| ps.find_syntax_plain_text())
  };

  let mut highlighter = HighlightLines::new(syntax, &ts.themes["base16-ocean.dark"]);
  let mut builder = AttributedText::builder();

  for line in code.lines() {
    let line = format!("{}\n", line);
    let ranges = highlighter
      .highlight_line(&line, &ps)
      .unwrap_or_default();

    for (style, text) in ranges {
      if !text.is_empty() {
        let color = style.foreground;
        let brush = Brush::from(Color::from_rgb(color.r, color.g, color.b));

        let span_style = SpanStyle { brush: Some(brush), ..Default::default() };

        builder.write_styled_text(text, span_style);
      }
    }
  }

  builder.build()
}

/// A code block widget that displays syntax-highlighted code
#[derive(Declare)]
pub struct CodeBlock {
  /// The syntax-highlighted attributed text
  pub text: AttributedText,
  /// Cached layout
  #[declare(skip)]
  layout: RefCell<Option<ParagraphLayoutRef>>,
}

impl CodeBlock {
  pub fn layout(&self) -> Option<Ref<'_, ParagraphLayoutRef>> {
    Ref::filter_map(self.layout.borrow(), |v| v.as_ref()).ok()
  }
}

impl Render for CodeBlock {
  fn measure(&self, clamp: BoxClamp, ctx: &mut MeasureCtx) -> Size {
    let text_style = Provider::of::<TextStyle>(ctx).unwrap();
    let text_align = Provider::of::<TextAlign>(ctx)
      .map(|align| *align)
      .unwrap_or_default();

    // Create paragraph style for code
    let paragraph_style = single_style_paragraph_style(&text_style, text_align);

    // Layout the attributed text
    let paragraph = AppCtx::text_services().paragraph(self.text.clone());
    let layout = paragraph.layout(&text_style, &paragraph_style, clamp);
    let size = layout.size();
    *self.layout.borrow_mut() = Some(layout);

    // Ensure we return finite dimensions
    Size::new(size.width.min(clamp.max.width), size.height.min(clamp.max.height))
  }

  #[inline]
  fn size_affected_by_child(&self) -> bool { false }

  fn paint(&self, ctx: &mut PaintingCtx) {
    let Some(layout) = self.layout.borrow().clone() else {
      return;
    };
    let brush = ctx.painter().fill_brush().clone();
    if !brush.is_visible() {
      return;
    }

    let payload = Resource::new(layout.draw_payload().clone());
    let rect = layout.draw_payload().bounds;
    ctx.painter().draw_text_payload(payload, rect);
  }

  #[cfg(feature = "debug")]
  fn debug_name(&self) -> std::borrow::Cow<'static, str> {
    std::borrow::Cow::Borrowed("code_block")
  }
}

/// Highlight code with the given language (legacy function, kept for
/// compatibility)
pub fn highlight_code(code: &str, language: &str) -> String {
  let ps = SyntaxSet::load_defaults_newlines();
  let ts = ThemeSet::load_defaults();

  let syntax = if language.is_empty() {
    ps.find_syntax_plain_text()
  } else {
    ps.find_syntax_by_token(language)
      .unwrap_or_else(|| ps.find_syntax_plain_text())
  };

  let mut highlighter = HighlightLines::new(syntax, &ts.themes["base16-ocean.dark"]);
  let mut result = String::new();

  for line in code.lines() {
    let line = format!("{}\n", line);
    let ranges = highlighter
      .highlight_line(&line, &ps)
      .unwrap_or_default();

    for (_style, text) in ranges {
      result.push_str(text);
    }
  }

  result
}

/// Create a widget for displaying highlighted code
pub fn create_code_widget(language: &str, code: &str) -> Widget<'static> {
  let highlighted_text = highlight_code_to_attributed_text(code, language);

  fn_widget! {
    @CodeBlock {
      text: highlighted_text.clone(),
      background: Color::from_rgb(0x24, 0x29, 0x33),
      radius: Radius::all(10.0),
      padding: EdgeInsets::all(12.0),
      margin: EdgeInsets::symmetrical(4.0, 0.0),
    }
  }
  .into_widget()
}
