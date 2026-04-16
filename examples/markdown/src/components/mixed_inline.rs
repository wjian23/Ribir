use ribir::{
  core::{
    text::{LineHeight, TextRange},
    wrap_render::WrapRender,
  },
  prelude::*,
};

use super::{InlineNode, InlineStyle, TODO_MARKER_SIZE, create_span_child};

const DEFAULT_INLINE_IMAGE_SIZE: f32 = 40.0;
const DEFAULT_INLINE_IMAGE_PLACEHOLDER: &str = "    ";
const TODO_MARKER_PLACEHOLDER: &str = "    ";
const INLINE_CODE_PLACEHOLDER_PADDING: &str = " ";
const INLINE_CODE_RADIUS: f32 = 6.0;
const INLINE_CODE_VERTICAL_PADDING: f32 = 2.0;
const INLINE_CODE_HORIZONTAL_PADDING: f32 = 6.0;

struct MixedInlinePlan {
  children: Vec<RichTextChild>,
  overlays: Vec<InlineOverlaySlot>,
}

enum InlineOverlayKind {
  Image { url: String },
  TodoMarker { state: super::TodoState },
  CodeChip { content: String },
}

struct InlineOverlaySlot {
  kind: InlineOverlayKind,
  range: TextRange,
  placeholder_size: f32,
}

struct InlineOverlayAnchor {
  rich_text: Stateful<RichText>,
  range: TextRange,
  fallback_width: f32,
}

impl Clone for InlineOverlayAnchor {
  fn clone(&self) -> Self {
    Self {
      rich_text: self.rich_text.clone_writer(),
      range: self.range,
      fallback_width: self.fallback_width,
    }
  }
}

impl InlineOverlayAnchor {
  fn range_rect(&self) -> Option<Rect> {
    let rich_text = self.rich_text.read();
    let layout = rich_text.layout()?;
    layout
      .selection_rects(self.range)
      .into_iter()
      .reduce(|bounds, rect| bounds.union(&rect))
  }

  fn slot_width(&self) -> f32 {
    self
      .range_rect()
      .map(|rect| rect.width())
      .filter(|width| *width > 0.)
      .unwrap_or(self.fallback_width)
      .max(1.0)
  }
}

#[derive(Clone, Declare)]
struct InlineImageBox {
  anchor: InlineOverlayAnchor,
  aspect_ratio: f32,
}

impl InlineImageBox {
  fn desired_size(&self) -> Size {
    let width = self.anchor.slot_width();
    let height = (width * self.aspect_ratio).max(1.0);
    Size::new(width, height)
  }
}

impl<'c> ComposeChild<'c> for InlineImageBox {
  type Child = Widget<'c>;

  fn compose_child(this: impl StateWriter<Value = Self>, child: Self::Child) -> Widget<'c> {
    WrapRender::combine_child(this, child)
  }
}

impl WrapRender for InlineImageBox {
  fn measure(&self, clamp: BoxClamp, host: &dyn Render, ctx: &mut MeasureCtx) -> Size {
    host.measure(BoxClamp::fixed_size(clamp.clamp(self.desired_size())), ctx)
  }

  fn size_affected_by_child(&self, _host: &dyn Render) -> bool { false }

  fn wrapper_dirty_phase(&self) -> DirtyPhase { DirtyPhase::Layout }
}

#[derive(Clone, Declare)]
struct InlineSquareBox {
  anchor: InlineOverlayAnchor,
}

impl InlineSquareBox {
  fn desired_size(&self) -> Size {
    let side = self.anchor.slot_width();
    Size::splat(side)
  }
}

impl<'c> ComposeChild<'c> for InlineSquareBox {
  type Child = Widget<'c>;

  fn compose_child(this: impl StateWriter<Value = Self>, child: Self::Child) -> Widget<'c> {
    WrapRender::combine_child(this, child)
  }
}

impl WrapRender for InlineSquareBox {
  fn measure(&self, clamp: BoxClamp, host: &dyn Render, ctx: &mut MeasureCtx) -> Size {
    host.measure(BoxClamp::fixed_size(clamp.clamp(self.desired_size())), ctx)
  }

  fn size_affected_by_child(&self, _host: &dyn Render) -> bool { false }

  fn wrapper_dirty_phase(&self) -> DirtyPhase { DirtyPhase::Layout }
}

#[derive(Clone, Declare)]
struct InlineRectBox {
  anchor: InlineOverlayAnchor,
}

impl InlineRectBox {
  fn desired_size(&self) -> Size {
    self
      .anchor
      .range_rect()
      .map(|rect| rect.size)
      .unwrap_or_else(|| Size::new(self.anchor.fallback_width.max(1.0), 1.0))
  }
}

impl<'c> ComposeChild<'c> for InlineRectBox {
  type Child = Widget<'c>;

  fn compose_child(this: impl StateWriter<Value = Self>, child: Self::Child) -> Widget<'c> {
    WrapRender::combine_child(this, child)
  }
}

impl WrapRender for InlineRectBox {
  fn measure(&self, clamp: BoxClamp, host: &dyn Render, ctx: &mut MeasureCtx) -> Size {
    host.measure(BoxClamp::fixed_size(clamp.clamp(self.desired_size())), ctx)
  }

  fn size_affected_by_child(&self, _host: &dyn Render) -> bool { false }

  fn wrapper_dirty_phase(&self) -> DirtyPhase { DirtyPhase::Layout }
}

/// A reusable mixed inline layout that reserves image slots with placeholder
/// glyphs so text layout and wrapping can stay entirely text-driven.
pub struct MixedInline {
  inlines: Vec<InlineNode>,
  inline_image_size: f32,
  placeholder_text: CowArc<str>,
}

impl MixedInline {
  pub fn new(inlines: Vec<InlineNode>) -> Self {
    Self {
      inlines,
      inline_image_size: DEFAULT_INLINE_IMAGE_SIZE,
      placeholder_text: DEFAULT_INLINE_IMAGE_PLACEHOLDER.into(),
    }
  }

  pub fn with_inline_image_size(mut self, size: f32) -> Self {
    self.inline_image_size = size.max(1.0);
    self
  }

  pub fn with_placeholder_text(mut self, text: impl Into<CowArc<str>>) -> Self {
    let text = text.into();
    self.placeholder_text =
      if text.is_empty() { DEFAULT_INLINE_IMAGE_PLACEHOLDER.into() } else { text };
    self
  }

  pub fn into_widget(self) -> Widget<'static> {
    let Self { inlines, inline_image_size, placeholder_text } = self;
    let plan = build_mixed_inline_plan(&inlines, inline_image_size, &placeholder_text);
    let rich_text = Stateful::new(RichText::default());
    let text_widget = ComposeChild::compose_child(rich_text.clone_writer(), plan.children);

    let overlays = plan
      .overlays
      .into_iter()
      .map(|slot| render_inline_overlay(slot, rich_text.clone_writer()))
      .collect::<Vec<_>>();

    fn_widget! {
      @Stack {
        fit: StackFit::Passthrough,
        @ { text_widget }
        @ { overlays }
      }
    }
    .into_widget()
  }
}

fn build_mixed_inline_plan(
  inlines: &[InlineNode], inline_image_size: f32, placeholder_text: &CowArc<str>,
) -> MixedInlinePlan {
  let mut children = Vec::new();
  let mut overlays = Vec::new();
  let mut current_text = String::new();
  let mut current_style = InlineStyle::default();
  let mut text_cursor = 0;

  for inline in inlines {
    match inline {
      InlineNode::Text { content, style } => {
        if style == &current_style || current_text.is_empty() {
          current_text.push_str(content);
          current_style = style.clone();
        } else {
          push_inline_run(
            &mut children,
            &mut overlays,
            &mut text_cursor,
            &mut current_text,
            &current_style,
          );
          current_text.push_str(content);
          current_style = style.clone();
        }
      }
      InlineNode::Image { url, alt: _ } => {
        push_inline_run(
          &mut children,
          &mut overlays,
          &mut text_cursor,
          &mut current_text,
          &current_style,
        );
        push_overlay(
          &mut children,
          &mut overlays,
          &mut text_cursor,
          placeholder_text.clone(),
          inline_image_size,
          InlineOverlayKind::Image { url: url.clone() },
        );
      }
      InlineNode::TodoMarker { state } => {
        push_inline_run(
          &mut children,
          &mut overlays,
          &mut text_cursor,
          &mut current_text,
          &current_style,
        );
        push_overlay(
          &mut children,
          &mut overlays,
          &mut text_cursor,
          CowArc::from(TODO_MARKER_PLACEHOLDER),
          TODO_MARKER_SIZE,
          InlineOverlayKind::TodoMarker { state: state.clone() },
        );
      }
    }
  }

  push_inline_run(
    &mut children,
    &mut overlays,
    &mut text_cursor,
    &mut current_text,
    &current_style,
  );

  MixedInlinePlan { children, overlays }
}

fn push_inline_run(
  children: &mut Vec<RichTextChild>, overlays: &mut Vec<InlineOverlaySlot>,
  text_cursor: &mut usize, current_text: &mut String, current_style: &InlineStyle,
) {
  if current_text.is_empty() {
    return;
  }

  if current_style.code {
    let placeholder_text =
      format!("{INLINE_CODE_PLACEHOLDER_PADDING}{current_text}{INLINE_CODE_PLACEHOLDER_PADDING}");
    push_overlay_slot(
      children,
      overlays,
      text_cursor,
      placeholder_text.len(),
      0.0,
      create_code_placeholder(&placeholder_text),
      InlineOverlayKind::CodeChip { content: current_text.clone() },
    );
  } else {
    *text_cursor += current_text.len();
    children.push(create_span_child(current_text, current_style));
  }
  current_text.clear();
}

fn push_overlay(
  children: &mut Vec<RichTextChild>, overlays: &mut Vec<InlineOverlaySlot>,
  text_cursor: &mut usize, placeholder_text: CowArc<str>, placeholder_size: f32,
  kind: InlineOverlayKind,
) {
  let placeholder_len = placeholder_text.len();
  push_overlay_slot(
    children,
    overlays,
    text_cursor,
    placeholder_len,
    placeholder_size,
    create_overlay_placeholder(placeholder_text, placeholder_size),
    kind,
  );
}

fn push_overlay_slot(
  children: &mut Vec<RichTextChild>, overlays: &mut Vec<InlineOverlaySlot>,
  text_cursor: &mut usize, placeholder_len: usize, placeholder_size: f32,
  placeholder_child: RichTextChild, kind: InlineOverlayKind,
) {
  let start = *text_cursor;
  let end = start + placeholder_len;
  children.push(placeholder_child);
  overlays.push(InlineOverlaySlot { kind, range: TextRange::new(start, end), placeholder_size });
  *text_cursor = end;
}

fn create_overlay_placeholder(text: CowArc<str>, placeholder_size: f32) -> RichTextChild {
  RichTextChild::Span(Box::new(Span {
    text: PipeValue::Value(text),
    font_size: Some(PipeValue::Value(placeholder_size)),
    text_line_height: Some(PipeValue::Value(LineHeight::Px(placeholder_size))),
    ..Default::default()
  }))
}

fn create_code_placeholder(text: &str) -> RichTextChild {
  RichTextChild::Span(Box::new(Span {
    text: PipeValue::Value(CowArc::from(text.to_string())),
    font: Some(PipeValue::Value(FontFace {
      families: Box::new([FontFamily::Monospace]),
      ..Default::default()
    })),
    foreground: Some(PipeValue::Value(Brush::from(Color::TRANSPARENT))),
    ..Default::default()
  }))
}

fn render_inline_overlay(
  slot: InlineOverlaySlot, rich_text: Stateful<RichText>,
) -> Widget<'static> {
  let anchor =
    InlineOverlayAnchor { rich_text, range: slot.range, fallback_width: slot.placeholder_size };
  match slot.kind {
    InlineOverlayKind::Image { url } => render_inline_image_overlay(anchor, url),
    InlineOverlayKind::TodoMarker { state } => render_todo_marker_overlay(anchor, state),
    InlineOverlayKind::CodeChip { content } => render_inline_code_overlay(anchor, content),
  }
}

fn render_inline_image_overlay(anchor: InlineOverlayAnchor, url: String) -> Widget<'static> {
  let state = crate::image_loader::request_image_state(&url);
  let anchor_for_widget = anchor.clone();
  fn_widget! {
    @InParentLayout {
      @CustomAnchor {
        data: anchor,
        anchor: overlay_anchor,
        @ {
          pipe!($read(state);).map(move |_| match $read(state).clone() {
            crate::image_loader::ImageState::Loaded(img) => {
              let aspect_ratio = img.height() as f32 / img.width().max(1) as f32;
              let image_anchor = anchor_for_widget.clone();
              fn_widget! {
                @InlineImageBox {
                  anchor: image_anchor,
                  aspect_ratio,
                  @FittedBox {
                    box_fit: BoxFit::Contain,
                    @ { img }
                  }
                }
              }
              .into_widget()
            }
            crate::image_loader::ImageState::Loading => Void::default().into_widget(),
            crate::image_loader::ImageState::Error(_) => Void::default().into_widget(),
          })
        }
      }
    }
  }
  .into_widget()
}

fn render_todo_marker_overlay(
  anchor: InlineOverlayAnchor, state: super::TodoState,
) -> Widget<'static> {
  let anchor_for_widget = anchor.clone();
  let marker = super::render_todo_marker(state);
  fn_widget! {
    @InParentLayout {
      @CustomAnchor {
        data: anchor,
        anchor: overlay_anchor,
        @InlineSquareBox {
          anchor: anchor_for_widget,
          @ { marker }
        }
      }
    }
  }
  .into_widget()
}

fn render_inline_code_overlay(anchor: InlineOverlayAnchor, content: String) -> Widget<'static> {
  let anchor_for_widget = anchor.clone();
  fn_widget! {
    let palette = Palette::of(BuildCtx::get());
    let typography = TypographyTheme::of(BuildCtx::get());
    let mut text_style = typography.body_medium.text.clone();
    text_style.font_face = FontFace {
      families: Box::new([FontFamily::Monospace]),
      ..Default::default()
    };

    @InParentLayout {
      @CustomAnchor {
        data: anchor,
        anchor: overlay_anchor,
        @InlineRectBox {
          anchor: anchor_for_widget,
          @Container {
            clamp: BoxClamp::EXPAND_BOTH,
            background: palette.surface_container_highest(),
            border: Border::all(BorderSide::new(1., palette.outline_variant().into())),
            radius: Radius::all(INLINE_CODE_RADIUS),
            padding: EdgeInsets::symmetrical(
              INLINE_CODE_VERTICAL_PADDING,
              INLINE_CODE_HORIZONTAL_PADDING,
            ),
            @Text {
              text: content.clone(),
              text_style,
              foreground: palette.on_surface(),
            }
          }
        }
      }
    }
  }
  .into_widget()
}

fn overlay_anchor(
  data: &InlineOverlayAnchor, child_size: Size, _clamp: BoxClamp, _ctx: &mut PlaceCtx,
) -> Anchor {
  let rect = data
    .range_rect()
    .unwrap_or_else(|| Rect::from_size(child_size));
  let x = rect.center().x - child_size.width * 0.5;
  let y = rect.center().y - child_size.height * 0.5;
  Anchor::left_top(x, y)
}

#[cfg(test)]
mod tests {
  use super::*;

  fn plain_text(content: &str) -> InlineNode {
    InlineNode::Text { content: content.to_string(), style: InlineStyle::default() }
  }

  fn code_text(content: &str) -> InlineNode {
    InlineNode::Text {
      content: content.to_string(),
      style: InlineStyle { code: true, ..InlineStyle::default() },
    }
  }

  #[test]
  fn mixed_inline_plan_tracks_image_placeholder_ranges() {
    let plan = build_mixed_inline_plan(
      &[
        plain_text("before "),
        InlineNode::Image {
          url: "https://example.com/a.png".to_string(),
          alt: "image".to_string(),
        },
        plain_text(" after"),
      ],
      48.0,
      &CowArc::from("▣"),
    );

    assert_eq!(plan.children.len(), 3);
    assert_eq!(plan.overlays.len(), 1);
    assert_eq!(
      plan.overlays[0].range,
      TextRange::new("before ".len(), "before ".len() + "▣".len())
    );
  }

  #[test]
  fn mixed_inline_plan_merges_adjacent_text_runs_with_same_style() {
    let plan =
      build_mixed_inline_plan(&[plain_text("foo"), plain_text("bar")], 40.0, &CowArc::from("▣"));

    assert_eq!(plan.children.len(), 1);
    assert!(plan.overlays.is_empty());
  }

  #[test]
  fn mixed_inline_plan_supports_todo_markers() {
    let plan = build_mixed_inline_plan(
      &[InlineNode::TodoMarker { state: super::super::TodoState::Checked }],
      40.0,
      &CowArc::from("▣"),
    );

    assert_eq!(plan.children.len(), 1);
    assert_eq!(plan.overlays.len(), 1);
    assert!(matches!(
      &plan.overlays[0].kind,
      InlineOverlayKind::TodoMarker { state: super::super::TodoState::Checked }
    ));
  }

  #[test]
  fn mixed_inline_plan_promotes_inline_code_to_code_chip_overlay() {
    let plan = build_mixed_inline_plan(&[code_text("Stateful<T>")], 40.0, &CowArc::from("▣"));

    assert_eq!(plan.children.len(), 1);
    assert_eq!(plan.overlays.len(), 1);
    assert!(matches!(
      &plan.overlays[0].kind,
      InlineOverlayKind::CodeChip { content } if content == "Stateful<T>"
    ));
  }
}
