use ribir_core::prelude::*;
use ribir_widgets::input::{INPUT, TEXT_CARET, TEXT_SELECTION, TEXTAREA, TextSelectionStyle};

use crate::md;

pub(super) fn init(classes: &mut Classes) {
  classes.insert(TEXT_CARET, |w| {
    rdl! {
      let mut w = FatObj::new(w);
      let blink_interval = Duration::from_millis(500);
      let u = Local::interval(blink_interval)
        .subscribe(move |idx| *$write(w.opacity()) = (idx % 2) as f32);
      let border = BuildCtx::color()
        .map(|color| Border::only_left(BorderSide::new(2., (*color).into())));
      @(w) {
        clamp: BoxClamp::fixed_width(2.),
        border,
        on_disposed: move |_| u.unsubscribe()
      }
    }
    .into_widget()
  });

  classes.insert(TEXT_SELECTION, |w| {
    let brush = BuildCtx::color()
      .into_container_color(BuildCtx::get())
      .map(|c| Brush::from(c.with_alpha(0.8)))
      .into_pipe_value();
    let (brush, brush_pipe) = brush.unzip();
    let style = Stateful::new(TextSelectionStyle { brush });
    let mut w = FatObj::new(w);
    if let Some(pipe) = brush_pipe {
      let style = style.clone_writer();
      let subscription = pipe
        .subscribe(move |brush| style.write().brush = brush)
        .unsubscribe_when_dropped();
      w.on_disposed(move |_| drop(subscription));
    }
    providers! {
      providers: [Provider::writer(style, Some(DirtyPhase::Paint))],
      @ { w }
    }
    .into_widget()
  });

  fn input_border(w: Widget) -> Widget {
    let mut w = FatObj::new(w);
    let blur = Palette::of(BuildCtx::get()).on_surface_variant();

    let focus_watcher = w.is_focused();
    let border = BuildCtx::color().combine_with(focus_watcher, move |(c, focus)| {
      let color = if *focus { *c } else { blur };
      Border::all(BorderSide::new(1., color.into()))
    });

    w.with_border(border).with_radius(md::RADIUS_2);
    w.into_widget()
  }
  classes.insert(INPUT, input_border);
  classes.insert(TEXTAREA, input_border);
}
