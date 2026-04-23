use std::{
  any::Any,
  cell::{Cell, RefCell},
  rc::Rc,
};

use ribir_core::prelude::*;

use crate::{
  input::{BoundaryResult, MoveMode, SelectableWithContent, TextPosition, TextSelectionRange},
  prelude::*,
};

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct SelectableRange<Position> {
  pub anchor: Position,
  pub focus: Position,
}

pub type TextSelectableRange = SelectableRange<TextPosition>;
pub type TextAreaData = SelectableAreaData;
type DefaultSelectionEntry<Data = TextAreaData> = dyn SelectionEntry<TextPosition, Data>;

pub trait SelectionCoordinator<Data = TextAreaData>: 'static {
  fn register_selection_entry(&self, entry: Box<DefaultSelectionEntry<Data>>);
  fn unregister_selection_entry(&self, track_id: &TrackId);
  fn selection_data(&self) -> Vec<Data>;
}

pub struct SelectionCoordinatorHandle<Data = TextAreaData>(Rc<dyn SelectionCoordinator<Data>>);

impl<Data> Clone for SelectionCoordinatorHandle<Data> {
  fn clone(&self) -> Self { Self(self.0.clone()) }
}

impl<Data: 'static> SelectionCoordinatorHandle<Data> {
  fn new(scope: impl SelectionCoordinator<Data> + 'static) -> Self {
    Self(Rc::new(scope) as Rc<dyn SelectionCoordinator<Data>>)
  }

  pub fn register_selection_entry(&self, entry: Box<DefaultSelectionEntry<Data>>) {
    self.0.register_selection_entry(entry);
  }

  pub fn unregister_selection_entry(&self, track_id: &TrackId) {
    self.0.unregister_selection_entry(track_id);
  }

  pub fn selection_data(&self) -> Vec<Data> { self.0.selection_data() }
}

pub struct SelectableAreaData {
  pub text: CowArc<str>,
  pub image: Option<Resource<PixelImage>>,
  pub custom_data: Option<Box<dyn Any>>,
}

impl Default for SelectableAreaData {
  fn default() -> Self { Self { text: "".into(), image: None, custom_data: None } }
}

impl From<CowArc<str>> for SelectableAreaData {
  fn from(text: CowArc<str>) -> Self { Self { text, ..Self::default() } }
}

impl From<String> for SelectableAreaData {
  fn from(text: String) -> Self { Self::from(CowArc::<str>::from(text)) }
}

impl From<Resource<PixelImage>> for SelectableAreaData {
  fn from(image: Resource<PixelImage>) -> Self { Self { image: Some(image), ..Self::default() } }
}

impl From<Option<Resource<PixelImage>>> for SelectableAreaData {
  fn from(image: Option<Resource<PixelImage>>) -> Self { Self { image, ..Self::default() } }
}

impl From<Box<dyn Any>> for SelectableAreaData {
  fn from(custom_data: Box<dyn Any>) -> Self {
    Self { custom_data: Some(custom_data), ..Self::default() }
  }
}

impl SelectableAreaData {
  pub fn is_empty(&self) -> bool {
    self.text.is_empty() && self.image.is_none() && self.custom_data.is_none()
  }

  pub fn custom_data_as<T: Any>(&self) -> Option<&T> {
    self.custom_data.as_ref()?.downcast_ref::<T>()
  }
}

pub fn selection_text(selection: &[TextAreaData]) -> Option<String> {
  let text = selection
    .iter()
    .filter_map(|fragment| (!fragment.text.is_empty()).then_some(fragment.text.as_ref()))
    .collect::<Vec<_>>()
    .join("\n");
  (!text.is_empty()).then_some(text)
}

pub fn selection_single_image(selection: &[TextAreaData]) -> Option<Resource<PixelImage>> {
  match selection {
    [fragment] if fragment.text.is_empty() && fragment.custom_data.is_none() => {
      fragment.image.clone()
    }
    _ => None,
  }
}

pub fn copy_selection_data_to_clipboard(selection: &[TextAreaData]) -> bool {
  let clipboard = AppCtx::clipboard();
  let mut clipboard = clipboard.borrow_mut();
  clipboard.clear().is_ok()
    && if let Some(text) = selection_text(selection) {
      clipboard.write_text(&text).is_ok()
    } else if let Some(image) = selection_single_image(selection) {
      clipboard.write_img(&image).is_ok()
    } else {
      false
    }
}

pub trait SelectionEntry<Position = TextPosition, Data = TextAreaData> {
  fn track_id(&self) -> TrackId;

  fn widget_id(&self) -> Option<WidgetId> { self.track_id().get() }

  fn hit_test(&self, point: Point) -> Option<Position>;
  fn nearest_position(&self, point: Point) -> Option<Position>;
  fn first_position(&self) -> Option<Position>;
  fn last_position(&self) -> Option<Position>;
  fn move_left(&self, pos: &Position, mode: MoveMode) -> BoundaryResult<Position>;
  fn move_right(&self, pos: &Position, mode: MoveMode) -> BoundaryResult<Position>;
  fn move_up(&self, pos: &Position) -> BoundaryResult<Position>;
  fn move_down(&self, pos: &Position) -> BoundaryResult<Position>;
  fn make_range(&self, anchor: Position, focus: Position) -> SelectableRange<Position>;
  fn is_collapsed(&self, range: &SelectableRange<Position>) -> bool;
  fn caret_rect(&self, pos: &Position) -> Option<Rect>;
  fn selection_rects(&self, range: &SelectableRange<Position>) -> Vec<Rect>;
  fn selection_data(&self, range: &SelectableRange<Position>) -> Option<Data>;
  fn select_unit(&self, pos: &Position, mode: MoveMode) -> Option<SelectableRange<Position>>;
}

struct StatefulTextSelectionEntry<S: 'static> {
  state: Box<dyn StateWriter<Value = S>>,
  track_id: TrackId,
}

fn as_text_selection_range(range: &TextSelectableRange) -> TextSelectionRange {
  TextSelectionRange { anchor: range.anchor, focus: range.focus }
}

fn from_text_selection_range(range: TextSelectionRange) -> TextSelectableRange {
  TextSelectableRange { anchor: range.anchor, focus: range.focus }
}

impl<S> SelectionEntry for StatefulTextSelectionEntry<S>
where
  S: SelectableWithContent + 'static,
{
  fn track_id(&self) -> TrackId { self.track_id.clone() }

  fn hit_test(&self, point: Point) -> Option<TextPosition> { self.state.read().hit_test(point) }

  fn nearest_position(&self, point: Point) -> Option<TextPosition> {
    self.state.read().nearest_position(point)
  }

  fn first_position(&self) -> Option<TextPosition> { self.state.read().first_position() }

  fn last_position(&self) -> Option<TextPosition> { self.state.read().last_position() }

  fn move_left(&self, pos: &TextPosition, mode: MoveMode) -> BoundaryResult<TextPosition> {
    self.state.read().move_left(pos, mode)
  }

  fn move_right(&self, pos: &TextPosition, mode: MoveMode) -> BoundaryResult<TextPosition> {
    self.state.read().move_right(pos, mode)
  }

  fn move_up(&self, pos: &TextPosition) -> BoundaryResult<TextPosition> {
    self.state.read().move_up(pos)
  }

  fn move_down(&self, pos: &TextPosition) -> BoundaryResult<TextPosition> {
    self.state.read().move_down(pos)
  }

  fn make_range(&self, anchor: TextPosition, focus: TextPosition) -> TextSelectableRange {
    from_text_selection_range(self.state.read().make_range(anchor, focus))
  }

  fn is_collapsed(&self, range: &TextSelectableRange) -> bool {
    self
      .state
      .read()
      .is_collapsed(&as_text_selection_range(range))
  }

  fn caret_rect(&self, pos: &TextPosition) -> Option<Rect> { self.state.read().caret_rect(pos) }

  fn selection_rects(&self, range: &TextSelectableRange) -> Vec<Rect> {
    self
      .state
      .read()
      .selection_rects(&as_text_selection_range(range))
  }

  fn selection_data(&self, range: &TextSelectableRange) -> Option<TextAreaData> {
    Some(
      self
        .state
        .read()
        .selected_content(&as_text_selection_range(range)),
    )
  }

  fn select_unit(&self, pos: &TextPosition, mode: MoveMode) -> Option<TextSelectableRange> {
    self
      .state
      .read()
      .select_unit(pos, mode)
      .map(from_text_selection_range)
  }
}

#[derive(Declare, Clone)]
pub struct SelectableImage {
  pub image: Resource<PixelImage>,
  #[declare(skip)]
  size: Cell<Option<Size>>,
}

struct StatefulImageSelectionEntry {
  state: Box<dyn StateWriter<Value = SelectableImage>>,
  track_id: TrackId,
}

struct SelectableAreaCoordinator {
  state: Box<dyn StateWriter<Value = SelectableArea>>,
}

impl SelectionCoordinator for SelectableAreaCoordinator {
  fn register_selection_entry(&self, entry: Box<DefaultSelectionEntry<TextAreaData>>) {
    self.state.read().register(entry);
  }

  fn unregister_selection_entry(&self, track_id: &TrackId) {
    self.state.read().unregister(track_id);
  }

  fn selection_data(&self) -> Vec<TextAreaData> { self.state.read().selection_data() }
}

impl SelectionEntry for StatefulImageSelectionEntry {
  fn track_id(&self) -> TrackId { self.track_id.clone() }

  fn hit_test(&self, _: Point) -> Option<TextPosition> { Some(TextPosition::default()) }

  fn nearest_position(&self, _: Point) -> Option<TextPosition> { Some(TextPosition::default()) }

  fn first_position(&self) -> Option<TextPosition> { Some(TextPosition::default()) }

  fn last_position(&self) -> Option<TextPosition> { Some(TextPosition::new(1)) }

  fn move_left(&self, _: &TextPosition, _: MoveMode) -> BoundaryResult<TextPosition> {
    BoundaryResult::AtStart
  }

  fn move_right(&self, _: &TextPosition, _: MoveMode) -> BoundaryResult<TextPosition> {
    BoundaryResult::AtEnd
  }

  fn move_up(&self, _: &TextPosition) -> BoundaryResult<TextPosition> { BoundaryResult::AtTop }

  fn move_down(&self, _: &TextPosition) -> BoundaryResult<TextPosition> { BoundaryResult::AtBottom }

  fn make_range(&self, _: TextPosition, _: TextPosition) -> TextSelectableRange {
    TextSelectableRange { anchor: TextPosition::default(), focus: TextPosition::new(1) }
  }

  fn is_collapsed(&self, _: &TextSelectableRange) -> bool { false }

  fn caret_rect(&self, _: &TextPosition) -> Option<Rect> {
    self.state.read().size.get().map(Rect::from_size)
  }

  fn selection_rects(&self, _: &TextSelectableRange) -> Vec<Rect> {
    self
      .state
      .read()
      .size
      .get()
      .map(Rect::from_size)
      .into_iter()
      .collect()
  }

  fn selection_data(&self, _: &TextSelectableRange) -> Option<TextAreaData> {
    Some(self.state.read().image.clone().into())
  }

  fn select_unit(&self, _: &TextPosition, _: MoveMode) -> Option<TextSelectableRange> {
    Some(self.make_range(TextPosition::default(), TextPosition::new(1)))
  }
}

fn boxed_text_selection_entry<S>(
  state: impl StateWriter<Value = S> + 'static, track_id: TrackId,
) -> Box<DefaultSelectionEntry>
where
  S: SelectableWithContent + 'static,
{
  Box::new(StatefulTextSelectionEntry { state: Box::new(state), track_id })
}

fn boxed_image_selection_entry(
  state: impl StateWriter<Value = SelectableImage> + 'static, track_id: TrackId,
) -> Box<DefaultSelectionEntry> {
  Box::new(StatefulImageSelectionEntry { state: Box::new(state), track_id })
}

pub(crate) fn register_to_parent_selection_coordinator<'c, S>(
  child: Widget<'c>, state: impl StateWriter<Value = S> + 'static,
) -> Widget<'c>
where
  S: SelectableWithContent + 'static,
{
  let Some(coordinator) =
    Provider::of::<SelectionCoordinatorHandle>(BuildCtx::get()).map(|area| area.clone())
  else {
    return child;
  };

  let mut child = FatObj::new(child);
  let track_id = child.track_id();
  coordinator.register_selection_entry(boxed_text_selection_entry(state, track_id.clone()));
  child.on_disposed(move |_| coordinator.unregister_selection_entry(&track_id));
  child.into_widget()
}

fn register_image_to_parent_selection_coordinator<'c>(
  child: Widget<'c>, state: impl StateWriter<Value = SelectableImage> + 'static,
) -> Widget<'c> {
  let Some(coordinator) =
    Provider::of::<SelectionCoordinatorHandle>(BuildCtx::get()).map(|area| area.clone())
  else {
    return child;
  };

  let mut child = FatObj::new(child);
  let track_id = child.track_id();
  coordinator.register_selection_entry(boxed_image_selection_entry(state, track_id.clone()));
  child.on_disposed(move |_| coordinator.unregister_selection_entry(&track_id));
  child.into_widget()
}

pub(crate) fn wrap_with_default_selection_area<'c>(child: Widget<'c>) -> Widget<'c> {
  SelectableArea::compose_child(Stateful::new(SelectableArea::default()), vec![child])
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct SelectableAreaHit {
  pub widget_id: WidgetId,
  pub position: TextPosition,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
struct SelectableAreaPosition<Position = TextPosition> {
  widget_id: WidgetId,
  position: Position,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
struct SelectableAreaSelection<Position = TextPosition> {
  anchor: SelectableAreaPosition<Position>,
  focus: SelectableAreaPosition<Position>,
}

#[derive(Declare)]
pub struct SelectableArea {
  #[declare(default = true)]
  pub default_copy: bool,
  #[declare(skip)]
  entries: RefCell<Vec<Box<DefaultSelectionEntry>>>,
  #[declare(skip)]
  selection: Cell<Option<SelectableAreaSelection>>,
  #[declare(skip)]
  selection_rects: RefCell<Vec<Rect>>,
}

impl Default for SelectableArea {
  fn default() -> Self {
    Self {
      default_copy: true,
      entries: RefCell::new(Vec::new()),
      selection: Cell::new(None),
      selection_rects: RefCell::new(Vec::new()),
    }
  }
}

impl SelectableArea {
  pub fn register(&self, entry: Box<DefaultSelectionEntry>) {
    let track_id = entry.track_id();
    if self
      .entries
      .borrow()
      .iter()
      .any(|existing| existing.track_id() == track_id)
    {
      return;
    }
    self.entries.borrow_mut().push(entry);
  }

  pub fn unregister(&self, track_id: &TrackId) {
    self
      .entries
      .borrow_mut()
      .retain(|existing| existing.track_id() != *track_id);
  }

  pub fn registered_count(&self) -> usize { self.entries.borrow().len() }

  pub fn registered_widget_ids(&self) -> Vec<WidgetId> {
    self
      .entries
      .borrow()
      .iter()
      .filter_map(|existing| existing.widget_id())
      .collect()
  }

  pub fn selection_rects(&self) -> Vec<Rect> { self.selection_rects.borrow().clone() }

  pub fn sort_entries_after_layout(&self, ctx: &impl WidgetCtx) {
    let mut entries = self.entries.borrow_mut();
    ctx.sort_by_tree_order(ctx.widget_id(), entries.as_mut_slice(), |entry| {
      entry.widget_id().unwrap()
    });
  }

  pub fn selection_data(&self) -> Vec<TextAreaData> {
    let entries = self.entries.borrow();
    let Some((start_idx, start, end_idx, end)) = self.normalized_selection(&entries) else {
      return Vec::new();
    };

    (start_idx..=end_idx)
      .filter_map(|idx| {
        let entry = entries.get(idx)?.as_ref();
        let range = Self::range_for_entry(idx, entry, start_idx, start, end_idx, end)?;
        let data = entry.selection_data(&range)?;
        (!data.is_empty()).then_some(data)
      })
      .collect()
  }

  pub fn selected_text(&self) -> Option<String> { selection_text(self.selection_data().as_slice()) }

  pub fn hit_test(&self, point: Point, ctx: &impl WidgetCtx) -> Option<SelectableAreaHit> {
    let entries = self.entries.borrow();
    entries
      .iter()
      .find_map(|entry| self.hit_test_entry(entry.as_ref(), point, ctx))
  }

  pub fn nearest_position(&self, point: Point, ctx: &impl WidgetCtx) -> Option<SelectableAreaHit> {
    if let Some(hit) = self.hit_test(point, ctx) {
      return Some(hit);
    }

    let entries = self.entries.borrow();
    entries
      .iter()
      .filter_map(|entry| {
        let widget_id = entry.widget_id()?;
        let (local_point, local_rect) = self.local_rect(point, widget_id, ctx)?;
        let position = entry.nearest_position(local_point)?;
        let distance = squared_distance_to_rect(local_point, local_rect);
        Some((distance, SelectableAreaHit { widget_id, position }))
      })
      .min_by(|(left, _), (right, _)| left.total_cmp(right))
      .map(|(_, hit)| hit)
  }

  pub(crate) fn select_all(&self, ctx: &impl WidgetCtx) -> Vec<Rect> {
    let entries = self.entries.borrow();
    let selection = entries
      .first()
      .zip(entries.last())
      .and_then(|(first, last)| {
        Some(SelectableAreaSelection {
          anchor: SelectableAreaPosition {
            widget_id: first.widget_id()?,
            position: first.first_position()?,
          },
          focus: SelectableAreaPosition {
            widget_id: last.widget_id()?,
            position: last.last_position()?,
          },
        })
      })
      .filter(|selection| selection.anchor != selection.focus);
    self.selection.set(selection);
    drop(entries);
    let rects = self.collect_selection_rects(ctx);
    *self.selection_rects.borrow_mut() = rects.clone();
    rects
  }

  fn set_selection_from_points(
    &self, anchor: Point, focus: Point, ctx: &impl WidgetCtx,
  ) -> Vec<Rect> {
    self
      .selection
      .set(self.selection_from_points(anchor, focus, ctx));
    let rects = self.collect_selection_rects(ctx);
    *self.selection_rects.borrow_mut() = rects.clone();
    rects
  }

  fn select_unit(&self, point: Point, mode: MoveMode, ctx: &impl WidgetCtx) -> Vec<Rect> {
    let Some(hit) = self
      .hit_test(point, ctx)
      .or_else(|| self.nearest_position(point, ctx))
    else {
      self.selection.set(None);
      self.selection_rects.borrow_mut().clear();
      return Vec::new();
    };
    let entries = self.entries.borrow();
    let Some(entry) = entries
      .iter()
      .find(|existing| existing.widget_id() == Some(hit.widget_id))
      .map(|existing| existing.as_ref())
    else {
      self.selection.set(None);
      self.selection_rects.borrow_mut().clear();
      return Vec::new();
    };

    let Some(range) = entry.select_unit(&hit.position, mode) else {
      self.selection.set(None);
      self.selection_rects.borrow_mut().clear();
      return Vec::new();
    };
    self.selection.set(Some(SelectableAreaSelection {
      anchor: SelectableAreaPosition { widget_id: hit.widget_id, position: range.anchor },
      focus: SelectableAreaPosition { widget_id: hit.widget_id, position: range.focus },
    }));
    drop(entries);
    let rects = self.collect_selection_rects(ctx);
    *self.selection_rects.borrow_mut() = rects.clone();
    rects
  }

  fn collect_selection_rects(&self, ctx: &impl WidgetCtx) -> Vec<Rect> {
    let entries = self.entries.borrow();
    let Some((start_idx, start, end_idx, end)) = self.normalized_selection(&entries) else {
      return Vec::new();
    };

    (start_idx..=end_idx)
      .filter_map(|idx| {
        let entry = entries.get(idx)?.as_ref();
        let widget_id = entry.widget_id()?;
        let range = Self::range_for_entry(idx, entry, start_idx, start, end_idx, end)?;
        Some(
          entry
            .selection_rects(&range)
            .into_iter()
            .map(move |rect| Rect::new(ctx.map_from(rect.origin, widget_id), rect.size)),
        )
      })
      .flatten()
      .collect()
  }

  fn selection_from_points(
    &self, anchor: Point, focus: Point, ctx: &impl WidgetCtx,
  ) -> Option<SelectableAreaSelection> {
    let anchor = self.nearest_position(anchor, ctx)?;
    let focus = self.nearest_position(focus, ctx)?;
    Some(SelectableAreaSelection {
      anchor: SelectableAreaPosition { widget_id: anchor.widget_id, position: anchor.position },
      focus: SelectableAreaPosition { widget_id: focus.widget_id, position: focus.position },
    })
  }

  fn normalized_selection(
    &self, entries: &[Box<DefaultSelectionEntry>],
  ) -> Option<(usize, SelectableAreaPosition, usize, SelectableAreaPosition)> {
    let selection = self.selection.get()?;
    let anchor_idx = Self::entry_idx(entries, selection.anchor.widget_id)?;
    let focus_idx = Self::entry_idx(entries, selection.focus.widget_id)?;
    if anchor_idx < focus_idx
      || (anchor_idx == focus_idx
        && selection.anchor.position.cluster <= selection.focus.position.cluster)
    {
      Some((anchor_idx, selection.anchor, focus_idx, selection.focus))
    } else {
      Some((focus_idx, selection.focus, anchor_idx, selection.anchor))
    }
  }

  fn entry_idx(entries: &[Box<DefaultSelectionEntry>], widget_id: WidgetId) -> Option<usize> {
    entries
      .iter()
      .position(|existing| existing.widget_id() == Some(widget_id))
  }

  fn range_for_entry(
    idx: usize, entry: &DefaultSelectionEntry, start_idx: usize, start: SelectableAreaPosition,
    end_idx: usize, end: SelectableAreaPosition,
  ) -> Option<TextSelectableRange> {
    if start_idx == end_idx {
      return Some(entry.make_range(start.position, end.position));
    }

    if idx == start_idx {
      let last = entry.last_position()?;
      return Some(entry.make_range(start.position, last));
    }

    if idx == end_idx {
      let first = entry.first_position()?;
      return Some(entry.make_range(first, end.position));
    }

    let first = entry.first_position()?;
    let last = entry.last_position()?;
    Some(entry.make_range(first, last))
  }

  fn hit_test_entry(
    &self, entry: &DefaultSelectionEntry, point: Point, ctx: &impl WidgetCtx,
  ) -> Option<SelectableAreaHit> {
    let widget_id = entry.widget_id()?;
    let (local_point, local_rect) = self.local_rect(point, widget_id, ctx)?;
    if !local_rect.contains(local_point) {
      return None;
    }

    let position = entry.hit_test(local_point)?;
    Some(SelectableAreaHit { widget_id, position })
  }

  fn local_rect(
    &self, point: Point, widget_id: WidgetId, ctx: &impl WidgetCtx,
  ) -> Option<(Point, Rect)> {
    let local_point = ctx.map_to(point, widget_id);
    let size = ctx.widget_box_size(widget_id)?;
    Some((local_point, Rect::from_size(size)))
  }
}

fn squared_distance_to_rect(point: Point, rect: Rect) -> f32 {
  let dx = if point.x < rect.min_x() {
    rect.min_x() - point.x
  } else if point.x > rect.max_x() {
    point.x - rect.max_x()
  } else {
    0.
  };

  let dy = if point.y < rect.min_y() {
    rect.min_y() - point.y
  } else if point.y > rect.max_y() {
    point.y - rect.max_y()
  } else {
    0.
  };

  dx * dx + dy * dy
}

impl<'c> ComposeChild<'c> for SelectableArea {
  type Child = Vec<Widget<'c>>;

  fn compose_child(this: impl StateWriter<Value = Self>, child: Self::Child) -> Widget<'c> {
    fn_widget! {
      let overlay_rects = Stateful::new(Vec::<Rect>::new());
      let mut host = @FocusScope {
        skip_host: false,
        on_key_down: move |e| {
          if !e.with_command_key() {
            return;
          }

          match e.key_code() {
            PhysicalKey::Code(KeyCode::KeyC) if $read(this).default_copy => {
              let selection = $read(this).selection_data();
              if !selection.is_empty() {
                let _ = copy_selection_data_to_clipboard(selection.as_slice());
              }
            }
            PhysicalKey::Code(KeyCode::KeyA) => {
              *$write(overlay_rects) = $read(this).select_all(&**e);
            }
            _ => {}
          }
        },
      };
      let selection_coordinator = SelectionCoordinatorHandle::new(SelectableAreaCoordinator {
        state: this.clone_boxed_writer(),
      });
      @Providers {
        providers: smallvec::smallvec![
          Provider::writer(this.clone_writer(), None),
          Provider::new(selection_coordinator),
        ],
        @(host) {
          @PointerSelectRegion {
            on_pointer_down: move |_| {
              $clone(host.focus_handle()).request_focus(FocusReason::Pointer)
            },
            on_custom: move |e: &mut PointerSelectEvent| match e.data() {
              PointerSelectData::Start(point) => {
                let rects = $read(this).set_selection_from_points(*point, *point, &**e);
                *$write(overlay_rects) = rects;
              }
              PointerSelectData::Move { from, to } | PointerSelectData::End { from, to } => {
                let rects = $read(this).set_selection_from_points(*from, *to, &**e);
                *$write(overlay_rects) = rects;
              }
            },
            on_double_tap: move |e| {
              let rects = $read(this).select_unit(e.position(), MoveMode::Word, &e.common);
              *$write(overlay_rects) = rects;
            },
            on_performed_layout: move |e| {
              $read(this).sort_entries_after_layout(e);
              *$write(overlay_rects) = $read(this).collect_selection_rects(e);
            },
            @SelectionOverlay {
              class: TEXT_SELECTION,
              rects: pipe!($read(overlay_rects).clone()),
              @Column {
                @ { child }
              }
            }
          }
        }
      }
    }
    .into_widget()
  }
}

impl Compose for SelectableImage {
  fn compose(this: impl StateWriter<Value = Self>) -> Widget<'static> {
    fn_widget! {
      let image = part_writer!(&mut this.image);
      let mut root = FatObj::new(image.clone_writer());
      root.on_performed_layout(move |e| {
        $read(this)
          .size
          .set(Some(e.box_size().unwrap_or_default()))
      });
      let root = root.into_widget();
      register_image_to_parent_selection_coordinator(root, $writer(this))
    }
    .into_widget()
  }
}

#[cfg(test)]
mod tests {
  use std::{
    borrow::Cow,
    io::{Error, ErrorKind},
  };

  use ribir_core::{
    clipboard::Clipboard, prelude::*, reset_test_env, test_helper::*, window::UiEvent,
  };
  use winit::event::ElementState;

  use super::*;

  #[derive(Default)]
  struct TestClipboard {
    text: String,
    image: Option<PixelImage>,
  }

  impl Clipboard for TestClipboard {
    fn read_text(&mut self) -> Result<String, Error> { Ok(self.text.clone()) }

    fn write_text(&mut self, text: &str) -> Result<(), Error> {
      self.text = text.into();
      Ok(())
    }

    fn read_img(&mut self) -> Result<PixelImage, Error> {
      self
        .image
        .clone()
        .ok_or_else(|| Error::new(ErrorKind::Unsupported, "clipboard read_img"))
    }

    fn write_img(&mut self, image: &PixelImage) -> Result<(), Error> {
      self.image = Some(image.clone());
      Ok(())
    }

    fn read(&mut self, _: &str) -> Result<Cow<'_, [u8]>, Error> {
      Err(Error::new(ErrorKind::Unsupported, "clipboard read"))
    }

    fn write(&mut self, _: &str, _: &[u8]) -> Result<(), Error> {
      Err(Error::new(ErrorKind::Unsupported, "clipboard write"))
    }

    fn clear(&mut self) -> Result<(), Error> {
      self.text.clear();
      self.image = None;
      Ok(())
    }
  }

  fn test_image() -> Resource<PixelImage> {
    Resource::new(PixelImage::new(
      vec![0, 0, 0, 255, 255, 0, 0, 255, 0, 255, 0, 255, 0, 0, 255, 255].into(),
      2,
      2,
      ColorFormat::Rgba8,
    ))
  }

  #[test]
  fn text_selectable_registers_directly_in_selectable_area() {
    reset_test_env!();
    let area_state = Stateful::new(SelectableArea::default());
    let area = area_state.clone_writer();

    let w = fn_widget! {
      let title = @TextSelectable::<CowArc<str>> { text: "Title" };
      let body = @TextSelectable::<CowArc<str>> { text: "Body" };
      SelectableArea::compose_child(
        area.clone_writer(),
        vec![title.clone_writer().into_widget(), body.clone_writer().into_widget()],
      )
    };

    let wnd = TestWindow::new_with_size(w, Size::new(240., 160.));
    wnd.draw_frame();

    assert_eq!(area_state.read().registered_count(), 2);

    let first = area_state.read().registered_widget_ids()[0];
    wnd.dispose_widget(first);
    wnd.draw_frame();

    assert_eq!(area_state.read().registered_count(), 1);
  }

  #[test]
  fn selectable_area_resorts_entries_after_dynamic_insert() {
    reset_test_env!();
    let area_state = Stateful::new(SelectableArea::default());
    let area = area_state.clone_writer();
    let show_prefix = Stateful::new(false);
    let (selected_text, w_selected_text) = split_value(None::<String>);

    let w = fn_widget! {
      let queried = Stateful::new(false);
      let title = @TextSelectable::<CowArc<str>> { text: "Title" };
      let body = @TextSelectable::<CowArc<str>> { text: "Body" };
      let mut children = vec![title.clone_writer().into_widget(), body.clone_writer().into_widget()];
      if *$read(show_prefix) {
        let prefix = @TextSelectable::<CowArc<str>> { text: "Prefix" };
        children.insert(0, prefix.clone_writer().into_widget());
      }
      let area = SelectableArea::compose_child(area.clone_writer(), children);

      @FatObj {
        on_performed_layout: move |e| {
          if !*$read(show_prefix) {
            *$write(show_prefix) = true;
            return;
          }

          if *$read(queried) {
            return;
          }

          *$write(queried) = true;
          let _ = $read(area_state).select_all(e);
          *$write(w_selected_text) = $read(area_state).selected_text();
        },
        @ { area }
      }
    };

    let wnd = TestWindow::new_with_size(w, Size::new(240., 160.));
    wnd.draw_frame();
    wnd.draw_frame();

    assert_eq!(area_state.read().registered_count(), 3);
    assert_eq!(*selected_text.read(), Some("Prefix\nTitle\nBody".into()));
  }

  #[test]
  fn selectable_area_hit_and_nearest_query_registered_widgets() {
    reset_test_env!();
    let area_state = Stateful::new(SelectableArea::default());
    let area = area_state.clone_writer();
    let (exact_hit, w_exact_hit) = split_value(None::<WidgetId>);
    let (nearest_hit, w_nearest_hit) = split_value(None::<WidgetId>);

    let w = fn_widget! {
      let queried = Stateful::new(false);
      let title = @Input {};
      let body = @TextArea {};
      $write(title).set_text("Title");
      $write(body).set_text("Body");
      let area = SelectableArea::compose_child(
        area.clone_writer(),
        vec![title.clone_writer().into_widget(), body.clone_writer().into_widget()],
      );

      @FatObj {
        on_performed_layout: move |e| {
          if *$read(queried) {
            return;
          }
          *$write(queried) = true;
          *$write(w_exact_hit) = $read(area_state)
            .hit_test(Point::new(16., 12.), e)
            .map(|hit| hit.widget_id);
          *$write(w_nearest_hit) = $read(area_state)
            .nearest_position(Point::new(16., 64.), e)
            .map(|hit| hit.widget_id);
        },
        @ { area }
      }
    };

    let wnd = TestWindow::new_with_size(w, Size::new(240., 160.));
    wnd.draw_frame();

    let registered = area_state.read().registered_widget_ids();
    assert_eq!(registered.len(), 2);
    assert_eq!(*exact_hit.read(), Some(registered[0]));
    assert_eq!(*nearest_hit.read(), Some(registered[1]));
  }

  #[test]
  fn selectable_area_collects_selection_rects_and_copies_text() {
    reset_test_env!();
    AppCtx::set_clipboard(Box::new(TestClipboard::default()));
    let area_state = Stateful::new(SelectableArea::default());
    let area = area_state.clone_writer();
    let (rect_count, w_rect_count) = split_value(0usize);

    let w = fn_widget! {
      let queried = Stateful::new(false);
      let title = @TextSelectable::<CowArc<str>> { text: "Title" };
      let body = @TextSelectable::<CowArc<str>> { text: "Body" };
      let area = SelectableArea::compose_child(
        area.clone_writer(),
        vec![title.clone_writer().into_widget(), body.clone_writer().into_widget()],
      );

      @FatObj {
        on_performed_layout: move |e| {
          if *$read(queried) {
            return;
          }
          *$write(queried) = true;

          let (anchor_widget_id, anchor_position, focus_widget_id, focus_position) = {
            let area = $read(area_state);
            let entries = area.entries.borrow();
            let first = entries.first().unwrap().as_ref();
            let last = entries.last().unwrap().as_ref();
            (
              first.widget_id().unwrap(),
              first.first_position().unwrap(),
              last.widget_id().unwrap(),
              last.last_position().unwrap(),
            )
          };

          $read(area_state).selection.set(Some(SelectableAreaSelection {
            anchor: SelectableAreaPosition {
              widget_id: anchor_widget_id,
              position: anchor_position,
            },
            focus: SelectableAreaPosition {
              widget_id: focus_widget_id,
              position: focus_position,
            },
          }));
          *$write(w_rect_count) = $read(area_state).collect_selection_rects(e).len();
        },
        @ { area }
      }
    };

    let wnd = TestWindow::new_with_size(w, Size::new(240., 160.));
    wnd.draw_frame();

    let selection = area_state.read().selection_data();
    assert_eq!(selection_text(&selection).as_deref(), Some("Title\nBody"));
    assert!(*rect_count.read() > 0);
    assert!(selection_single_image(&selection).is_none());
  }

  #[test]
  fn selectable_area_selection_data_and_copy_support_images() {
    reset_test_env!();
    AppCtx::set_clipboard(Box::new(TestClipboard::default()));
    let area_state = Stateful::new(SelectableArea::default());
    let area = area_state.clone_writer();
    let image = test_image();

    let w = fn_widget! {
      let title = @TextSelectable::<CowArc<str>> { text: "Title" };
      let art = @SelectableImage { image: image.clone() };
      let area = SelectableArea::compose_child(
        area.clone_writer(),
        vec![title.clone_writer().into_widget(), art.clone_writer().into_widget()],
      );

      @FatObj {
        on_performed_layout: move |_| {
          let (anchor_widget_id, anchor_position, focus_widget_id, focus_position) = {
            let area = $read(area_state);
            let entries = area.entries.borrow();
            let first = entries.first().unwrap().as_ref();
            let last = entries.last().unwrap().as_ref();
            (
              first.widget_id().unwrap(),
              first.first_position().unwrap(),
              last.widget_id().unwrap(),
              last.last_position().unwrap(),
            )
          };

          $read(area_state).selection.set(Some(SelectableAreaSelection {
            anchor: SelectableAreaPosition {
              widget_id: anchor_widget_id,
              position: anchor_position,
            },
            focus: SelectableAreaPosition {
              widget_id: focus_widget_id,
              position: focus_position,
            },
          }));
        },
        @ { area }
      }
    };

    let wnd = TestWindow::new_with_size(w, Size::new(240., 160.));
    wnd.draw_frame();

    let selection = area_state.read().selection_data();
    assert_eq!(selection_text(&selection).as_deref(), Some("Title"));
    assert!(matches!(
      selection.as_slice(),
      [text, image]
        if &*text.text == "Title"
          && text.image.is_none()
          && image.text.is_empty()
          && image.image.is_some()
    ));
    assert!(selection_single_image(&selection).is_none());
  }

  #[test]
  fn selectable_area_copies_single_selected_image() {
    reset_test_env!();
    AppCtx::set_clipboard(Box::new(TestClipboard::default()));
    let area_state = Stateful::new(SelectableArea::default());
    let area = area_state.clone_writer();
    let image = test_image();
    let expected = image.pixel_bytes().to_vec();

    let w = fn_widget! {
      let art = @SelectableImage { image: image.clone() };
      let area = SelectableArea::compose_child(
        area.clone_writer(),
        vec![art.clone_writer().into_widget()],
      );

      @FatObj {
        on_performed_layout: move |_| {
          let (widget_id, anchor_position, focus_position) = {
            let area = $read(area_state);
            let entries = area.entries.borrow();
            let entry = entries.first().unwrap().as_ref();
            (
              entry.widget_id().unwrap(),
              entry.first_position().unwrap(),
              entry.last_position().unwrap(),
            )
          };

          $read(area_state).selection.set(Some(SelectableAreaSelection {
            anchor: SelectableAreaPosition { widget_id, position: anchor_position },
            focus: SelectableAreaPosition { widget_id, position: focus_position },
          }));
        },
        @ { area }
      }
    };

    let wnd = TestWindow::new_with_size(w, Size::new(80., 80.));
    wnd.draw_frame();

    let selection = area_state.read().selection_data();
    assert!(selection_text(&selection).is_none());
    assert!(selection_single_image(&selection).is_some());
    let copied = selection_single_image(&selection).unwrap();
    assert_eq!(copied.width(), 2);
    assert_eq!(copied.height(), 2);
    assert_eq!(copied.pixel_bytes(), expected.as_slice());
  }

  #[test]
  fn selectable_area_receives_keydown_after_focus() {
    reset_test_env!();
    AppCtx::set_clipboard(Box::new(TestClipboard::default()));
    let area_state = Stateful::new(SelectableArea::default());
    let area = area_state.clone_writer();

    let w = fn_widget! {
      let title = @TextSelectable::<CowArc<str>> { text: "Title" };
      let body = @TextSelectable::<CowArc<str>> { text: "Body" };
      SelectableArea::compose_child(
        area.clone_writer(),
        vec![title.clone_writer().into_widget(), body.clone_writer().into_widget()],
      )
    };

    let wnd = TestWindow::new_with_size(w, Size::new(240., 160.));
    wnd.draw_frame();

    wnd.process_cursor_move(Point::new(16., 12.));
    wnd.process_mouse_press(Box::new(DummyDeviceId), MouseButtons::PRIMARY);
    wnd.process_mouse_release(Box::new(DummyDeviceId), MouseButtons::PRIMARY);
    wnd.draw_frame();
    assert!(wnd.focusing().is_some());

    let (anchor_widget_id, anchor_position, focus_widget_id, focus_position) = {
      let area = area_state.read();
      let entries = area.entries.borrow();
      let first = entries.first().unwrap().as_ref();
      let last = entries.last().unwrap().as_ref();
      (
        first.widget_id().unwrap(),
        first.first_position().unwrap(),
        last.widget_id().unwrap(),
        last.last_position().unwrap(),
      )
    };

    area_state
      .read()
      .selection
      .set(Some(SelectableAreaSelection {
        anchor: SelectableAreaPosition { widget_id: anchor_widget_id, position: anchor_position },
        focus: SelectableAreaPosition { widget_id: focus_widget_id, position: focus_position },
      }));

    assert!(AppCtx::send_ui_event(UiEvent::ModifiersChanged {
      wnd_id: wnd.id(),
      state: ModifiersState::CONTROL,
    }));
    AppCtx::run_until_stalled();
    wnd.process_keyboard_event(
      PhysicalKey::Code(KeyCode::KeyC),
      VirtualKey::Character("c".into()),
      false,
      KeyLocation::Standard,
      ElementState::Pressed,
    );
    wnd.draw_frame();

    assert_eq!(
      AppCtx::clipboard()
        .borrow_mut()
        .read_text()
        .unwrap(),
      "Title\nBody"
    );
  }

  #[test]
  fn selectable_area_can_disable_default_copy_shortcut() {
    reset_test_env!();
    AppCtx::set_clipboard(Box::new(TestClipboard::default()));
    let area_state = Stateful::new(SelectableArea { default_copy: false, ..Default::default() });
    let area = area_state.clone_writer();

    let w = fn_widget! {
      let title = @TextSelectable::<CowArc<str>> { text: "Title" };
      SelectableArea::compose_child(area.clone_writer(), vec![title.clone_writer().into_widget()])
    };

    let wnd = TestWindow::new_with_size(w, Size::new(240., 80.));
    wnd.draw_frame();

    wnd.process_cursor_move(Point::new(16., 12.));
    wnd.process_mouse_press(Box::new(DummyDeviceId), MouseButtons::PRIMARY);
    wnd.process_mouse_release(Box::new(DummyDeviceId), MouseButtons::PRIMARY);
    wnd.draw_frame();

    let (widget_id, anchor_position, focus_position) = {
      let area = area_state.read();
      let entries = area.entries.borrow();
      let entry = entries.first().unwrap().as_ref();
      (entry.widget_id().unwrap(), entry.first_position().unwrap(), entry.last_position().unwrap())
    };

    area_state
      .read()
      .selection
      .set(Some(SelectableAreaSelection {
        anchor: SelectableAreaPosition { widget_id, position: anchor_position },
        focus: SelectableAreaPosition { widget_id, position: focus_position },
      }));

    assert!(AppCtx::send_ui_event(UiEvent::ModifiersChanged {
      wnd_id: wnd.id(),
      state: ModifiersState::CONTROL,
    }));
    AppCtx::run_until_stalled();
    wnd.process_keyboard_event(
      PhysicalKey::Code(KeyCode::KeyC),
      VirtualKey::Character("c".into()),
      false,
      KeyLocation::Standard,
      ElementState::Pressed,
    );
    wnd.draw_frame();

    assert_eq!(
      AppCtx::clipboard()
        .borrow_mut()
        .read_text()
        .unwrap(),
      ""
    );
  }
}
