use crate::{prelude::*, wrap_render::*};

/// A wrapper that constrains child to fixed width and/or height based on
/// `Measure` values.
///
/// This is a built-in `FatObj` field. Setting the `width` or `height` field
/// attaches a `FixedSize` which constrains the child's dimensions.
///
/// When using `Measure::Percent`, the percentage is calculated relative to the
/// incoming clamp's max size.
///
/// # Example
///
/// Set a widget to 50% width of its parent's max width:
///
/// ```rust
/// use ribir::prelude::*;
///
/// fn_widget! {
///   @Text {
///     width: 0.5.percent(),
///     text: "50% width"
///   }
/// };
/// ```
#[derive(Clone, Default)]
pub struct FixedSize {
  pub width: Option<Measure>,
  pub height: Option<Measure>,
}

impl Declare for FixedSize {
  type Builder = FatObj<()>;
  #[inline]
  fn declarer() -> Self::Builder { FatObj::new(()) }
}

impl_compose_child_for_wrap_render!(FixedSize);

impl WrapRender for FixedSize {
  fn measure(&self, clamp: BoxClamp, host: &dyn Render, ctx: &mut LayoutCtx) -> Size {
    let mut new_clamp = clamp;
    if let Some(w) = self.width {
      let fixed_w = w.into_pixel(clamp.max.width);
      new_clamp = new_clamp.with_fixed_width(fixed_w);
    }
    if let Some(h) = self.height {
      let fixed_h = h.into_pixel(clamp.max.height);
      new_clamp = new_clamp.with_fixed_height(fixed_h);
    }
    host.measure(new_clamp, ctx)
  }

  #[inline]
  fn wrapper_dirty_phase(&self) -> DirtyPhase { DirtyPhase::Layout }
}

#[cfg(test)]
mod tests {
  use ribir_dev_helper::*;

  use super::*;
  use crate::test_helper::*;

  widget_layout_test!(
    fixed_width_pixel,
    WidgetTester::new(fn_widget! {
      @FixedSize {
        width: 100.px(),
        @Void {}
      }
    })
    .with_wnd_size(Size::new(500., 500.)),
    LayoutCase::default().with_size(Size::new(100., 500.))
  );

  widget_layout_test!(
    fixed_height_pixel,
    WidgetTester::new(fn_widget! {
      @FixedSize {
        height: 100.,
        @Void {}
      }
    })
    .with_wnd_size(Size::new(500., 500.)),
    LayoutCase::default().with_size(Size::new(500., 100.))
  );

  widget_layout_test!(
    fixed_width_percent,
    WidgetTester::new(fn_widget! {
      @FixedSize {
        width: Measure::Percent(0.5),
        @Void {}
      }
    })
    .with_wnd_size(Size::new(500., 500.)),
    LayoutCase::default().with_size(Size::new(250., 500.))
  );

  widget_layout_test!(
    fixed_both,
    WidgetTester::new(fn_widget! {
      @FixedSize {
        width: 100.px(),
        height: 50.px(),
        @Void {}
      }
    })
    .with_wnd_size(Size::new(500., 500.)),
    LayoutCase::default().with_size(Size::new(100., 50.))
  );
}
