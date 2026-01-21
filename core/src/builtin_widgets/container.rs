use crate::prelude::*;

/// A container widget that sizes itself to the maximum allowed by its clamp
/// constraints.
///
/// Container's size is determined by `clamp.max`, making it ideal for creating
/// fixed-size boxes when combined with the `width` and `height` builtin
/// attributes.
///
/// # Example
///
/// Place text inside a 100x100 container.
///
/// ```rust
/// use ribir::prelude::*;
///
/// container! {
///   width: 100.,
///   height: 100.,
///   background: Color::BLUE,
///   @Text { text: "Hello" }
/// };
/// ```
#[derive(Declare, SingleChild, Clone, Default)]
pub struct Container;

impl Render for Container {
  fn measure(&self, clamp: BoxClamp, ctx: &mut MeasureCtx) -> Size {
    let size = clamp.max;
    let child_clamp = BoxClamp::max_size(size);

    ctx.perform_single_child_layout(child_clamp);
    size
  }

  #[inline]
  fn size_affected_by_child(&self) -> bool { false }
}

#[cfg(test)]
mod tests {
  use ribir_dev_helper::*;

  use super::*;
  use crate::test_helper::*;

  const TEST_SIZE: Size = Size::new(100., 100.);

  widget_layout_test!(
    smoke,
    WidgetTester::new(fn_widget! {
      @Container {
        width: 100.,
        height: 100.,
      }
    }),
    LayoutCase::default().with_size(TEST_SIZE)
  );

  widget_layout_test!(
    container_with_clamp,
    WidgetTester::new(fn_widget! {
      @Container {
        clamp: BoxClamp::fixed_size(Size::new(200., 150.)),
      }
    }),
    LayoutCase::default().with_size(Size::new(200., 150.))
  );

  widget_layout_test!(
    container_with_percent_width,
    WidgetTester::new(fn_widget! {
      @Container {
        clamp: BoxClamp::max_width(400.),
        width: 0.5.percent(),
        height: 100.,
      }
    })
    .with_wnd_size(Size::new(800., 600.)),
    LayoutCase::default().with_size(Size::new(200., 100.))
  );
}
