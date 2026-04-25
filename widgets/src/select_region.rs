use ribir_core::prelude::*;

/// Region select data in global window coordinates.
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
pub struct PointerSelectRegion {
  /// The sensitivity of the region select, in pixels. A smaller value means
  /// more precise selection.
  #[declare(default = 0.)]
  pub sensitivity: f32,
}

fn notify_select_changed(wid: WidgetId, e: PointerSelectData, wnd: &Window) {
  wnd.bubble_custom_event(wid, e);
}

impl<'c> ComposeChild<'c> for PointerSelectRegion {
  type Child = Widget<'c>;

  fn compose_child(this: impl StateWriter<Value = Self>, child: Self::Child) -> Widget<'c> {
    fn_widget! {
      let mut child = FatObj::new(child);
      let grab_handle = Stateful::new(None);
      let from = Stateful::new(None);
      @(child) {
        on_pointer_down: move |e| {
          if e.mouse_buttons() == MouseButtons::PRIMARY {
            let pos = e.global_pos();

            *$write(from) = Some(pos);
            if $read(this).sensitivity == 0. {
              *$write(grab_handle) = GrabPointer::grab(e.current_target(), &e.window());
              notify_select_changed(e.current_target(), PointerSelectData::Start(pos), &e.window());
              e.stop_propagation();
            }
          }
        },
        on_pointer_move: move |e| {
          let from = *$read(from);
          if let Some(from) = from {
            if $read(grab_handle).is_some(){
              notify_select_changed(
                e.current_target(),
                PointerSelectData::Move { from, to: e.global_pos() },
                &e.window(),
              );
            } else if $read(this).sensitivity < (e.global_pos() - from).length() {
              *$write(grab_handle) = GrabPointer::grab(e.current_target(), &e.window());
              notify_select_changed(
                e.current_target(),
                PointerSelectData::Start(from),
                &e.window(),
              );
              e.stop_propagation();
            }
          }
        },
        on_pointer_up: move |e| {
          let from_pos = $write(from).take();
          if let Some(handle) = $write(grab_handle).take()
            && let Some(from) = from_pos
          {
            handle.release();
            notify_select_changed(
              e.current_target(),
              PointerSelectData::End { from, to: e.global_pos() },
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

  #[test]
  fn drag_delta_stays_stable_when_target_moves() {
    reset_test_env!();

    let offset = Stateful::new(0.);
    let reported_delta = Stateful::new(0.);
    let w = fn_widget! {
      @PointerSelectRegion {
        on_custom: move |e: &mut PointerSelectEvent| {
          if let PointerSelectData::Move { from, to }
            | PointerSelectData::End { from, to } = e.data() {
            let delta = to.x - from.x;
            *$write(offset) = delta;
            *$write(reported_delta) = delta;
          }
        },
        @MockBox {
          x: pipe!(*$read(offset)),
          size: Size::new(100., 100.),
        }
      }
    };

    let wnd = TestWindow::new_with_size(w, Size::new(200., 100.));
    wnd.draw_frame();

    wnd.process_cursor_move(Point::new(20., 20.));
    wnd.process_mouse_press(Box::new(DummyDeviceId), MouseButtons::PRIMARY);
    wnd.draw_frame();

    wnd.process_cursor_move(Point::new(40., 20.));
    wnd.draw_frame();
    assert_eq!(*reported_delta.read(), 20.);

    wnd.process_cursor_move(Point::new(60., 20.));
    wnd.draw_frame();
    assert_eq!(*reported_delta.read(), 40.);

    wnd.process_mouse_release(Box::new(DummyDeviceId), MouseButtons::PRIMARY);
    wnd.draw_frame();
    assert_eq!(*reported_delta.read(), 40.);
  }
}
