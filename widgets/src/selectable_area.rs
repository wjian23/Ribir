use std::{any::Any, cell::Cell, rc::Rc};

use ribir_core::{prelude::*, wrap_render::WrapRender};

use crate::{
  input::{
    BoundaryResult, MoveMode, SelectableWithContent, TextPosition, TextSelectionRange,
    key_move_mode,
  },
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

pub trait SelectionCoordinator<T = TextPosition, D = TextAreaData>: 'static {
  fn register_selection_entry(&self, entry: Box<dyn SelectionEntry<T, D>>);
  fn unregister_selection_entry(&self, track_id: &TrackId);
  fn clear_selection(&self);
  fn selection_data(&self) -> Vec<D>;
  fn selection_for_entry(&self, track_id: &TrackId) -> Option<SelectableRange<T>>;
  fn set_selection_for_entry(&self, track_id: &TrackId, selection: Option<SelectableRange<T>>);
}

pub struct SelectionCoordinatorHandle<T = TextPosition, D = TextAreaData>(
  Rc<dyn SelectionCoordinator<T, D>>,
);

impl<T, D> Clone for SelectionCoordinatorHandle<T, D> {
  fn clone(&self) -> Self { Self(self.0.clone()) }
}

impl<T: 'static, D: 'static> SelectionCoordinatorHandle<T, D> {
  fn new(scope: impl SelectionCoordinator<T, D> + 'static) -> Self {
    Self(Rc::new(scope) as Rc<dyn SelectionCoordinator<T, D>>)
  }

  pub fn register_selection_entry(&self, entry: Box<dyn SelectionEntry<T, D>>) {
    self.0.register_selection_entry(entry);
  }

  pub fn unregister_selection_entry(&self, track_id: &TrackId) {
    self.0.unregister_selection_entry(track_id);
  }

  pub fn clear_selection(&self) { self.0.clear_selection(); }

  pub fn selection_data(&self) -> Vec<D> { self.0.selection_data() }

  pub fn selection_for_entry(&self, track_id: &TrackId) -> Option<SelectableRange<T>> {
    self.0.selection_for_entry(track_id)
  }

  pub fn set_selection_for_entry(&self, track_id: &TrackId, selection: Option<SelectableRange<T>>) {
    self
      .0
      .set_selection_for_entry(track_id, selection)
  }
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

pub trait SelectionClipboardData: 'static {
  fn copy_selection_to_clipboard(selection: &[Self]) -> bool
  where
    Self: Sized;
}

impl SelectionClipboardData for TextAreaData {
  fn copy_selection_to_clipboard(selection: &[Self]) -> bool {
    copy_selection_data_to_clipboard(selection)
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

struct SelectableAreaCoordinator<T: 'static + Eq + Ord, D: 'static> {
  state: Box<dyn StateWriter<Value = SelectableArea<T, D>>>,
}

impl<T: 'static + Clone + Eq + Ord, D: 'static> SelectionCoordinator<T, D>
  for SelectableAreaCoordinator<T, D>
{
  fn register_selection_entry(&self, entry: Box<dyn SelectionEntry<T, D>>) {
    self.state.write().register(entry);
  }

  fn unregister_selection_entry(&self, track_id: &TrackId) {
    self.state.write().unregister(track_id);
  }

  fn clear_selection(&self) { self.state.write().selection = None; }

  fn selection_data(&self) -> Vec<D> { self.state.read().selection_data() }

  fn selection_for_entry(&self, track_id: &TrackId) -> Option<SelectableRange<T>> {
    self.state.read().selection_for_track(track_id)
  }

  fn set_selection_for_entry(&self, track_id: &TrackId, selection: Option<SelectableRange<T>>) {
    self
      .state
      .write()
      .set_selection_for_track(track_id, selection);
  }
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

pub(crate) fn boxed_text_selection_entry<S>(
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

pub(crate) fn register_fatobj_to_parent_selection_coordinator<S, T>(
  child: &mut FatObj<T>, state: impl StateWriter<Value = S> + 'static,
) where
  S: SelectableWithContent + 'static,
{
  let Some(coordinator) =
    Provider::of::<SelectionCoordinatorHandle>(BuildCtx::get()).map(|area| area.clone())
  else {
    return;
  };

  let track_id = child.track_id();
  coordinator.register_selection_entry(boxed_text_selection_entry(state, track_id.clone()));
  child.on_disposing(move |_| coordinator.unregister_selection_entry(&track_id));
}

pub(crate) fn register_to_parent_selection_coordinator<'c, S>(
  child: Widget<'c>, state: impl StateWriter<Value = S> + 'static,
) -> Widget<'c>
where
  S: SelectableWithContent + 'static,
{
  let mut child = FatObj::new(child);
  register_fatobj_to_parent_selection_coordinator(&mut child, state);
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
  child.on_disposing(move |_| coordinator.unregister_selection_entry(&track_id));
  child.into_widget()
}

pub(crate) fn wrap_with_default_selection_area<'c>(child: Widget<'c>) -> Widget<'c> {
  SelectableArea::compose_child(
    Stateful::new(SelectableArea::<TextPosition, TextAreaData>::default()),
    vec![child],
  )
}

#[derive(Debug, PartialEq, Eq)]
pub struct SelectableAreaHit<T = TextPosition> {
  pub widget_id: WidgetId,
  pub position: T,
}

#[derive(Clone, Debug, PartialEq, Eq, Ord, PartialOrd)]
pub struct SelectableAreaPosition<Position = TextPosition> {
  pub idx: usize,
  pub position: Position,
}

#[derive(Clone, Debug, PartialEq, Eq, PartialOrd, Ord)]
pub struct SelectableAreaSelection<Position = TextPosition> {
  pub anchor: SelectableAreaPosition<Position>,
  pub focus: SelectableAreaPosition<Position>,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct SelectableAreaSelectionChanged<Position = TextPosition> {
  pub from: Option<SelectableAreaSelection<Position>>,
  pub to: Option<SelectableAreaSelection<Position>>,
}

pub type SelectableAreaSelectionChangedEvent<Position> =
  CustomEvent<SelectableAreaSelectionChanged<Position>>;

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct SelectableAreaEntriesChanged {
  pub from_revision: usize,
  pub to_revision: usize,
  pub entry_count: usize,
}

pub type SelectableAreaEntriesChangedEvent = CustomEvent<SelectableAreaEntriesChanged>;

#[derive(Declare)]
pub struct SelectableArea<P: 'static = TextPosition, D: 'static = TextAreaData> {
  #[declare(default = true)]
  pub default_copy: bool,
  #[declare(skip)]
  entries: Vec<Box<dyn SelectionEntry<P, D>>>,
  #[declare(skip)]
  selection: Option<SelectableAreaSelection<P>>,
  #[declare(skip)]
  entries_revision: usize,
  #[declare(skip)]
  notified_entries_revision: usize,
}

impl<P: 'static, D: 'static> Default for SelectableArea<P, D> {
  fn default() -> Self {
    Self {
      default_copy: true,
      entries: Vec::new(),
      selection: None,
      entries_revision: 0,
      notified_entries_revision: 0,
    }
  }
}

impl<P: 'static + Eq + Clone + Ord> SelectableArea<P, TextAreaData> {
  pub fn selected_text(&self) -> Option<String> { selection_text(self.selection_data().as_slice()) }
}

impl<P: 'static + Eq + Clone + Ord, D: 'static> SelectableArea<P, D> {
  pub fn register(&mut self, entry: Box<dyn SelectionEntry<P, D>>) {
    let track_id = entry.track_id();
    if self
      .entries
      .iter()
      .any(|existing| existing.track_id() == track_id)
    {
      return;
    }
    self.entries.push(entry);
    self.selection = None;
    self.entries_revision += 1;
  }

  pub fn unregister(&mut self, track_id: &TrackId) {
    let len = self.entries.len();
    self
      .entries
      .retain(|entry| entry.track_id() != *track_id);
    if self.entries.len() != len {
      self.selection = None;
      self.entries_revision += 1;
    }
  }

  pub fn registered_count(&self) -> usize { self.entries.len() }

  pub fn registered_widget_ids(&self) -> Vec<WidgetId> {
    self
      .entries
      .iter()
      .filter_map(|existing| existing.widget_id())
      .collect()
  }

  pub fn selection(&self) -> Option<&SelectableAreaSelection<P>> { self.selection.as_ref() }

  pub fn set_selection(&mut self, selection: Option<SelectableAreaSelection<P>>) {
    self.selection = selection;
  }

  pub fn caret_rect(&self, ctx: &impl WidgetCtx) -> Option<Rect> {
    let selection = self.selection.as_ref()?;
    self.caret_rect_at(&selection.focus, ctx)
  }

  pub fn caret_rect_at(
    &self, position: &SelectableAreaPosition<P>, ctx: &impl WidgetCtx,
  ) -> Option<Rect> {
    let idx = position
      .idx
      .min(self.entries.len().checked_sub(1)?);
    let entry = self.entries.get(idx)?.as_ref();
    let widget_id = entry.widget_id()?;
    let rect = entry.caret_rect(&position.position)?;
    Some(Rect::new(ctx.map_from(rect.origin, widget_id), rect.size))
  }

  pub fn selection_for_track(&self, track_id: &TrackId) -> Option<SelectableRange<P>> {
    let idx = self
      .entries
      .iter()
      .position(|entry| entry.track_id() == *track_id)?;
    let entry = self.entries.get(idx)?.as_ref();
    let (start_idx, start, end_idx, end) = self.normalized_selection(&self.entries)?;
    Self::range_for_entry(idx, entry, start_idx, &start, end_idx, &end)
  }

  pub fn set_selection_for_track(
    &mut self, track_id: &TrackId, selection: Option<SelectableRange<P>>,
  ) {
    let Some(idx) = self
      .entries
      .iter()
      .position(|entry| entry.track_id() == *track_id)
    else {
      return;
    };

    self.selection = selection.map(|selection| SelectableAreaSelection {
      anchor: SelectableAreaPosition { idx, position: selection.anchor },
      focus: SelectableAreaPosition { idx, position: selection.focus },
    });
  }

  pub fn sort_entries_after_layout(&mut self, ctx: &impl WidgetCtx) {
    let before = self
      .entries
      .iter()
      .map(|entry| entry.track_id())
      .collect::<Vec<_>>();
    ctx.sort_by_tree_order(ctx.widget_id(), self.entries.as_mut_slice(), |entry| {
      entry.widget_id().unwrap()
    });
    let after = self
      .entries
      .iter()
      .map(|entry| entry.track_id())
      .collect::<Vec<_>>();
    if before != after {
      self.entries_revision += 1;
    }
  }

  pub fn selection_data(&self) -> Vec<D> {
    let Some((start_idx, start, end_idx, end)) = self.normalized_selection(&self.entries) else {
      return Vec::new();
    };

    (start_idx..=end_idx)
      .filter_map(|idx| {
        let entry = self.entries.get(idx)?.as_ref();
        let range = Self::range_for_entry(idx, entry, start_idx, &start, end_idx, &end)?;
        entry.selection_data(&range)
      })
      .collect()
  }

  pub fn hit_test(&self, point: Point, ctx: &impl WidgetCtx) -> Option<SelectableAreaHit<P>> {
    self
      .entries
      .iter()
      .find_map(|entry| self.hit_test_entry(entry.as_ref(), point, ctx))
  }

  pub fn nearest_position<F>(
    &self, point: Point, ctx: &impl WidgetCtx, distance_to_rect: F,
  ) -> Option<SelectableAreaHit<P>>
  where
    F: Fn(Point, Rect) -> f32,
  {
    if let Some(hit) = self.hit_test(point, ctx) {
      return Some(hit);
    }

    self
      .entries
      .iter()
      .filter_map(|entry| {
        let widget_id = entry.widget_id()?;
        let (local_point, local_rect) = self.local_rect(point, widget_id, ctx)?;
        let position = entry.nearest_position(local_point)?;
        let distance = distance_to_rect(local_point, local_rect);
        Some((distance, SelectableAreaHit { widget_id, position }))
      })
      .min_by(|(left, _), (right, _)| left.total_cmp(right))
      .map(|(_, hit)| hit)
  }

  pub(crate) fn select_all(&mut self) {
    let len = self.entries.len();
    let selection = self
      .entries
      .first()
      .zip(self.entries.last())
      .and_then(|(first, last)| {
        Some(SelectableAreaSelection {
          anchor: SelectableAreaPosition { idx: 0, position: first.first_position()? },
          focus: SelectableAreaPosition { idx: len - 1, position: last.last_position()? },
        })
      })
      .filter(|selection| selection.anchor != selection.focus);
    self.selection = selection;
  }

  fn collect_selection_rects(&self, ctx: &impl WidgetCtx) -> Vec<Rect> {
    let entries = &self.entries;
    let Some((start_idx, start, end_idx, end)) = self.normalized_selection(entries) else {
      return Vec::new();
    };

    (start_idx..=end_idx)
      .filter_map(|idx| {
        let entry = entries.get(idx)?.as_ref();
        let widget_id = entry.widget_id()?;
        let range = Self::range_for_entry(idx, entry, start_idx, &start, end_idx, &end)?;
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
  ) -> Option<SelectableAreaSelection<P>> {
    let anchor = self.nearest_position(anchor, ctx, vertical_distance_to_rect)?;
    let focus = self.nearest_position(focus, ctx, vertical_distance_to_rect)?;
    let anchor_idx = Self::entry_idx(&self.entries, anchor.widget_id)?;
    let focus_idx = Self::entry_idx(&self.entries, focus.widget_id)?;
    Some(SelectableAreaSelection {
      anchor: SelectableAreaPosition { idx: anchor_idx, position: anchor.position },
      focus: SelectableAreaPosition { idx: focus_idx, position: focus.position },
    })
  }

  fn normalized_selection(
    &self, entries: &[Box<dyn SelectionEntry<P, D>>],
  ) -> Option<(usize, SelectableAreaPosition<P>, usize, SelectableAreaPosition<P>)> {
    let selection = self.selection.as_ref()?;
    let anchor_idx = selection.anchor.idx.min(entries.len() - 1);
    let focus_idx = selection.focus.idx.min(entries.len() - 1);
    let anchor =
      SelectableAreaPosition { idx: anchor_idx, position: selection.anchor.position.clone() };
    let focus =
      SelectableAreaPosition { idx: focus_idx, position: selection.focus.position.clone() };
    if anchor_idx < focus_idx || (anchor_idx == focus_idx && anchor.position <= focus.position) {
      Some((anchor_idx, anchor, focus_idx, focus))
    } else {
      Some((focus_idx, focus, anchor_idx, anchor))
    }
  }

  fn entry_idx(entries: &[Box<dyn SelectionEntry<P, D>>], widget_id: WidgetId) -> Option<usize> {
    entries
      .iter()
      .position(|existing| existing.widget_id() == Some(widget_id))
  }

  fn range_for_entry(
    idx: usize, entry: &dyn SelectionEntry<P, D>, start_idx: usize,
    start: &SelectableAreaPosition<P>, end_idx: usize, end: &SelectableAreaPosition<P>,
  ) -> Option<SelectableRange<P>> {
    if start_idx == end_idx {
      return Some(entry.make_range(start.position.clone(), end.position.clone()));
    }

    if idx == start_idx {
      let last = entry.last_position()?;
      return Some(entry.make_range(start.position.clone(), last));
    }

    if idx == end_idx {
      let first = entry.first_position()?;
      return Some(entry.make_range(first, end.position.clone()));
    }

    let first = entry.first_position()?;
    let last = entry.last_position()?;
    Some(entry.make_range(first, last))
  }

  fn hit_test_entry(
    &self, entry: &dyn SelectionEntry<P, D>, point: Point, ctx: &impl WidgetCtx,
  ) -> Option<SelectableAreaHit<P>> {
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

  fn set_selection_with_event<E>(&mut self, selection: Option<SelectableAreaSelection<P>>, e: &E)
  where
    E: std::ops::Deref<Target = CommonEvent>,
  {
    let from = self.selection.clone();
    if from == selection {
      self.selection = selection;
      return;
    }

    self.selection = selection.clone();
    e.window().bubble_custom_event(
      e.current_target(),
      SelectableAreaSelectionChanged { from, to: selection },
    );
  }

  fn take_entries_changed(&mut self) -> Option<SelectableAreaEntriesChanged> {
    let from_revision = self.notified_entries_revision;
    let to_revision = self.entries_revision;
    if from_revision == to_revision {
      return None;
    }

    self.notified_entries_revision = to_revision;
    Some(SelectableAreaEntriesChanged {
      from_revision,
      to_revision,
      entry_count: self.entries.len(),
    })
  }

  fn sync_selection_from_points<E>(
    &mut self, anchor: Point, focus: Point, e: &E, ctx: &impl WidgetCtx,
  ) where
    E: std::ops::Deref<Target = CommonEvent>,
  {
    let selection = self.selection_from_points(anchor, focus, ctx);
    self.set_selection_with_event(selection, e);
  }

  fn sync_select_unit<E>(&mut self, point: Point, mode: MoveMode, e: &E, ctx: &impl WidgetCtx)
  where
    E: std::ops::Deref<Target = CommonEvent>,
  {
    let selection = self.selection_for_unit(point, mode, ctx);
    self.set_selection_with_event(selection, e);
  }

  fn selection_for_unit(
    &self, point: Point, mode: MoveMode, ctx: &impl WidgetCtx,
  ) -> Option<SelectableAreaSelection<P>> {
    let hit = self
      .hit_test(point, ctx)
      .or_else(|| self.nearest_position(point, ctx, vertical_distance_to_rect))?;
    let entry = self
      .entries
      .iter()
      .find(|existing| existing.widget_id() == Some(hit.widget_id))
      .map(|existing| existing.as_ref())?;
    let range = entry.select_unit(&hit.position, mode)?;
    let idx = Self::entry_idx(&self.entries, hit.widget_id)?;
    Some(SelectableAreaSelection {
      anchor: SelectableAreaPosition { idx, position: range.anchor },
      focus: SelectableAreaPosition { idx, position: range.focus },
    })
  }

  fn previous_focus_position(&self, from_idx: usize) -> Option<SelectableAreaPosition<P>> {
    (0..from_idx).rev().find_map(|idx| {
      self
        .entries
        .get(idx)?
        .last_position()
        .map(|position| SelectableAreaPosition { idx, position })
    })
  }

  fn next_focus_position(&self, from_idx: usize) -> Option<SelectableAreaPosition<P>> {
    ((from_idx + 1)..self.entries.len()).find_map(|idx| {
      self
        .entries
        .get(idx)?
        .first_position()
        .map(|position| SelectableAreaPosition { idx, position })
    })
  }

  fn entry_start_position(&self, idx: usize) -> Option<SelectableAreaPosition<P>> {
    self
      .entries
      .get(idx)?
      .first_position()
      .map(|position| SelectableAreaPosition { idx, position })
  }

  fn entry_end_position(&self, idx: usize) -> Option<SelectableAreaPosition<P>> {
    self
      .entries
      .get(idx)?
      .last_position()
      .map(|position| SelectableAreaPosition { idx, position })
  }

  fn resolve_backward_position(
    &self, focus: &SelectableAreaPosition<P>, moved: BoundaryResult<P>,
  ) -> Option<SelectableAreaPosition<P>> {
    match moved {
      BoundaryResult::InBlock(position) => {
        Some(SelectableAreaPosition { idx: focus.idx, position })
      }
      BoundaryResult::AtStart | BoundaryResult::AtTop => self
        .previous_focus_position(focus.idx)
        .or_else(|| self.entry_start_position(focus.idx))
        .or_else(|| Some(focus.clone())),
      BoundaryResult::AtEnd | BoundaryResult::AtBottom => self
        .entry_end_position(focus.idx)
        .or_else(|| Some(focus.clone())),
      BoundaryResult::Unavailable => None,
    }
  }

  fn resolve_forward_position(
    &self, focus: &SelectableAreaPosition<P>, moved: BoundaryResult<P>,
  ) -> Option<SelectableAreaPosition<P>> {
    match moved {
      BoundaryResult::InBlock(position) => {
        Some(SelectableAreaPosition { idx: focus.idx, position })
      }
      BoundaryResult::AtEnd | BoundaryResult::AtBottom => self
        .next_focus_position(focus.idx)
        .or_else(|| self.entry_end_position(focus.idx))
        .or_else(|| Some(focus.clone())),
      BoundaryResult::AtStart | BoundaryResult::AtTop => self
        .entry_start_position(focus.idx)
        .or_else(|| Some(focus.clone())),
      BoundaryResult::Unavailable => None,
    }
  }

  fn selection_action(
    &self, event: &KeyboardEvent,
  ) -> Result<Option<SelectableAreaSelection<P>>, ()> {
    let Some(selection) = self.selection.as_ref() else {
      return Err(());
    };
    let Some(entry) = self
      .entries
      .get(selection.focus.idx)
      .map(|entry| entry.as_ref())
    else {
      return Ok(None);
    };

    let focus = match event.key() {
      VirtualKey::Named(NamedKey::ArrowLeft) => self.resolve_backward_position(
        &selection.focus,
        entry.move_left(&selection.focus.position, key_move_mode(event)),
      ),
      VirtualKey::Named(NamedKey::ArrowRight) => self.resolve_forward_position(
        &selection.focus,
        entry.move_right(&selection.focus.position, key_move_mode(event)),
      ),
      VirtualKey::Named(NamedKey::ArrowUp) => {
        self.resolve_backward_position(&selection.focus, entry.move_up(&selection.focus.position))
      }
      VirtualKey::Named(NamedKey::ArrowDown) => {
        self.resolve_forward_position(&selection.focus, entry.move_down(&selection.focus.position))
      }
      VirtualKey::Named(NamedKey::Home) => self.resolve_backward_position(
        &selection.focus,
        entry.move_left(&selection.focus.position, MoveMode::LineBoundary),
      ),
      VirtualKey::Named(NamedKey::End) => self.resolve_forward_position(
        &selection.focus,
        entry.move_right(&selection.focus.position, MoveMode::LineBoundary),
      ),
      _ => return Err(()),
    };

    let Some(focus) = focus else {
      return Ok(None);
    };
    let anchor = if event.with_shift_key() { selection.anchor.clone() } else { focus.clone() };
    Ok(Some(SelectableAreaSelection { anchor, focus }))
  }

  fn keyboard_selection_handle<E>(&mut self, event: &KeyboardEvent, e: &E) -> bool
  where
    E: std::ops::Deref<Target = CommonEvent>,
    D: SelectionClipboardData,
  {
    if event.with_command_key() {
      match event.key_code() {
        PhysicalKey::Code(KeyCode::KeyC) if self.default_copy => {
          let selection = self.selection_data();
          let _ = (!selection.is_empty()) && D::copy_selection_to_clipboard(selection.as_slice());
          return true;
        }
        PhysicalKey::Code(KeyCode::KeyA) => {
          let selection = {
            self.select_all();
            self.selection.clone()
          };
          self.set_selection_with_event(selection, e);
          return true;
        }
        _ => {}
      }
    }

    match self.selection_action(event) {
      Ok(selection) => {
        self.set_selection_with_event(selection, e);
        true
      }
      Err(()) => false,
    }
  }
}

pub fn horizontal_distance_to_rect(point: Point, rect: Rect) -> f32 {
  if point.x < rect.min_x() {
    rect.min_x() - point.x
  } else if point.x > rect.max_x() {
    point.x - rect.max_x()
  } else {
    0.
  }
}

pub fn vertical_distance_to_rect(point: Point, rect: Rect) -> f32 {
  if point.y < rect.min_y() {
    rect.min_y() - point.y
  } else if point.y > rect.max_y() {
    point.y - rect.max_y()
  } else {
    0.
  }
}

pub fn auto_distance_to_rect(point: Point, rect: Rect) -> f32 {
  let dx = horizontal_distance_to_rect(point, rect);
  let dy = vertical_distance_to_rect(point, rect);
  dx * dx + dy * dy
}

impl<'c, T: 'static + Clone + Eq + Ord, D: 'static + SelectionClipboardData> ComposeChild<'c>
  for SelectableArea<T, D>
{
  type Child = Vec<Widget<'c>>;

  fn compose_child(this: impl StateWriter<Value = Self>, child: Self::Child) -> Widget<'c> {
    fn_widget! {
      let selection_coordinator = SelectionCoordinatorHandle::new(SelectableAreaCoordinator {
        state: this.clone_boxed_writer(),
      });
      @Providers {
        providers: smallvec::smallvec![
          Provider::writer(this.clone_writer(), None),
          Provider::new(selection_coordinator),
        ],
        // @(host) {
          @PointerSelectRegion {
            sensitivity: 1.0,
            on_custom: move |e: &mut PointerSelectEvent| {
              match e.data() {
                PointerSelectData::Start(point) => {
                  let point = e.map_to_local(*point);
                  println!("start: {:?}", point);
                  $write(this).sync_selection_from_points(point, point, e, &**e);
                }
                PointerSelectData::Move { from, to } | PointerSelectData::End { from, to } => {
                  let from = e.map_to_local(*from);
                  let to = e.map_to_local(*to);
                  $write(this).sync_selection_from_points(from, to, e, &**e);
                }
              }
              e.stop_propagation();
            },
            on_tap: move |e| {
              println!("tap: {:?}", e.position());
              $write(this).sync_selection_from_points(e.position(), e.position(), e, &**e);
            },
            on_double_tap: move |e| {
              $write(this).sync_select_unit(e.position(), MoveMode::Word, e, &e.common);
            },
            on_performed_layout: move |e| {
              let mut this = $write(this);
              this.sort_entries_after_layout(e);
              let changed = this.take_entries_changed();
              drop(this);
              if let Some(changed) = changed {
                let wnd = e.window();
                let target = e.current_target();
                wnd.clone().once_layout_ready(move || wnd.bubble_custom_event(target, changed));
              }
            },
              @Stack {
                on_key_down: move |e| {
                  let _ = $write(this).keyboard_selection_handle(e, e);
                },

                @Providers {
                  providers: {
                    let provider = Provider::new(SelectionRectProvider::new(move |ctx| {
                      $read(this).collect_selection_rects(ctx)
                    }));
                    let this = $writer(this);
                    smallvec::smallvec![
                      Provider::writer(this, Some(DirtyPhase::LayoutSubtree)),
                      provider,
                      ]
                    },
                  @InParentLayout {
                    @SelectionHighlight { class: TEXT_SELECTION }
                  }
                }

                @Column { @ { child } }
            }
          }
        // }
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
    let area_state = Stateful::new(SelectableArea::<TextPosition, TextAreaData>::default());
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
    let area_state = Stateful::new(SelectableArea::<TextPosition, TextAreaData>::default());
    let area = area_state.clone_writer();
    let show_prefix = Stateful::new(false);

    let w = fn_widget! {
      let title = @TextSelectable::<CowArc<str>> { text: "Title" };
      let body = @TextSelectable::<CowArc<str>> { text: "Body" };
      let mut children = vec![title.into_widget(), body.into_widget()];
      if *$read(show_prefix) {
        let prefix = @TextSelectable::<CowArc<str>> { text: "Prefix" };
        children.insert(0, prefix.into_widget());
      }
      let area = SelectableArea::compose_child(area.clone_writer(), children);

      @FatObj {
        on_performed_layout: move |_| {
          if !*$read(show_prefix) {
            *$write(show_prefix) = true;
          }
        },
        @ { area }
      }
    };

    let wnd = TestWindow::new_with_size(w, Size::new(240., 160.));
    wnd.draw_frame();
    wnd.draw_frame();
    wnd.draw_frame();

    wnd.process_cursor_move(Point::new(16., 12.));
    wnd.process_mouse_press(Box::new(DummyDeviceId), MouseButtons::PRIMARY);
    wnd.process_mouse_release(Box::new(DummyDeviceId), MouseButtons::PRIMARY);
    wnd.draw_frame();

    assert!(AppCtx::send_ui_event(UiEvent::ModifiersChanged {
      wnd_id: wnd.id(),
      state: ModifiersState::CONTROL,
    }));
    AppCtx::run_until_stalled();
    wnd.process_keyboard_event(
      PhysicalKey::Code(KeyCode::KeyA),
      VirtualKey::Character("a".into()),
      false,
      KeyLocation::Standard,
      ElementState::Pressed,
    );
    wnd.draw_frame();

    let selected_text = area_state.read().selected_text().unwrap();
    assert!(selected_text.ends_with("Title\nBody"));
  }

  #[test]
  fn selectable_area_hit_and_nearest_query_registered_widgets() {
    reset_test_env!();
    let area_state = Stateful::new(SelectableArea::<TextPosition, TextAreaData>::default());
    let area = area_state.clone_writer();
    let (exact_hit, w_exact_hit) = split_value(None::<WidgetId>);
    let (nearest_hit, w_nearest_hit) = split_value(None::<WidgetId>);

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
          if $read(area_state).registered_widget_ids().len() < 2 {
            return;
          }
          *$write(queried) = true;
          *$write(w_exact_hit) = $read(area_state)
            .hit_test(Point::new(16., 12.), e)
            .map(|hit| hit.widget_id);
          *$write(w_nearest_hit) = $read(area_state)
            .nearest_position(Point::new(16., 64.), e, vertical_distance_to_rect)
            .map(|hit| hit.widget_id);
        },
        @ { area }
      }
    };

    let wnd = TestWindow::new_with_size(w, Size::new(240., 160.));
    for _ in 0..10 {
      wnd.draw_frame();
      if area_state.read().registered_widget_ids().len() >= 2 {
        break;
      }
      AppCtx::run_until_stalled();
    }

    let registered = area_state.read().registered_widget_ids();
    assert!(registered.len() >= 2);
    let exact_hit = exact_hit.read().as_ref().copied().unwrap();
    let nearest_hit = nearest_hit.read().as_ref().copied().unwrap();
    assert!(registered.contains(&exact_hit));
    assert!(registered.contains(&nearest_hit));
    assert_ne!(exact_hit, nearest_hit);
  }

  #[test]
  fn selectable_area_nearest_position_supports_x_auto_and_custom_distance() {
    reset_test_env!();
    let area_state = Stateful::new(SelectableArea::<TextPosition, TextAreaData>::default());
    let area = area_state.clone_writer();
    let (nearest_hits, w_nearest_hits) = split_value(Vec::<WidgetId>::new());

    let w = fn_widget! {
      let queried = Stateful::new(false);
      let left = @TextSelectable::<CowArc<str>> { text: "Left" };
      let right = @TextSelectable::<CowArc<str>> { text: "Right" };
      let row = @Row {
        @ { left.clone_writer() }
        @ { right.clone_writer() }
      };
      let area = SelectableArea::compose_child(area.clone_writer(), vec![row.into_widget()]);

      @FatObj {
        on_performed_layout: move |e| {
          if *$read(queried) {
            return;
          }

          let query_point = Point::new(220., 64.);
          let area = $read(area_state);
          let registered = area.registered_widget_ids();
          if registered.len() < 2 {
            return;
          }
          *$write(queried) = true;

          *$write(w_nearest_hits) = vec![
            area
              .nearest_position(query_point, e, vertical_distance_to_rect)
              .map(|hit| hit.widget_id)
              .unwrap(),
            area
              .nearest_position(query_point, e, horizontal_distance_to_rect)
              .map(|hit| hit.widget_id)
              .unwrap(),
            area
              .nearest_position(query_point, e, auto_distance_to_rect)
              .map(|hit| hit.widget_id)
              .unwrap(),
            area
              .nearest_position(query_point, e, |point, rect| {
                -horizontal_distance_to_rect(point, rect)
              })
              .map(|hit| hit.widget_id)
              .unwrap(),
          ];
        },
        @ { area }
      }
    };

    let wnd = TestWindow::new_with_size(w, Size::new(240., 80.));
    for _ in 0..10 {
      wnd.draw_frame();
      if area_state.read().registered_widget_ids().len() >= 2 && nearest_hits.read().len() == 4 {
        break;
      }
      AppCtx::run_until_stalled();
    }

    let registered = area_state.read().registered_widget_ids();
    assert_eq!(registered.len(), 2);

    let nearest_hits = nearest_hits.read();
    assert_eq!(nearest_hits.len(), 4);
    assert_eq!(nearest_hits[0], registered[0]);
    assert_eq!(nearest_hits[1], registered[1]);
    assert_eq!(nearest_hits[2], registered[1]);
    assert_eq!(nearest_hits[3], registered[0]);
  }

  #[test]
  fn selectable_area_collects_selection_rects_and_copies_text() {
    reset_test_env!();
    AppCtx::set_clipboard(Box::new(TestClipboard::default()));
    let area_state = Stateful::new(SelectableArea::<TextPosition, TextAreaData>::default());
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

          let (anchor_position, focus_position) = {
            let area = $read(area_state);
            let first = area.entries.first().unwrap().as_ref();
            let last = area.entries.last().unwrap().as_ref();
            (
              first.first_position().unwrap(),
              last.last_position().unwrap(),
            )
          };

          $write(area_state).set_selection(Some(SelectableAreaSelection {
            anchor: SelectableAreaPosition {
              idx: 0,
              position: anchor_position,
            },
            focus: SelectableAreaPosition {
              idx: 1,
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
    let area_state = Stateful::new(SelectableArea::<TextPosition, TextAreaData>::default());
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
          let (anchor_position, focus_position) = {
            let area = $read(area_state);
            let first = area.entries.first().unwrap().as_ref();
            let last = area.entries.last().unwrap().as_ref();
            (
              first.first_position().unwrap(),
              last.last_position().unwrap(),
            )
          };

          $write(area_state).set_selection(Some(SelectableAreaSelection {
            anchor: SelectableAreaPosition {
              idx: 0,
              position: anchor_position,
            },
            focus: SelectableAreaPosition {
              idx: 1,
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
    let area_state = Stateful::new(SelectableArea::<TextPosition, TextAreaData>::default());
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
          let (anchor_position, focus_position) = {
            let area = $read(area_state);
            let entry = area.entries.first().unwrap().as_ref();
            (
              entry.first_position().unwrap(),
              entry.last_position().unwrap(),
            )
          };

          $write(area_state).set_selection(Some(SelectableAreaSelection {
            anchor: SelectableAreaPosition { idx: 0, position: anchor_position },
            focus: SelectableAreaPosition { idx: 0, position: focus_position },
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
    let area_state = Stateful::new(SelectableArea::<TextPosition, TextAreaData>::default());
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

    let (anchor_position, focus_position) = {
      let area = area_state.read();
      let first = area.entries.first().unwrap().as_ref();
      let last = area.entries.last().unwrap().as_ref();
      (first.first_position().unwrap(), last.last_position().unwrap())
    };

    area_state
      .write()
      .set_selection(Some(SelectableAreaSelection {
        anchor: SelectableAreaPosition { idx: 0, position: anchor_position },
        focus: SelectableAreaPosition { idx: 1, position: focus_position },
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
  fn selectable_area_emits_selection_change_and_moves_with_keyboard() {
    reset_test_env!();
    let area_state = Stateful::new(SelectableArea::<TextPosition, TextAreaData>::default());
    let area = area_state.clone_writer();
    let changes = Stateful::new(Vec::<SelectableAreaSelectionChanged<TextPosition>>::new());

    let w = fn_widget! {
      let title = @TextSelectable::<CowArc<str>> { text: "Title" };
      let mut area = FatObj::new(SelectableArea::compose_child(
        area.clone_writer(),
        vec![title.clone_writer().into_widget()],
      ));
      area.on_custom(move |e: &mut SelectableAreaSelectionChangedEvent<TextPosition>| {
        $write(changes).push(e.data().clone());
      });
      @ area.into_widget()
    };

    let wnd = TestWindow::new_with_size(w, Size::new(240., 80.));
    wnd.draw_frame();

    wnd.process_cursor_move(Point::new(16., 12.));
    wnd.process_mouse_press(Box::new(DummyDeviceId), MouseButtons::PRIMARY);
    wnd.process_mouse_release(Box::new(DummyDeviceId), MouseButtons::PRIMARY);
    wnd.draw_frame();

    let start = {
      let area = area_state.read();
      area
        .entries
        .first()
        .unwrap()
        .as_ref()
        .first_position()
        .unwrap()
    };
    area_state
      .write()
      .set_selection(Some(SelectableAreaSelection {
        anchor: SelectableAreaPosition { idx: 0, position: start },
        focus: SelectableAreaPosition { idx: 0, position: start },
      }));

    wnd.process_keyboard_event(
      PhysicalKey::Code(KeyCode::ArrowRight),
      VirtualKey::Named(NamedKey::ArrowRight),
      false,
      KeyLocation::Standard,
      ElementState::Pressed,
    );
    wnd.draw_frame();

    let selection = area_state.read().selection().cloned().unwrap();
    assert_eq!(selection.anchor, selection.focus);
    assert!(selection.focus.position.cluster > start.cluster);
    assert!(changes.read().iter().any(|change| {
      change
        .to
        .as_ref()
        .is_some_and(|selection| selection.focus.position.cluster > start.cluster)
    }));
  }

  #[test]
  fn selectable_area_can_disable_default_copy_shortcut() {
    reset_test_env!();
    AppCtx::set_clipboard(Box::new(TestClipboard::default()));
    let area_state = Stateful::new(SelectableArea::<TextPosition, TextAreaData> {
      default_copy: false,
      ..Default::default()
    });
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

    let (anchor_position, focus_position) = {
      let area = area_state.read();
      let entry = area.entries.first().unwrap().as_ref();
      (entry.first_position().unwrap(), entry.last_position().unwrap())
    };

    area_state
      .write()
      .set_selection(Some(SelectableAreaSelection {
        anchor: SelectableAreaPosition { idx: 0, position: anchor_position },
        focus: SelectableAreaPosition { idx: 0, position: focus_position },
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

  #[test]
  fn selectable_area_emits_entry_change_after_register_and_unregister() {
    reset_test_env!();
    let area_state = Stateful::new(SelectableArea::<TextPosition, TextAreaData>::default());
    let area = area_state.clone_writer();
    let changes = Stateful::new(Vec::<SelectableAreaEntriesChanged>::new());

    let w = fn_widget! {
      let title = @TextSelectable::<CowArc<str>> { text: "Title" };
      let body = @TextSelectable::<CowArc<str>> { text: "Body" };
      let mut area = FatObj::new(SelectableArea::compose_child(
        area.clone_writer(),
        vec![title.into_widget(), body.into_widget()],
      ));
      area.on_custom(move |e: &mut SelectableAreaEntriesChangedEvent| {
        $write(changes).push(e.data().clone());
      });
      @ area.into_widget()
    };

    let wnd = TestWindow::new_with_size(w, Size::new(240., 160.));
    wnd.draw_frame();
    wnd.draw_frame();

    let first = area_state.read().registered_widget_ids()[0];
    wnd.dispose_widget(first);
    wnd.draw_frame();
    wnd.draw_frame();

    let changes = changes.read();
    assert!(
      changes
        .iter()
        .any(|change| change.entry_count == 2),
      "expected initial entry registration event, got {changes:?}"
    );
    assert!(
      changes
        .iter()
        .any(|change| change.entry_count == 1),
      "expected unregister entry event, got {changes:?}"
    );
    assert!(
      changes
        .windows(2)
        .all(|window| window[0].to_revision <= window[1].to_revision)
    );
  }
}
