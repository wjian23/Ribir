use std::ops::Range;

use ribir_core::prelude::{anchor::Anchor, *};

use super::{
  edit_text::EditText,
  text_selectable::{
    BoundaryResult, MoveMode, Selectable as LocalSelectable, SelectableWithContent, TextPosition,
    TextSelectable, TextSelection, TextSelectionRange,
  },
};
use crate::{
  prelude::*,
  selectable_area::{
    SelectableArea, SelectableAreaPosition, SelectableAreaSelection,
    SelectableAreaSelectionChangedEvent, TextAreaData,
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

impl<T: Default + SyncLayoutVisualText + EditText + Clone + 'static> Compose for BasicEditor<T> {
  fn compose(this: impl StateWriter<Value = Self>) -> Widget<'static> {
    // let area_state = Stateful::new(SelectableArea::<TextPosition,
    // TextAreaData>::default());
    fn_widget! {
      let mut area = @SelectableArea::<TextPosition, TextAreaData> {};
      let text_state = part_writer!(&mut this.host.text);

      let text = part_writer!(&mut this.host);


      let caret = pipe!(*$read(area.is_focused()))
        .transform(|p| p.distinct_until_changed())
        .map(move |v| v.then(|| fn_widget!{ Self::caret_widget($writer(this)) }));


      @Stack {
        fit: StackFit::Passthrough,
        on_focus_in: move |e| {
          e.window().set_ime_allowed(true);
          $read(this).sync_selection_to_area(&mut $write(area));
        },
        on_focus_out: move |e| { e.window().set_ime_allowed(false); },
        on_chars: move |e| {
          let before = $read(this).snapshot();
          let mut editor_state = $write(this);
          if editor_state.chars_handle(e) {
            editor_state.sync_selection_to_area(&mut $write(area));
            editor_state.bubble_change_events(before, e, true);
          } else {
            editor_state.forget_modifies();
          }
        },
        on_key_down: move |e| {
          let before = $read(this).snapshot();
          let mut editor_state = $write(this);
          if editor_state.keys_handle(e) {
            editor_state.sync_selection_to_area(&mut $write(area));
            editor_state.bubble_change_events(before, e, true);
          } else {
            editor_state.forget_modifies();
          }
        },
        on_ime_pre_edit: move |e| {
          let before = $read(this).snapshot();
          let mut editor_state = $write(this);
          editor_state.process_pre_edit(e);
          editor_state.sync_selection_to_area(&mut $write(area));
          editor_state.bubble_change_events(before, e, true);
        },
        on_custom: move |e: &mut SelectableAreaSelectionChangedEvent<TextPosition>| {
          let Some(selection) = e.data().to.as_ref() else {
            return;
          };
          let from = $read(this).cluster_rg();
          if $write(this).sync_selection_from_area(selection) {
            let to = $read(this).cluster_rg();
            e.window()
              .bubble_custom_event(e.current_target(), TextSelectChanged { from, to });
          }
        },

        @(area){ @ {text } }
        @InParentLayout {
          @Providers {
            providers: [Provider::writer(text_state.clone_writer(), Some(DirtyPhase::Layout))],
            @ { caret }
          }
        }
      }
    }
    .into_widget()
  }
}

impl<T> BasicEditor<T> {
  pub fn cluster_rg(&self) -> Range<usize> { self.selection.cluster_rg() }

  pub fn selection_range(&self) -> &TextSelectionRange { &self.selection.range }

  pub fn selection_range_mut(&mut self) -> &mut TextSelectionRange { &mut self.selection.range }
}

impl<T: EditText + SyncLayoutVisualText + 'static> BasicEditor<T> {
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
    LocalSelectable::caret_rect(&self.host, &self.selection.focus).unwrap_or_default()
  }

  fn sync_selection_from_area(
    &mut self, selection: &SelectableAreaSelection<TextPosition>,
  ) -> bool {
    if selection.anchor.idx != 0 || selection.focus.idx != 0 {
      return false;
    }
    let selection =
      TextSelectionRange { anchor: selection.anchor.position, focus: selection.focus.position };
    if self.selection.range == selection {
      false
    } else {
      self.selection.range = selection;
      true
    }
  }

  fn sync_selection_to_area(&self, area: &mut SelectableArea<TextPosition, TextAreaData>) {
    area.set_selection(Some(SelectableAreaSelection {
      anchor: SelectableAreaPosition { idx: 0, position: self.selection.anchor },
      focus: SelectableAreaPosition { idx: 0, position: self.selection.focus },
    }));
  }
}

impl<T: EditText + SyncLayoutVisualText + 'static> LocalSelectable for BasicEditor<T> {
  type Position = TextPosition;
  type Range = TextSelectionRange;

  fn hit_test(&self, point: Point) -> Option<Self::Position> {
    LocalSelectable::hit_test(&self.host, point)
  }

  fn nearest_position(&self, point: Point) -> Option<Self::Position> {
    LocalSelectable::nearest_position(&self.host, point)
  }

  fn first_position(&self) -> Option<Self::Position> { LocalSelectable::first_position(&self.host) }

  fn last_position(&self) -> Option<Self::Position> { LocalSelectable::last_position(&self.host) }

  fn move_left(&self, pos: &Self::Position, mode: MoveMode) -> BoundaryResult<Self::Position> {
    LocalSelectable::move_left(&self.host, pos, mode)
  }

  fn move_right(&self, pos: &Self::Position, mode: MoveMode) -> BoundaryResult<Self::Position> {
    LocalSelectable::move_right(&self.host, pos, mode)
  }

  fn move_up(&self, pos: &Self::Position) -> BoundaryResult<Self::Position> {
    LocalSelectable::move_up(&self.host, pos)
  }

  fn move_down(&self, pos: &Self::Position) -> BoundaryResult<Self::Position> {
    LocalSelectable::move_down(&self.host, pos)
  }

  fn make_range(&self, anchor: Self::Position, focus: Self::Position) -> Self::Range {
    LocalSelectable::make_range(&self.host, anchor, focus)
  }

  fn is_collapsed(&self, range: &Self::Range) -> bool {
    LocalSelectable::is_collapsed(&self.host, range)
  }

  fn caret_rect(&self, pos: &Self::Position) -> Option<Rect> {
    LocalSelectable::caret_rect(&self.host, pos)
  }

  fn selection_rects(&self, range: &Self::Range) -> Vec<Rect> {
    LocalSelectable::selection_rects(&self.host, range)
  }

  fn select_unit(&self, pos: &Self::Position, mode: MoveMode) -> Option<Self::Range> {
    LocalSelectable::select_unit(&self.host, pos, mode)
  }
}

impl<T: EditText + SyncLayoutVisualText + 'static> SelectableWithContent for BasicEditor<T> {
  fn selection_text(&self, range: &TextSelectionRange) -> String {
    self.substr(range.cluster_rg()).to_string()
  }
}

impl<T: EditText + SyncLayoutVisualText + 'static> BasicEditor<T> {
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
    emit_selection_change: bool,
  ) {
    let after = self.snapshot();
    let EditorSnapshot { text: from_text, selection: from_selection } = before;

    if from_text != after.text {
      e.window().bubble_custom_event(
        e.current_target(),
        TextChanged { from: from_text, to: after.text.clone() },
      );
    }

    if emit_selection_change && from_selection != after.selection {
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
