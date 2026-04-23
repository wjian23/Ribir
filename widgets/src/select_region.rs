use ribir_core::prelude::*;

/// region select data
#[derive(Copy, Clone)]
pub enum PointerSelectData {
  Start(Point),
  Move { from: Point, to: Point },
  End { from: Point, to: Point },
}

impl PointerSelectData {
  pub fn endpoints(&self) -> (Point, Point) {
    match self {
      PointerSelectData::Start(p) => (*p, *p),
      PointerSelectData::Move { from, to } | PointerSelectData::End { from, to } => (*from, *to),
    }
  }
}

/// region select event
pub type PointerSelectEvent = CustomEvent<PointerSelectData>;

/// A Widget that extends Widget to emit SelectRegionEvent
#[declare]
pub struct PointerSelectRegion {}

fn notify_select_changed(wid: WidgetId, e: PointerSelectData, wnd: &Window) {
  wnd.bubble_custom_event(wid, e);
}

impl<'c> ComposeChild<'c> for PointerSelectRegion {
  type Child = Widget<'c>;

  fn compose_child(_: impl StateWriter<Value = Self>, child: Self::Child) -> Widget<'c> {
    fn_widget! {
      let mut child = FatObj::new(child);
      let grab_handle = Stateful::new(None);
      let from = Stateful::new(None);
      @(child) {
        on_pointer_down: move |e| {
          if e.mouse_buttons() == MouseButtons::PRIMARY {
            let pos = e.position();
            *$write(from) = Some(pos);
            *$write(grab_handle) = Some(GrabPointer::grab(e.current_target(), &e.window()));
            notify_select_changed(e.current_target(), PointerSelectData::Start(pos), &e.window());
          }
        },
        on_pointer_move: move |e| {
          let from = *$read(from);
          if let Some(from) = from
            && $read(grab_handle).is_some()
          {
            notify_select_changed(
              e.current_target(),
              PointerSelectData::Move { from, to: e.position() },
              &e.window()
            );
          }
        },
        on_pointer_up: move |e| {
          let from = $write(from).take();
          if $write(grab_handle).take().is_some()
            && let Some(from) = from
          {
            notify_select_changed(
              e.current_target(),
              PointerSelectData::End { from, to: e.position() },
              &e.window()
            );
          }
        },
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
  fn double_tap_is_preserved_while_drag_grabs_pointer() {
    reset_test_env!();

    let taps = Stateful::new(0usize);
    let w = fn_widget! {
      @PointerSelectRegion {
        on_double_tap: move |_| *$write(taps) += 1,
        @MockBox { size: Size::new(100., 100.) }
      }
    };

    let wnd = TestWindow::new_with_size(w, Size::new(100., 100.));
    wnd.draw_frame();

    for _ in 0..2 {
      wnd.process_cursor_move(Point::new(50., 50.));
      wnd.process_mouse_press(Box::new(DummyDeviceId), MouseButtons::PRIMARY);
      wnd.process_mouse_release(Box::new(DummyDeviceId), MouseButtons::PRIMARY);
      wnd.draw_frame();
    }

    assert_eq!(*taps.read(), 1);
  }
}
