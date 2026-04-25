use std::{cell::RefCell, marker::PhantomData, ops::Range};

use ribir_core::{prelude::*, wrap_render::WrapRender};

use super::{text_glyphs::*, *};
use crate::selectable_area::{
  SelectionCoordinatorHandle, register_to_parent_selection_coordinator,
  wrap_with_default_selection_area,
};

class_names! {
  #[doc = "The name of the class for the text selection highlight rectangles"]
  TEXT_SELECTION,
}

/// Shared local selection movement granularity for text-backed widgets.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum MoveMode {
  Grapheme,
  Word,
  LineBoundary,
}

/// Reports whether a local selection move stayed inside the current selectable
/// or reached one of its local boundaries.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum BoundaryResult<P> {
  InBlock(P),
  AtStart,
  AtEnd,
  AtTop,
  AtBottom,
  Unavailable,
}

pub type TextPosition = CaretPosition;

#[derive(Copy, Clone, Debug, PartialEq, Eq, Default)]
pub struct TextSelectionRange {
  pub anchor: TextPosition,
  pub focus: TextPosition,
}

pub trait Selectable: 'static {
  type Position: Clone + PartialEq + Eq;
  type Range: Clone + PartialEq + Eq;

  fn hit_test(&self, point: Point) -> Option<Self::Position>;
  fn nearest_position(&self, point: Point) -> Option<Self::Position>;

  fn first_position(&self) -> Option<Self::Position>;
  fn last_position(&self) -> Option<Self::Position>;

  fn move_left(&self, pos: &Self::Position, mode: MoveMode) -> BoundaryResult<Self::Position>;

  fn move_right(&self, pos: &Self::Position, mode: MoveMode) -> BoundaryResult<Self::Position>;

  fn move_up(&self, pos: &Self::Position) -> BoundaryResult<Self::Position>;
  fn move_down(&self, pos: &Self::Position) -> BoundaryResult<Self::Position>;

  fn make_range(&self, anchor: Self::Position, focus: Self::Position) -> Self::Range;

  fn is_collapsed(&self, range: &Self::Range) -> bool;

  fn caret_rect(&self, pos: &Self::Position) -> Option<Rect>;

  fn selection_rects(&self, range: &Self::Range) -> Vec<Rect>;

  fn select_unit(&self, pos: &Self::Position, mode: MoveMode) -> Option<Self::Range>;
}

pub trait SelectableWithContent:
  Selectable<Position = TextPosition, Range = TextSelectionRange>
{
  fn selection_text(&self, range: &TextSelectionRange) -> String;

  fn selected_content(&self, range: &TextSelectionRange) -> crate::selectable_area::TextAreaData {
    self.selection_text(range).into()
  }
}

/// Text-backed selectable host used by `Input` and `TextArea`.
#[derive(Default, Declare)]
pub struct TextSelectable<T>
where
  T: 'static,
{
  #[declare(custom)]
  pub text: TextGlyphs<T>,
}

#[derive(Clone, Debug, PartialEq, Default)]
pub struct TextSelectionStyle {
  pub brush: Brush,
}

#[derive(Default, Declare)]
pub struct SelectionOverlay {
  #[declare(custom)]
  pub rects: Vec<Rect>,
}

#[derive(Default, Declare)]
pub struct SelectionHighlight {
  #[declare(skip)]
  rects: RefCell<Vec<Rect>>,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct TextSelection<T> {
  pub range: TextSelectionRange,
  _marker: PhantomData<fn() -> T>,
}

impl<T> Default for TextSelection<T> {
  fn default() -> Self { Self { range: TextSelectionRange::default(), _marker: PhantomData } }
}

impl TextSelectionRange {
  pub fn cluster_rg(&self) -> Range<usize> {
    let start = self.anchor.cluster.min(self.focus.cluster);
    let end = self.anchor.cluster.max(self.focus.cluster);
    Range { start, end }
  }

  pub fn splat(pos: TextPosition) -> Self { Self { anchor: pos, focus: pos } }
}

impl<T> TextSelectableDeclarer<T> {
  pub fn with_text<K: ?Sized>(&mut self, text: impl RInto<PipeValue<T>, K>) -> &mut Self {
    let text = text.r_into().map(TextGlyphs::new);
    self.text = Some(text);
    self
  }
}

impl SelectionOverlayDeclarer {
  pub fn with_rects<K: ?Sized>(&mut self, rects: impl RInto<PipeValue<Vec<Rect>>, K>) -> &mut Self {
    self.rects = Some(rects.r_into());
    self
  }
}

impl<T: BaseText + SyncLayoutVisualText + Clone + 'static> Compose for TextSelectable<T> {
  fn compose(this: impl StateWriter<Value = Self>) -> Widget<'static> {
    fn_widget! {
      let text_state = part_writer!(&mut this.text);
      let selectable = $writer(this);
      let root = FatObj::new(text_state.clone_writer()).into_widget();
        let hosted =
          fn_widget! { register_to_parent_selection_coordinator(root, selectable) }.into_widget();

      if Provider::of::<SelectionCoordinatorHandle>(BuildCtx::get()).is_some() {
        hosted
      } else {
        wrap_with_default_selection_area(hosted)
      }
    }
    .into_widget()
  }
}

fn is_move_by_word(event: &KeyboardEvent) -> bool {
  #[cfg(target_os = "macos")]
  return event.with_alt_key();
  #[cfg(not(target_os = "macos"))]
  return event.with_ctrl_key();
}

pub(crate) fn key_move_mode(event: &KeyboardEvent) -> MoveMode {
  if is_move_by_word(event) {
    MoveMode::Word
  } else if event.with_command_key() {
    MoveMode::LineBoundary
  } else {
    MoveMode::Grapheme
  }
}

impl<T: BaseText + SyncLayoutVisualText + 'static> Selectable for TextGlyphs<T> {
  type Position = TextPosition;
  type Range = TextSelectionRange;

  fn hit_test(&self, point: Point) -> Option<Self::Position> {
    self
      .glyphs()
      .and_then(|glyphs| glyphs.hit_test_position(point))
  }

  fn nearest_position(&self, point: Point) -> Option<Self::Position> {
    self
      .glyphs()
      .map(|glyphs| glyphs.nearest_position(point))
  }

  fn first_position(&self) -> Option<Self::Position> {
    self.glyphs().map(|_| TextPosition::default())
  }

  fn last_position(&self) -> Option<Self::Position> {
    self
      .glyphs()
      .map(|_| TextPosition::new(self.text().len()))
  }

  fn move_left(&self, pos: &Self::Position, mode: MoveMode) -> BoundaryResult<Self::Position> {
    let Some(glyphs) = self.glyphs() else {
      return BoundaryResult::Unavailable;
    };

    let next = match mode {
      MoveMode::Grapheme => glyphs.prev(*pos),
      MoveMode::Word => {
        let text = self.text();
        let mut rg = text.select_token(pos.cluster);
        if rg.start == pos.cluster && pos.cluster > 0 {
          rg = text.select_token(pos.cluster - 1);
        }
        TextPosition::new(rg.start)
      }
      MoveMode::LineBoundary => glyphs.line_begin(*pos),
    };

    if next != *pos {
      BoundaryResult::InBlock(next)
    } else if self.first_position() == Some(*pos) {
      BoundaryResult::AtStart
    } else {
      BoundaryResult::InBlock(next)
    }
  }

  fn move_right(&self, pos: &Self::Position, mode: MoveMode) -> BoundaryResult<Self::Position> {
    let Some(glyphs) = self.glyphs() else {
      return BoundaryResult::Unavailable;
    };

    let next = match mode {
      MoveMode::Grapheme => glyphs.next(*pos),
      MoveMode::Word => {
        let text = self.text();
        let mut rg = text.select_token(pos.cluster);
        if rg.end == pos.cluster && pos.cluster < text.len() {
          rg = text.select_token(pos.cluster + 1);
        }
        TextPosition::new(rg.end)
      }
      MoveMode::LineBoundary => glyphs.line_end(*pos),
    };

    if next != *pos {
      BoundaryResult::InBlock(next)
    } else if self.last_position() == Some(*pos) {
      BoundaryResult::AtEnd
    } else {
      BoundaryResult::InBlock(next)
    }
  }

  fn move_up(&self, pos: &Self::Position) -> BoundaryResult<Self::Position> {
    let Some(glyphs) = self.glyphs() else {
      return BoundaryResult::Unavailable;
    };

    let next = glyphs.up(*pos);
    if next != *pos { BoundaryResult::InBlock(next) } else { BoundaryResult::AtTop }
  }

  fn move_down(&self, pos: &Self::Position) -> BoundaryResult<Self::Position> {
    let Some(glyphs) = self.glyphs() else {
      return BoundaryResult::Unavailable;
    };

    let next = glyphs.down(*pos);
    if next != *pos { BoundaryResult::InBlock(next) } else { BoundaryResult::AtBottom }
  }

  fn make_range(&self, anchor: Self::Position, focus: Self::Position) -> Self::Range {
    TextSelectionRange { anchor, focus }
  }

  fn is_collapsed(&self, range: &Self::Range) -> bool { range.anchor == range.focus }

  fn caret_rect(&self, pos: &Self::Position) -> Option<Rect> {
    self.glyphs().map(|glyphs| glyphs.caret_box(*pos))
  }

  fn selection_rects(&self, range: &Self::Range) -> Vec<Rect> {
    self
      .glyphs()
      .map(|glyphs| glyphs.select_range(&range.cluster_rg()))
      .unwrap_or_default()
  }

  fn select_unit(&self, pos: &Self::Position, mode: MoveMode) -> Option<Self::Range> {
    match mode {
      MoveMode::Word => {
        let rg = self.text().select_token(pos.cluster);
        Some(TextSelectionRange {
          anchor: TextPosition::new(rg.start),
          focus: TextPosition::new(rg.end),
        })
      }
      MoveMode::LineBoundary => {
        let glyphs = self.glyphs()?;
        let anchor = glyphs.line_begin(*pos);
        let focus = glyphs.line_end(*pos);
        Some(TextSelectionRange { anchor, focus })
      }
      MoveMode::Grapheme => {
        let len = self.text().measure_bytes(pos.cluster, 1);
        let focus = TextPosition::new(pos.cluster + len);
        Some(TextSelectionRange { anchor: *pos, focus })
      }
    }
  }
}

impl<T: BaseText + SyncLayoutVisualText + 'static> Selectable for TextSelectable<T> {
  type Position = TextPosition;
  type Range = TextSelectionRange;

  fn hit_test(&self, point: Point) -> Option<Self::Position> {
    Selectable::hit_test(&self.text, point)
  }

  fn nearest_position(&self, point: Point) -> Option<Self::Position> {
    Selectable::nearest_position(&self.text, point)
  }

  fn first_position(&self) -> Option<Self::Position> { Selectable::first_position(&self.text) }

  fn last_position(&self) -> Option<Self::Position> { Selectable::last_position(&self.text) }

  fn move_left(&self, pos: &Self::Position, mode: MoveMode) -> BoundaryResult<Self::Position> {
    Selectable::move_left(&self.text, pos, mode)
  }

  fn move_right(&self, pos: &Self::Position, mode: MoveMode) -> BoundaryResult<Self::Position> {
    Selectable::move_right(&self.text, pos, mode)
  }

  fn move_up(&self, pos: &Self::Position) -> BoundaryResult<Self::Position> {
    Selectable::move_up(&self.text, pos)
  }

  fn move_down(&self, pos: &Self::Position) -> BoundaryResult<Self::Position> {
    Selectable::move_down(&self.text, pos)
  }

  fn make_range(&self, anchor: Self::Position, focus: Self::Position) -> Self::Range {
    Selectable::make_range(&self.text, anchor, focus)
  }

  fn is_collapsed(&self, range: &Self::Range) -> bool {
    Selectable::is_collapsed(&self.text, range)
  }

  fn caret_rect(&self, pos: &Self::Position) -> Option<Rect> {
    Selectable::caret_rect(&self.text, pos)
  }

  fn selection_rects(&self, range: &Self::Range) -> Vec<Rect> {
    Selectable::selection_rects(&self.text, range)
  }

  fn select_unit(&self, pos: &Self::Position, mode: MoveMode) -> Option<Self::Range> {
    Selectable::select_unit(&self.text, pos, mode)
  }
}

impl<T: BaseText + SyncLayoutVisualText + 'static> SelectableWithContent for TextSelectable<T> {
  fn selection_text(&self, range: &TextSelectionRange) -> String {
    self
      .text
      .text()
      .substr(range.cluster_rg())
      .to_string()
  }
}

impl SelectionOverlay {
  pub fn paint_rects(rects: &[Rect], ctx: &mut PaintingCtx) {
    let brush = Provider::of::<TextSelectionStyle>(ctx).map(|style| style.brush.clone());

    if let Some(brush) = brush
      && !rects.is_empty()
      && brush.is_visible()
    {
      let painter = ctx.painter();
      let rects = rects
        .iter()
        .copied()
        .filter(|rect| painter.intersection_paint_bounds(rect).is_some())
        .collect::<Vec<_>>();
      painter.save();
      painter.set_fill_brush(brush);
      rects.into_iter().for_each(|rect| {
        painter.rect(&rect, true).fill();
      });
      painter.restore();
    }
  }
}

pub struct SelectionRectProvider {
  pub rects: Box<SelectionRectsFn>,
}

impl SelectionRectProvider {
  pub fn new(rects: impl Fn(&MeasureCtx) -> Vec<Rect> + 'static) -> Self {
    Self { rects: Box::new(rects) }
  }
}

type SelectionRectsFn = dyn Fn(&MeasureCtx) -> Vec<Rect>;

impl Render for SelectionHighlight {
  fn measure(&self, clamp: BoxClamp, ctx: &mut MeasureCtx) -> Size {
    *self.rects.borrow_mut() = Provider::of::<SelectionRectProvider>(ctx)
      .map(|provider| (provider.rects)(ctx))
      .unwrap_or_default();
    clamp.max
  }

  fn paint(&self, ctx: &mut PaintingCtx) {
    let brush = Provider::of::<TextSelectionStyle>(ctx).map(|style| style.brush.clone());

    if let Some(brush) = brush
      && !self.rects.borrow().is_empty()
      && brush.is_visible()
    {
      let painter = ctx.painter();
      let rects = self
        .rects
        .borrow()
        .iter()
        .copied()
        .filter(|rect| painter.intersection_paint_bounds(rect).is_some())
        .collect::<Vec<_>>();
      painter.save();
      painter.set_fill_brush(brush);
      rects.into_iter().for_each(|rect| {
        painter.rect(&rect, true).fill();
      });
      painter.restore();
    }
  }
}

impl<T: SyncLayoutVisualText + 'static> WrapRender for TextSelection<T> {
  fn paint(&self, host: &dyn Render, ctx: &mut PaintingCtx) {
    let rects = Provider::of::<TextGlyphs<T>>(ctx).and_then(|text| {
      text
        .glyphs()
        .map(|glyphs| glyphs.select_range(&self.range.cluster_rg()))
    });

    if let Some(rects) = rects {
      SelectionOverlay::paint_rects(&rects, ctx);
    }

    host.paint(ctx);
  }

  fn wrapper_dirty_phase(&self) -> DirtyPhase { DirtyPhase::Paint }
}

impl<'c, T: SyncLayoutVisualText + 'static> ComposeChild<'c> for TextSelection<T> {
  type Child = Widget<'c>;

  fn compose_child(this: impl StateWriter<Value = Self>, child: Self::Child) -> Widget<'c> {
    WrapRender::combine_child(this, child)
  }
}

impl<T> std::ops::Deref for TextSelection<T> {
  type Target = TextSelectionRange;

  fn deref(&self) -> &Self::Target { &self.range }
}

impl<T> std::ops::DerefMut for TextSelection<T> {
  fn deref_mut(&mut self) -> &mut Self::Target { &mut self.range }
}

impl<T> std::ops::Deref for TextSelectable<T> {
  type Target = TextGlyphs<T>;

  fn deref(&self) -> &Self::Target { &self.text }
}

impl<T> std::ops::DerefMut for TextSelectable<T> {
  fn deref_mut(&mut self) -> &mut Self::Target { &mut self.text }
}
