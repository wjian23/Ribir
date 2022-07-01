use crate::{Color, ShallowImage};
use lyon_tessellation::StrokeOptions;
use text::{Em, FontFace, FontSize, Pixel};

/// Encapsulates the text style for painting.
#[derive(Clone, Debug, PartialEq)]
pub struct TextStyle {
  /// The size of glyphs (in logical pixels) to use when painting the text.
  pub font_size: FontSize,
  /// The style drawn as a foreground for the text.
  pub foreground: Brush,
  /// The font face to use when painting the text.
  // todo: use ids instead of
  pub font_face: FontFace,
  /// Not support now.
  pub letter_space: Option<Pixel>,
  /// The path style(fill or stroke) to use when painting.
  pub path_style: PathStyle,
  /// The factor use to multiplied by the font size to specify the text line
  /// height.
  pub line_height: Option<Em>,
}

bitflags::bitflags! {
  /// - Repeat mode repeat the image to full tile the path, if the image greater
  /// than the path, image will be clipped.
  /// - Cover mode resize the image to cover the entire path, even if it has to
  /// stretch the image or cut a little bit off one of the edges
  pub struct TileMode: u8 {
    const REPEAT_X = 0b00000001;
    const REPEAT_Y = 0b00000010;
    const REPEAT_BOTH = Self::REPEAT_X.bits | Self::REPEAT_Y.bits;
    const COVER_X = 0b00000100;
    const COVER_Y = 0b00001000;
    const COVER_BOTH = Self::COVER_X.bits | Self::COVER_Y.bits;
    const REPEAT_X_COVER_Y = Self::REPEAT_X.bits | Self::COVER_Y.bits;
    const COVER_X_REPEAT_Y = Self::COVER_X.bits | Self::REPEAT_Y.bits;
  }
}

impl TileMode {
  #[inline]
  pub fn is_cover_mode(&self) -> bool { self.bits & (TileMode::COVER_BOTH.bits) > 0 }
}

#[derive(Clone, Debug, PartialEq)]
pub enum Brush {
  Color(Color),
  Image {
    img: ShallowImage,
    tile_mode: TileMode,
  },
  Gradient, // todo,
}

/// The style to paint path, maybe fill or stroke.
#[derive(Clone, Copy, PartialEq, Debug)]
pub enum PathStyle {
  /// Fill the path.
  Fill,
  /// Stroke path with line width.
  Stroke(StrokeOptions),
}

impl Default for TextStyle {
  fn default() -> Self {
    Self {
      font_size: FontSize::Pixel(14.0.into()),
      foreground: Color::BLACK.into(),
      font_face: Default::default(),
      letter_space: None,
      path_style: PathStyle::Fill,
      line_height: None,
    }
  }
}
impl From<Color> for Brush {
  #[inline]
  fn from(c: Color) -> Self { Brush::Color(c) }
}

impl Default for Brush {
  #[inline]
  fn default() -> Self { Brush::Color(Color::BLACK) }
}
