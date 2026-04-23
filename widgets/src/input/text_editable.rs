use std::ops::Range;

use ribir_core::prelude::{anchor::Anchor, *};

use super::{
  edit_text::EditText,
  text_selectable::{
    BoundaryResult, MoveMode, Selectable as LocalSelectable, SelectableWithContent, TEXT_SELECTION,
    TextPosition, TextSelectable, TextSelection, TextSelectionRange, key_move_mode,
  },
};
use crate::{
  prelude::*,
  selectable_area::{
    SelectionCoordinatorHandle, copy_selection_data_to_clipboard,
    register_to_parent_selection_coordinator,
  },
};

class_names! {
  #[doc = "Class name for the text caret"]
  TEXT_CARET,
}

#[derive(Declare, Default)]
pub struct BasicEditor<T: 'static> {
  host: TextSelectable<T>,
  #[declare(skip)]
  selection: TextSelection<T>,
  pre_edit: Option<PreEditState>,
}

impl<T: Default + VisualText + EditText + Clone + 'static> Compose for BasicEditor<T> {
  fn compose(this: impl StateWriter<Value = Self>) -> Widget<'static> {
    let selectable = this.clone_writer();
    let hosted_in_area = Provider::of::<SelectionCoordinatorHandle>(BuildCtx::get()).is_some();
    let root = fn_widget! {
      let text_state = part_writer!(&mut this.host.text);
      let selection_state = part_writer!(&mut this.selection);
      let mut text = FatObj::new(text_state.clone_writer());

      let caret = pipe!(*$read(text.is_focused()))
        .transform(|p| p.distinct_until_changed())
        .map(move |v| v.then(|| fn_widget!{ Self::caret_widget($writer(this)) }));

      let mut caret = FatObj::new(caret);
        @Stack {
          fit: StackFit::Passthrough,
          @Providers {
            providers: [Provider::writer(text_state.clone_writer(), Some(DirtyPhase::Layout))],
            @PointerSelectRegion {
              on_custom: move |e: &mut PointerSelectEvent| {
                let before = $read(this).snapshot();
                if let PointerSelectData::Move { from, to } = e.data() {
                  let new_sel = $read(this).selection_from_points(*from, *to);
                  if let Some(new_sel) = new_sel  {
                    $write(selection_state).range = new_sel;
                  }
                }
                $read(this).bubble_change_events(before, e);
              },
              on_pointer_down: move |e| {
                let before = $read(this).snapshot();
                let pos = $read(this).position_from_point(e.position());
                if let Some(pos) = pos {
                  let mut selection = $write(selection_state);
                  if e.with_shift_key() {
                    selection.focus = pos;
                } else {
                    selection.range = TextSelectionRange::splat(pos);
                  }
                }
                $read(this).bubble_change_events(before, e);
              },
              on_tap: move |e| {
                let before = $read(this).snapshot();
                let new_sel = $read(this).selection_from_points(e.position(), e.position());
                if let Some(new_sel) = new_sel {
                  $write(selection_state).range = new_sel;
                }
                $read(this).bubble_change_events(before, e);
              },
              on_double_tap: move |e| {
                let before = $read(this).snapshot();
                let new_sel = {
                  let this = $read(this);
                  this
                    .position_from_point(e.position())
                  .and_then(|pos| this.host.select_unit(&pos, MoveMode::Word))
                };
                if let Some(new_sel) = new_sel {
                  $write(selection_state).range = new_sel;
                }
                $read(this).bubble_change_events(before, e);
              },
              @SelectionOverlay {
                class: TEXT_SELECTION,
                rects: pipe!(if hosted_in_area {
                  Vec::new()
                } else {
                  $read(this).selection_rects()
                }),
                @(text) {
                  margin: pipe!(EdgeInsets::only_right(*$read(caret.layout_width()))),
                  on_focus_in: move |e| { e.window().set_ime_allowed(true); },
                  on_focus_out: move |e| { e.window().set_ime_allowed(false); },
                  on_chars: move |e| {
                    let before = $read(this).snapshot();
                    let mut this = $write(this);
                    if this.chars_handle(e) {
                      this.bubble_change_events(before, e);
                    } else {
                      this.forget_modifies();
                    }
                  },
                  on_key_down: move |e| {
                    let before = $read(this).snapshot();
                    let mut this = $write(this);
                    if this.selection_keys_handle(e) || this.keys_handle(e) {
                      this.bubble_change_events(before, e);
                    } else {
                      this.forget_modifies();
                    }
                  },
                  on_ime_pre_edit: move |e| {
                    let before = $read(this).snapshot();
                    let mut this = $write(this);
                    this.process_pre_edit(e);
                    this.bubble_change_events(before, e);
                  },
                }
              }
            }
        }
        @InParentLayout { @ { caret } }
      }
    }
    .into_widget();
    register_to_parent_selection_coordinator(root, selectable)
  }
}

impl<T> BasicEditor<T> {
  pub fn cluster_rg(&self) -> Range<usize> { self.selection.cluster_rg() }

  pub fn selection_range(&self) -> &TextSelectionRange { &self.selection.range }

  pub fn selection_range_mut(&mut self) -> &mut TextSelectionRange { &mut self.selection.range }
}

impl<T: EditText + 'static> BasicEditor<T> {
  fn selection_rects(&self) -> Vec<Rect> { self.host.selection_rects(&self.selection.range) }

  fn caret_widget(this: impl StateWriter<Value = Self>) -> Widget<'static> {
    let default_text_style = TextStyle::default();
    let fallback_caret_height = Provider::of::<TextStyle>(BuildCtx::get())
      .map(|style| {
        style
          .line_height
          .resolve_for_font_size(style.font_size)
      })
      .unwrap_or_else(|| {
        default_text_style
          .line_height
          .resolve_for_font_size(default_text_style.font_size)
      });
    fn_widget! {
      @Providers {
        providers: [Provider::writer(this, Some(DirtyPhase::Layout))],
        @CustomAnchor {
          data: (),
          anchor: |_: &(), _child_size: Size, _clamp: BoxClamp, ctx: &mut PlaceCtx| {
            let id = ctx.widget_id();
            let editor = Provider::reader_of::<BasicEditor<T>>(ctx).unwrap();

            let caret_rect = editor.read().caret_box();
            let in_pre_edit = editor.read().is_in_pre_edit();
            let wnd = ctx.window();
            let scroll = Provider::writer_of::<ScrollableWidget>(ctx);
            wnd.clone().once_layout_ready(move || {
              let caret_size = wnd.widget_size(id);
              if caret_size.is_none() {
                return;
              }
              let caret_size = caret_size.unwrap();
              if !in_pre_edit
                && let Some(scrollable) = scroll
              {
                let lt = scrollable
                  .write()
                  .map_to_content(Point::zero(), id, &wnd)
                  .unwrap();
                scrollable
                  .write()
                  .visible_content_box(Rect::new(lt, caret_size), Anchor::default());
              }
              let pos = wnd.map_to_global(Point::zero(), id);
              wnd.set_ime_cursor_area(&Rect::new(pos, caret_size));
            });
            Anchor::left_top(caret_rect.origin.x, caret_rect.origin.y)
          },
          @Void {
            class: TEXT_CARET,
            hint_height: pipe!({
              let height = $read(this).caret_box().height();
              if height > 0. { height } else { fallback_caret_height }
            }),
          }
        }
      }
    }
    .into_widget()
  }

  fn caret_box(&self) -> Rect {
    self
      .host
      .caret_rect(&self.selection.focus)
      .unwrap_or_default()
  }

  fn position_from_point(&self, point: Point) -> Option<TextPosition> {
    self
      .host
      .hit_test(point)
      .or_else(|| self.host.nearest_position(point))
  }

  fn selection_from_points(&self, anchor: Point, focus: Point) -> Option<TextSelectionRange> {
    let anchor = self.position_from_point(anchor)?;
    let focus = self.position_from_point(focus)?;
    Some(self.host.make_range(anchor, focus))
  }

  fn resolve_boundary_position(
    &self, current: TextPosition, moved: BoundaryResult<TextPosition>,
  ) -> Option<TextPosition> {
    match moved {
      BoundaryResult::InBlock(pos) => Some(pos),
      BoundaryResult::AtStart => self.host.first_position().or(Some(current)),
      BoundaryResult::AtEnd => self.host.last_position().or(Some(current)),
      BoundaryResult::AtTop | BoundaryResult::AtBottom => Some(current),
      BoundaryResult::Unavailable => None,
    }
  }

  fn selection_keys_handle(&mut self, event: &KeyboardEvent) -> bool {
    match self.selection_action(event) {
      Ok(Some(selection)) => {
        self.selection.range = selection;
        true
      }
      Ok(None) => true,
      Err(()) => false,
    }
  }

  fn selection_action(&self, event: &KeyboardEvent) -> Result<Option<TextSelectionRange>, ()> {
    if let Ok(selection) = self.selection_command(event) {
      return Ok(selection);
    }

    let current = self.selection.focus;
    let next = match event.key() {
      VirtualKey::Named(NamedKey::ArrowLeft) => self.resolve_boundary_position(
        current,
        self
          .host
          .move_left(&current, key_move_mode(event)),
      ),
      VirtualKey::Named(NamedKey::ArrowRight) => self.resolve_boundary_position(
        current,
        self
          .host
          .move_right(&current, key_move_mode(event)),
      ),
      VirtualKey::Named(NamedKey::ArrowUp) => {
        self.resolve_boundary_position(current, self.host.move_up(&current))
      }
      VirtualKey::Named(NamedKey::ArrowDown) => {
        self.resolve_boundary_position(current, self.host.move_down(&current))
      }
      VirtualKey::Named(NamedKey::Home) => self.resolve_boundary_position(
        current,
        self
          .host
          .move_left(&current, MoveMode::LineBoundary),
      ),
      VirtualKey::Named(NamedKey::End) => self.resolve_boundary_position(
        current,
        self
          .host
          .move_right(&current, MoveMode::LineBoundary),
      ),
      _ => return Err(()),
    };

    let Some(next) = next else {
      return Ok(None);
    };
    let anchor = if event.with_shift_key() { self.selection.anchor } else { next };
    Ok(Some(self.host.make_range(anchor, next)))
  }

  fn selection_command(&self, event: &KeyboardEvent) -> Result<Option<TextSelectionRange>, ()> {
    if !event.with_command_key() {
      return Err(());
    }

    if *event.key_code() == PhysicalKey::Code(KeyCode::KeyC)
      && let Some(scope) = Provider::of::<SelectionCoordinatorHandle>(event)
    {
      let selection = scope.selection_data();
      if !selection.is_empty() {
        let _ = copy_selection_data_to_clipboard(selection.as_slice());
        return Ok(None);
      }
    }

    match event.key_code() {
      PhysicalKey::Code(KeyCode::KeyC) => {
        let text = self.substr(self.cluster_rg());
        if !text.is_empty() {
          let clipboard = AppCtx::clipboard();
          let _ = clipboard.borrow_mut().clear();
          let _ = clipboard.borrow_mut().write_text(&text);
        }
        Ok(None)
      }
      PhysicalKey::Code(KeyCode::KeyA) => {
        let Some(anchor) = self.host.first_position() else {
          return Ok(None);
        };
        let Some(focus) = self.host.last_position() else {
          return Ok(None);
        };
        if anchor == focus { Ok(None) } else { Ok(Some(self.host.make_range(anchor, focus))) }
      }
      _ => Err(()),
    }
  }
}

impl<T: EditText + 'static> LocalSelectable for BasicEditor<T> {
  type Position = TextPosition;
  type Range = TextSelectionRange;

  fn hit_test(&self, point: Point) -> Option<Self::Position> { self.host.hit_test(point) }

  fn nearest_position(&self, point: Point) -> Option<Self::Position> {
    self.host.nearest_position(point)
  }

  fn first_position(&self) -> Option<Self::Position> { self.host.first_position() }

  fn last_position(&self) -> Option<Self::Position> { self.host.last_position() }

  fn move_left(&self, pos: &Self::Position, mode: MoveMode) -> BoundaryResult<Self::Position> {
    self.host.move_left(pos, mode)
  }

  fn move_right(&self, pos: &Self::Position, mode: MoveMode) -> BoundaryResult<Self::Position> {
    self.host.move_right(pos, mode)
  }

  fn move_up(&self, pos: &Self::Position) -> BoundaryResult<Self::Position> {
    self.host.move_up(pos)
  }

  fn move_down(&self, pos: &Self::Position) -> BoundaryResult<Self::Position> {
    self.host.move_down(pos)
  }

  fn make_range(&self, anchor: Self::Position, focus: Self::Position) -> Self::Range {
    self.host.make_range(anchor, focus)
  }

  fn is_collapsed(&self, range: &Self::Range) -> bool { self.host.is_collapsed(range) }

  fn caret_rect(&self, pos: &Self::Position) -> Option<Rect> { self.host.caret_rect(pos) }

  fn selection_rects(&self, range: &Self::Range) -> Vec<Rect> { self.host.selection_rects(range) }

  fn select_unit(&self, pos: &Self::Position, mode: MoveMode) -> Option<Self::Range> {
    self.host.select_unit(pos, mode)
  }
}

impl<T: EditText + 'static> SelectableWithContent for BasicEditor<T> {
  fn selection_text(&self, range: &TextSelectionRange) -> String {
    self.substr(range.cluster_rg()).to_string()
  }
}

impl<T: EditText + 'static> BasicEditor<T> {
  fn chars_handle(&mut self, event: &CharsEvent) -> bool {
    if event.common.with_command_key() {
      return false;
    }

    let chars = event
      .chars
      .chars()
      .filter(|c| !c.is_control() || c.is_ascii_whitespace())
      .collect::<String>();
    if !chars.is_empty() {
      self.insert(&chars);
      return true;
    }
    false
  }

  fn keys_handle(&mut self, event: &KeyboardEvent) -> bool {
    let mut deal = false;
    if event.with_command_key() {
      deal = self.edit_with_command(event);
    }
    if !deal {
      deal = self.edit_with_key(event);
    }
    deal
  }

  fn edit_with_command(&mut self, event: &KeyboardEvent) -> bool {
    if !event.with_command_key() {
      return false;
    }
    match event.key_code() {
      PhysicalKey::Code(KeyCode::KeyV) => {
        let clipboard = AppCtx::clipboard();
        let txt = clipboard.borrow_mut().read_text();
        if let Ok(txt) = txt {
          self.insert(&txt);
          return true;
        }
      }
      PhysicalKey::Code(KeyCode::KeyX) => {
        let rg = self.cluster_rg();
        if !rg.is_empty() {
          let txt = self.substr(rg).to_string();
          self.del_sel();
          let clipboard = AppCtx::clipboard();
          let _ = clipboard.borrow_mut().clear();
          let _ = clipboard.borrow_mut().write_text(&txt);
          return true;
        }
      }
      _ => {}
    };
    false
  }

  fn edit_with_key(&mut self, key: &KeyboardEvent) -> bool {
    match key.key() {
      VirtualKey::Named(NamedKey::Backspace) => {
        let mut rg = self.cluster_rg();
        if rg.is_empty() {
          let len = self.measure_bytes(rg.start, -1);
          rg = Range { start: rg.start - len, end: rg.start };
        }
        !self.delete(rg).is_empty()
      }
      VirtualKey::Named(NamedKey::Delete) => {
        let mut rg = self.cluster_rg();
        if rg.is_empty() {
          let len = self.measure_bytes(rg.start, 1);
          rg = Range { start: rg.start, end: rg.start + len };
        }
        !self.delete(rg).is_empty()
      }
      _ => false,
    }
  }

  fn insert(&mut self, chars: &str) -> usize {
    let del_rg = self.del_sel();
    let len = self.text_mut().insert_str(del_rg.start, chars);
    let pos = TextPosition::new(len + del_rg.start);
    self.selection.range = TextSelectionRange::splat(pos);
    len
  }

  fn del_sel(&mut self) -> Range<usize> { self.delete(self.cluster_rg()) }

  fn delete(&mut self, rg: Range<usize>) -> Range<usize> {
    let del_rg = self.text_mut().del_rg_str(rg);
    self.selection.range = TextSelectionRange::splat(TextPosition::new(del_rg.start));
    del_rg
  }

  fn snapshot(&self) -> EditorSnapshot {
    EditorSnapshot {
      text: self.substr(0..self.len()).to_string().into(),
      selection: self.cluster_rg(),
    }
  }

  fn bubble_change_events(
    &self, before: EditorSnapshot, e: &impl std::ops::Deref<Target = CommonEvent>,
  ) {
    let after = self.snapshot();
    let EditorSnapshot { text: from_text, selection: from_selection } = before;

    if from_text != after.text {
      e.window().bubble_custom_event(
        e.current_target(),
        TextChanged { from: from_text, to: after.text.clone() },
      );
    }

    if from_selection != after.selection {
      e.window().bubble_custom_event(
        e.current_target(),
        TextSelectChanged { from: from_selection, to: after.selection },
      );
    }
  }

  fn is_in_pre_edit(&self) -> bool { self.pre_edit.is_some() }

  fn process_pre_edit(&mut self, e: &ImePreEditEvent) {
    match &e.pre_edit {
      ImePreEdit::Begin => {
        self.del_sel();
        self.pre_edit = Some(PreEditState { position: self.cluster_rg().start, value: None });
      }
      ImePreEdit::PreEdit { value, cursor } => {
        let Some(pre_edit) = self.pre_edit.as_mut() else {
          return;
        };
        // Safety: it is safe to modify all the fields to avoid conflicts with the
        // borrow checker.
        let PreEditState { position: pos, value: editing } =
          unsafe { &mut *(pre_edit as *mut PreEditState) };
        if let Some(txt) = editing {
          self.delete(Range { start: *pos, end: *pos + txt.len() });
        }
        let len = self.insert(value);
        let pos = if len == value.len() {
          *editing = Some(value.clone());
          TextPosition::new(*pos + cursor.map(|(start, _)| start).unwrap_or(0))
        } else {
          *editing = Some(
            self
              .substr(Range { start: *pos, end: *pos + len })
              .to_string(),
          );
          TextPosition::new(*pos + len)
        };
        self.selection.range = TextSelectionRange::splat(pos);
      }
      ImePreEdit::End => {
        if let Some(PreEditState { value: Some(txt), position, .. }) = self.pre_edit.take() {
          self.delete(Range { start: position, end: position + txt.len() });
        }
      }
    }
  }
}

#[derive(Debug)]
struct PreEditState {
  position: usize,
  value: Option<String>,
}

struct EditorSnapshot {
  text: CowArc<str>,
  selection: Range<usize>,
}

impl<T> std::ops::Deref for BasicEditor<T> {
  type Target = TextSelectable<T>;

  fn deref(&self) -> &Self::Target { &self.host }
}

impl<T> std::ops::DerefMut for BasicEditor<T> {
  fn deref_mut(&mut self) -> &mut Self::Target { &mut self.host }
}
