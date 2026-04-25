use ribir_algo::CowArc;

use crate::{
  paragraph::{TextByteIndex, TextInlineBox, TextRange, TextSpan},
  style::SpanStyle,
};

/// A read-only rich text value made of one logical string plus styled byte
/// ranges.
#[derive(Debug, Clone, PartialEq)]
pub struct AttributedText<Brush> {
  pub text: CowArc<str>,
  pub spans: Box<[TextSpan<Brush>]>,
  pub inline_boxes: Box<[TextInlineBox]>,
}

impl<Brush> Default for AttributedText<Brush> {
  fn default() -> Self { Self::plain("") }
}

impl<Brush> AttributedText<Brush> {
  #[inline]
  pub fn plain(text: impl Into<CowArc<str>>) -> Self {
    Self {
      text: text.into(),
      spans: Vec::new().into_boxed_slice(),
      inline_boxes: Vec::new().into_boxed_slice(),
    }
  }

  #[inline]
  pub fn styled(text: impl Into<CowArc<str>>, style: SpanStyle<Brush>) -> Self {
    let text = text.into();
    let spans = if text.is_empty() {
      Default::default()
    } else {
      vec![TextSpan { range: TextRange::new(0, text.len()), style }].into_boxed_slice()
    };
    Self { text, spans, inline_boxes: Vec::new().into_boxed_slice() }
  }

  #[inline]
  pub fn from_parts(
    text: impl Into<CowArc<str>>, spans: impl Into<Box<[TextSpan<Brush>]>>,
  ) -> Self {
    Self::from_parts_with_inline_boxes(text, spans, Vec::new().into_boxed_slice())
  }

  #[inline]
  pub fn from_parts_with_inline_boxes(
    text: impl Into<CowArc<str>>, spans: impl Into<Box<[TextSpan<Brush>]>>,
    inline_boxes: impl Into<Box<[TextInlineBox]>>,
  ) -> Self {
    Self { text: text.into(), spans: spans.into(), inline_boxes: inline_boxes.into() }
  }

  #[inline]
  pub fn builder() -> AttributedTextBuilder<Brush> { AttributedTextBuilder::default() }

  #[inline]
  pub fn len_bytes(&self) -> usize { self.text.len() }

  #[inline]
  pub fn is_empty(&self) -> bool { self.text.is_empty() }
}

#[derive(Debug, Clone, PartialEq)]
pub struct AttributedTextBuilder<Brush> {
  text: String,
  spans: Vec<TextSpan<Brush>>,
  inline_boxes: Vec<TextInlineBox>,
}

impl<Brush> Default for AttributedTextBuilder<Brush> {
  fn default() -> Self { Self { text: String::new(), spans: Vec::new(), inline_boxes: Vec::new() } }
}

impl<Brush> AttributedTextBuilder<Brush> {
  #[inline]
  pub fn push_text(mut self, text: impl AsRef<str>) -> Self {
    self.write_text(text);
    self
  }

  #[inline]
  pub fn write_text(&mut self, text: impl AsRef<str>) -> &mut Self {
    self.text.push_str(text.as_ref());
    self
  }

  #[inline]
  pub fn push_styled_text(mut self, text: impl AsRef<str>, style: SpanStyle<Brush>) -> Self {
    self.write_styled_text(text, style);
    self
  }

  #[inline]
  pub fn write_styled_text(&mut self, text: impl AsRef<str>, style: SpanStyle<Brush>) -> &mut Self {
    let text = text.as_ref();
    if text.is_empty() {
      return self;
    }

    let start = self.text.len();
    self.text.push_str(text);
    let end = self.text.len();
    self
      .spans
      .push(TextSpan { range: TextRange::new(start, end), style });
    self
  }

  #[inline]
  pub fn append(mut self, text: AttributedText<Brush>) -> Self {
    self.write_attributed_text(text);
    self
  }

  #[inline]
  pub fn push_inline_box(mut self, width: f32, height: f32, id: u64) -> Self {
    self.write_inline_box(width, height, id);
    self
  }

  #[inline]
  pub fn write_inline_box(&mut self, width: f32, height: f32, id: u64) -> &mut Self {
    self.inline_boxes.push(TextInlineBox {
      id,
      index: TextByteIndex(self.text.len()),
      width,
      height,
    });
    self
  }

  pub fn write_attributed_text(&mut self, text: AttributedText<Brush>) -> &mut Self {
    let AttributedText { text, spans, inline_boxes } = text;
    let offset = self.text.len();
    self.text.push_str(text.as_ref());
    self
      .spans
      .extend(spans.into_vec().into_iter().map(|mut span| {
        span.range.start.0 += offset;
        span.range.end.0 += offset;
        span
      }));
    self.inline_boxes.extend(
      inline_boxes
        .into_vec()
        .into_iter()
        .map(|mut inline_box| {
          inline_box.index.0 += offset;
          inline_box
        }),
    );
    self
  }

  #[inline]
  pub fn build(self) -> AttributedText<Brush> {
    AttributedText {
      text: self.text.into(),
      spans: self.spans.into_boxed_slice(),
      inline_boxes: self.inline_boxes.into_boxed_slice(),
    }
  }
}

impl<Brush> From<AttributedTextBuilder<Brush>> for AttributedText<Brush> {
  #[inline]
  fn from(value: AttributedTextBuilder<Brush>) -> Self { value.build() }
}

impl<Brush> From<CowArc<str>> for AttributedText<Brush> {
  #[inline]
  fn from(value: CowArc<str>) -> Self { Self::plain(value) }
}

impl<Brush> From<String> for AttributedText<Brush> {
  #[inline]
  fn from(value: String) -> Self { Self::plain(value) }
}

impl<Brush> From<&str> for AttributedText<Brush> {
  #[inline]
  fn from(value: &str) -> Self { Self::plain(value.to_owned()) }
}

#[cfg(test)]
mod tests {
  use super::*;

  #[test]
  fn builder_shifts_ranges_when_appending_text() {
    let rich = AttributedText::builder()
      .push_text("Hello ")
      .push_styled_text("Ribir", SpanStyle { brush: Some(7_u8), ..Default::default() })
      .append(AttributedText::styled("!", SpanStyle { brush: Some(9_u8), ..Default::default() }))
      .build();

    assert_eq!(&*rich.text, "Hello Ribir!");
    assert_eq!(rich.spans.len(), 2);
    assert_eq!(rich.spans[0].range, TextRange::new(6, 11));
    assert_eq!(rich.spans[1].range, TextRange::new(11, 12));
    assert_eq!(rich.spans[0].style.brush, Some(7));
    assert_eq!(rich.spans[1].style.brush, Some(9));
    assert!(rich.inline_boxes.is_empty());
  }

  #[test]
  fn from_parts_keeps_text_and_spans() {
    let spans = vec![TextSpan {
      range: TextRange::new(0, 1),
      style: SpanStyle { font_size: Some(18.), brush: Some(3_u8), ..Default::default() },
    }]
    .into_boxed_slice();

    let rich = AttributedText::from_parts("A", spans.clone());

    assert_eq!(&*rich.text, "A");
    assert_eq!(rich.spans, spans);
    assert!(rich.inline_boxes.is_empty());
  }

  #[test]
  fn builder_shifts_inline_box_indices_when_appending_text() {
    let rich = AttributedText::<u8>::builder()
      .push_text("A")
      .append(
        AttributedText::<u8>::builder()
          .push_inline_box(10., 12., 7)
          .push_text("B")
          .build(),
      )
      .build();

    assert_eq!(&*rich.text, "AB");
    assert_eq!(rich.inline_boxes.len(), 1);
    assert_eq!(rich.inline_boxes[0].id, 7);
    assert_eq!(rich.inline_boxes[0].index, TextByteIndex(1));
  }
}
