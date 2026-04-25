use std::ops::{Deref, DerefMut, Range};

use ribir_core::prelude::*;

use crate::prelude::*;

mod edit_text;
mod text_glyphs;

mod text_editable;
mod text_selectable;

pub use edit_text::*;
pub use text_editable::*;
pub use text_glyphs::*;
pub use text_selectable::*;

class_names!(
  ///Class name for the input widget
  INPUT,
  ///Class name for the text area widget
  TEXTAREA,
);

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct TextChanged {
  pub from: CowArc<str>,
  pub to: CowArc<str>,
}

pub type TextChangedEvent = CustomEvent<TextChanged>;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct TextSelectChanged {
  pub from: Range<usize>,
  pub to: Range<usize>,
}

pub type TextSelectChangedEvent = CustomEvent<TextSelectChanged>;

/// The `Input` struct is a widget that represents a text input field
/// that displays a single line of text. if you need multi line text, use
/// `[TextArea]`
///
/// The Input will emit the [TextChangedEvent] event when the text is changed,
/// emit the [TextSelectChanged] event when the text selection is changed.
/// The Input also implement the [EditableText] trait, which you can set
/// the text and the caret selection.
///
/// ## Example
///
/// ```rust no_run
/// use ribir::prelude::*;
/// let w = fn_widget! {
///   let input = @Input {};
///   @Column {
///     @ Text { text: pipe!("the input value is:".to_string() + $read(input).text()) }
///     @ Row {
///       @ Text { text: "input value:" }
///       @ { input }
///     }
///   }
/// };
/// App::run(w);
/// ```
pub struct Input {
  basic: BasicEditor<InputText>,
}

impl Input {
  /// set the text and the caret selection will be reset to the start.
  pub fn set_text(&mut self, text: &str) {
    let v = text
      .chars()
      .filter(|c| *c != '\n' && *c != '\r')
      .collect::<String>();
    *self.basic.text_mut() = InputText::new(v);
    *self.basic.selection_range_mut() = TextSelectionRange::default();
  }

  pub fn text(&self) -> &CowArc<str> { &self.basic.text().0 }

  /// set the caret selection, and the caret position will be set to the `to`
  /// cluster
  pub fn select(&mut self, from: usize, to: usize) {
    *self.basic.selection_range_mut() =
      TextSelectionRange { anchor: TextPosition::new(from), focus: TextPosition::new(to) };
  }

  /// return the selection range of the text
  pub fn selection(&self) -> Range<usize> { self.basic.cluster_rg() }
}

pub struct InputDeclarer {
  text: Option<CowArc<str>>,
  obj: FatObj<()>,
}

impl Declare for Input {
  type Builder = InputDeclarer;

  fn declarer() -> Self::Builder { InputDeclarer { text: None, obj: FatObj::new(()) } }
}

impl ObjDeclarer for InputDeclarer {
  type Target = FatObj<Stateful<Input>>;

  fn finish(mut self) -> Self::Target {
    let input = Input { basic: BasicEditor::default() };
    let input = if let Some(text) = self.text.take() {
      let i = Stateful::new(input);
      i.write().set_text(text.as_ref());
      i
    } else {
      Stateful::new(input)
    };

    self.obj.map(|_| input)
  }
}

impl InputDeclarer {
  /// Initialize the input text once during widget composition.
  pub fn with_text(&mut self, text: impl Into<CowArc<str>>) -> &mut Self {
    assert!(self.text.is_none(), "Input: `text` is already set");
    self.text = Some(text.into());
    self
  }
}

impl Deref for InputDeclarer {
  type Target = FatObj<()>;

  fn deref(&self) -> &Self::Target { &self.obj }
}

impl DerefMut for InputDeclarer {
  fn deref_mut(&mut self) -> &mut Self::Target { &mut self.obj }
}

/// The `TextArea` struct is a widget that represents a text input field
/// that displays multiple lines of text. for single line text, use `[Input]`
pub struct TextArea {
  /// if true, the text will be auto wrap when the text is too long
  auto_wrap: bool,
  basic: BasicEditor<CowArc<str>>,
}

impl TextArea {
  /// set the text and the caret selection will be reset to the start.
  pub fn set_text(&mut self, text: &str) {
    *self.basic.text_mut() = text.to_string().into();
    *self.basic.selection_range_mut() = TextSelectionRange::default();
  }

  pub fn text(&self) -> &CowArc<str> { self.basic.text() }

  /// set the caret selection, and the caret position will be set to the `to`
  /// cluster
  pub fn select(&mut self, from: usize, to: usize) {
    *self.basic.selection_range_mut() =
      TextSelectionRange { anchor: TextPosition::new(from), focus: TextPosition::new(to) };
  }

  /// return the selection range of the text
  pub fn selection(&self) -> Range<usize> { self.basic.cluster_rg() }
}

pub struct TextAreaDeclarer {
  text: Option<CowArc<str>>,
  auto_wrap: bool,
  obj: FatObj<()>,
}

impl Declare for TextArea {
  type Builder = TextAreaDeclarer;

  fn declarer() -> Self::Builder {
    TextAreaDeclarer { text: None, auto_wrap: true, obj: FatObj::new(()) }
  }
}

impl ObjDeclarer for TextAreaDeclarer {
  type Target = FatObj<Stateful<TextArea>>;

  fn finish(mut self) -> Self::Target {
    let text_area = TextArea { auto_wrap: self.auto_wrap, basic: BasicEditor::default() };
    let text_area = if let Some(text) = self.text.take() {
      let t = Stateful::new(text_area);
      t.write().set_text(text.as_ref());
      t
    } else {
      Stateful::new(text_area)
    };

    self.obj.map(|_| text_area)
  }
}

impl TextAreaDeclarer {
  /// Initialize the text area content once during widget composition.
  pub fn with_text(&mut self, text: impl Into<CowArc<str>>) -> &mut Self {
    assert!(self.text.is_none(), "TextArea: `text` is already set");
    self.text = Some(text.into());
    self
  }
}

impl Deref for TextAreaDeclarer {
  type Target = FatObj<()>;

  fn deref(&self) -> &Self::Target { &self.obj }
}

impl DerefMut for TextAreaDeclarer {
  fn deref_mut(&mut self) -> &mut Self::Target { &mut self.obj }
}

#[derive(Clone, Eq, PartialEq, Default)]
pub struct InputText(CowArc<str>);
impl InputText {
  pub fn new(v: impl Into<CowArc<str>>) -> Self { InputText(v.into()) }
  pub fn text(&self) -> &CowArc<str> { &self.0 }
}

impl BaseText for InputText {
  fn len(&self) -> usize { self.0.len() }
  fn substr(&self, rg: Range<usize>) -> Substr { self.0.substr(rg) }
  fn measure_bytes(&self, byte_from: usize, char_len: isize) -> usize {
    self.0.measure_bytes(byte_from, char_len)
  }
  fn select_token(&self, byte_from: usize) -> Range<usize> {
    BaseText::select_token(&self.0, byte_from)
  }
}

impl VisualText for InputText {
  fn layout_glyphs(&self, clamp: BoxClamp, ctx: &MeasureCtx) -> ParagraphLayoutRef {
    self.0.layout_glyphs(clamp, ctx)
  }

  fn paint(
    &self, painter: &mut Painter, style: PaintingStyle, glyphs: &ParagraphLayoutRef, rect: Rect,
  ) {
    self.0.paint(painter, style, glyphs, rect);
  }
}

impl SyncLayoutVisualText for InputText {
  fn sync_layout_glyphs(&self, layout_ctx: &TextGlyphLayoutContext) -> ParagraphLayoutRef {
    self.0.sync_layout_glyphs(layout_ctx)
  }
}

impl EditText for InputText {
  fn insert_str(&mut self, at: usize, v: &str) -> usize {
    let new_v = v
      .chars()
      .filter(|c| *c != '\n' && *c != '\r')
      .collect::<String>();
    self.0.insert_str(at, new_v.as_str())
  }

  fn del_rg_str(&mut self, rg: Range<usize>) -> Range<usize> { self.0.del_rg_str(rg) }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct CaretPosition {
  /// the cluster of the caret
  pub cluster: usize,
  /// the affinity of the caret when the logical cluster maps to multiple visual
  /// slots.
  pub affinity: CaretAffinity,
  /// the position of the caret, it may be set by the ui interaction
  pub position: Option<(usize, usize)>,
}

impl Ord for CaretPosition {
  fn cmp(&self, other: &Self) -> std::cmp::Ordering {
    self.cluster.cmp(&other.cluster).then_with(|| {
      if self.affinity == other.affinity {
        std::cmp::Ordering::Equal
      } else if self.affinity == CaretAffinity::Downstream {
        std::cmp::Ordering::Greater
      } else {
        std::cmp::Ordering::Less
      }
    })
  }
}

impl PartialOrd for CaretPosition {
  fn partial_cmp(&self, other: &Self) -> Option<std::cmp::Ordering> { Some(self.cmp(other)) }
}

impl CaretPosition {
  pub const fn new(cluster: usize) -> Self {
    Self { cluster, affinity: CaretAffinity::Downstream, position: None }
  }
}

impl Default for CaretPosition {
  fn default() -> Self { Self::new(0) }
}

impl Compose for Input {
  fn compose(this: impl StateWriter<Value = Self>) -> Widget<'static> {
    fn_widget! {
      let basic = part_writer!(&mut this.basic);
      focus_scope! {
        skip_host: true,
        @TextClamp {
          rows: Some(1.),
          cols: Some(20.),
          class: INPUT,
          @FatObj {
            scrollable: Scrollable::X,
            @ { basic }
          }
        }
      }
    }
    .into_widget()
  }
}

impl Compose for TextArea {
  fn compose(this: impl StateWriter<Value = Self>) -> Widget<'static> {
    fn_widget! {
      let basic = part_writer!(&mut this.basic);
      focus_scope! {
        @TextClamp {
          rows: Some(2.),
          cols: Some(20.),
          class: TEXTAREA,
          @Scrollbar {
            text_overflow: pipe!(if $read(this).auto_wrap {
              TextOverflow::AutoWrap
            } else {
              TextOverflow::Overflow
            }),
            @ { basic }
          }
        }
      }
    }
    .into_widget()
  }
}

#[cfg(test)]
mod tests {
  use ribir_core::{prelude::*, reset_test_env, test_helper::*};

  use super::*;

  #[test]
  fn input_edit() {
    reset_test_env!();
    let (value, w_value) = split_value(String::default());
    let w = fn_widget! {
      let input = @Input { auto_focus: true };
      watch!($read(input).text().clone())
        .subscribe(move |text| *$write(w_value) = text.to_string());
      input
    };

    let wnd = TestWindow::new_with_size(w, Size::new(200., 200.));
    wnd.draw_frame();
    assert_eq!(*value.read(), "");

    wnd.process_receive_chars("hello\nworld".into());
    wnd.draw_frame();
    assert_eq!(*value.read(), "helloworld");
  }

  #[test]
  fn declarer_text_initializes_input_and_text_area() {
    reset_test_env!();
    let (input_text, w_input_text) = split_value(String::default());
    let (text_area_text, w_text_area_text) = split_value(String::default());
    let w = fn_widget! {
      let input = @Input { text: "hello\nworld" };
      let text_area = @TextArea { text: "hello\nworld" };
      watch!($read(input).text().clone())
        .subscribe(move |text| *$write(w_input_text) = text.to_string());
      watch!($read(text_area).text().clone())
        .subscribe(move |text| *$write(w_text_area_text) = text.to_string());
      @Column {
        @ { input }
        @ { text_area }
      }
    };

    let wnd = TestWindow::new_with_size(w, Size::new(200., 200.));
    wnd.draw_frame();

    assert_eq!(*input_text.read(), "helloworld");
    assert_eq!(*text_area_text.read(), "hello\nworld");
  }

  #[test]
  fn input_tap_focus() {
    reset_test_env!();
    let (value, w_value) = split_value(String::default());
    let w = fn_widget! {
      let input = @Input {  };
      watch!($read(input).text().clone())
        .subscribe(move |text| *$write(w_value) = text.to_string());

      @Container {
        size: Size::new(200., 24.),
        @ { input }
      }
    };

    let wnd = TestWindow::new_with_size(w, Size::new(200., 200.));
    wnd.draw_frame();
    assert_eq!(*value.read(), "");

    wnd.process_cursor_move(Point::new(50., 10.));

    wnd.process_mouse_press(Box::new(DummyDeviceId), MouseButtons::PRIMARY);
    wnd.process_mouse_release(Box::new(DummyDeviceId), MouseButtons::PRIMARY);
    wnd.draw_frame();

    wnd.process_receive_chars("hello".into());
    wnd.draw_frame();
    assert_eq!(*value.read(), "hello");
  }

  #[test]
  fn input_double_tap_selects_word() {
    reset_test_env!();

    let input_state = Stateful::new(Input { basic: BasicEditor::default() });
    input_state.write().set_text("hello world");
    let input = input_state.clone_writer();

    let w = fn_widget! {
      @Container {
        size: Size::new(200., 24.),
        @ { input.clone_writer() }
      }
    };

    let wnd = TestWindow::new_with_size(w, Size::new(200., 200.));
    wnd.draw_frame();

    for _ in 0..2 {
      wnd.process_cursor_move(Point::new(18., 10.));
      wnd.process_mouse_press(Box::new(DummyDeviceId), MouseButtons::PRIMARY);
      wnd.process_mouse_release(Box::new(DummyDeviceId), MouseButtons::PRIMARY);
      wnd.draw_frame();
    }

    assert_eq!(input_state.read().selection(), 0..5);
  }

  #[test]
  fn input_emits_text_and_selection_changed_events() {
    reset_test_env!();
    let text_changes = Stateful::new(Vec::<TextChanged>::new());
    let selection_changes = Stateful::new(Vec::<TextSelectChanged>::new());
    let w = fn_widget! {
      @Input {
        auto_focus: true,
        on_raw_custom: move |e: &mut RawCustomEvent| {
          if let Some(e) = e.downcast_ref::<TextChanged>() {
            $write(text_changes).push(e.data().clone());
          } else if let Some(e) = e.downcast_ref::<TextSelectChanged>() {
            $write(selection_changes).push(e.data().clone());
          }
        },
      }
    };

    let wnd = TestWindow::new_with_size(w, Size::new(200., 200.));
    wnd.draw_frame();

    wnd.process_receive_chars("ab".into());
    wnd.draw_frame();

    assert_eq!(&*text_changes.read(), &[TextChanged { from: "".into(), to: "ab".into() }]);
    assert_eq!(&*selection_changes.read(), &[TextSelectChanged { from: 0..0, to: 2..2 }]);
  }
}
