use ribir_types::{Point, Rect, Vector};

use crate::{
  font::FontFaceId,
  paragraph::{ClusterIndex, LineIndex},
  style::TextDecoration,
};

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub struct GlyphId(pub u16);

#[derive(Debug, Clone, PartialEq, Default)]
pub struct TextDrawPayload<Brush> {
  pub bounds: Rect,
  pub origin_offset: Vector,
  pub backgrounds: Box<[DrawTextBackground<Brush>]>,
  pub runs: Box<[DrawGlyphRun<Brush>]>,
  pub inline_boxes: Box<[DrawInlineBox]>,
  pub decorations: Box<[DrawTextDecoration<Brush>]>,
}

#[derive(Debug, Clone, PartialEq)]
pub struct DrawGlyphRun<Brush> {
  pub face_id: FontFaceId,
  pub logical_font_size: f32,
  pub brush: Option<Brush>,
  pub glyphs: Box<[DrawGlyph]>,
}

#[derive(Debug, Clone, PartialEq)]
pub struct DrawGlyph {
  pub glyph_id: GlyphId,
  pub cluster: ClusterIndex,
  pub baseline_origin: Point,
  pub advance: Vector,
}

#[derive(Debug, Clone, PartialEq)]
pub struct DrawTextDecoration<Brush> {
  pub decoration: TextDecoration,
  pub brush: Option<Brush>,
  pub rect: Rect,
}

#[derive(Debug, Clone, PartialEq)]
pub struct DrawTextBackground<Brush> {
  pub line: LineIndex,
  pub brush: Option<Brush>,
  pub rect: Rect,
  pub radius: f32,
}

#[derive(Debug, Clone, PartialEq)]
pub struct DrawInlineBox {
  pub line: LineIndex,
  pub id: u64,
  pub rect: Rect,
}
