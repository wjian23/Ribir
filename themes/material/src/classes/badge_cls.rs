use ribir_core::prelude::*;
use ribir_widgets::prelude::{BADGE, BADGE_LARGE, BADGE_SMALL};

pub(super) fn init(classes: &mut Classes) {
  classes.insert(BADGE, |w| {
    fn_widget! {
      @FatObj {
        background: Palette::of(BuildCtx::get()).error(),
        radius: Radius::all(16.),
        @ { w }
      }
    }
    .into_widget()
  });

  classes.insert(BADGE_SMALL, |w| {
    fn_widget! {
      @FatObj {
        clamp: BoxClamp::fixed_size(Size::new(6., 6.)),
        padding: EdgeInsets::all(0.),
        @ { w }
      }
    }
    .into_widget()
  });

  classes.insert(BADGE_LARGE, |w| {
    fn_widget! {
      @FatObj {
        clamp: BoxClamp::min_width(16.).with_min_height(16.),
        padding: EdgeInsets::horizontal(4.),
        @ { w }
      }
    }
    .into_widget()
  });
}
