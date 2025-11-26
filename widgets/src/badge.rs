use ribir_core::{class_names, prelude::*};

use crate::prelude::*;

/// The `Badge` widget is used to show notifications, counts, or status
/// information on top of another widget.
///
/// # Example
///
/// ```
/// # use ribir_core::prelude::*;
/// # use ribir_widgets::prelude::*;
///
/// let _badge = badge! {
///   content: Some(CowArc::from("New")),
///   @Text { text: "Child widget" }
/// };
///
/// let _dot_badge = badge! {
///   content: Some(CowArc::from("")),
///   @Text { text: "Child widget" }
/// };
/// ```
#[derive(Clone, Declare, PartialEq)]
pub struct Badge {
  /// The content to display inside the badge.
  /// - `Some("text")`: Display the text.
  /// - `Some("")`: Display a small dot.
  /// - `None`: Hide the badge.
  #[declare(default)]
  pub content: Option<CowArc<str>>,
  /// The offset to adjust the badge's position relative to the bounding
  /// rectangle of the child widget.
  #[declare(default = Anchor::right_top(0., 0.))]
  pub offset: Anchor,
}

/// The `NumBadge` widget is a specialized badge for displaying numeric counts.
///
/// # Example
///
/// ```
/// # use ribir_core::prelude::*;
/// # use ribir_widgets::prelude::*;
///
/// let _num_badge = num_badge! {
///   count: Some(5),
///   @Text { text: "Child widget" }
/// };
///
/// let _overflow_badge = num_badge! {
///   count: Some(100),
///   max_count: 99u32,
///   @Text { text: "Child widget" }
/// };
/// ```
#[derive(Clone, Declare, PartialEq)]
pub struct NumBadge {
  /// The number to display.
  /// - `Some(n)`: Display the number `n` (or `max_count+` if `n > max_count`).
  /// - `None`: Hide the badge.
  #[declare(default)]
  pub count: Option<u32>,
  /// The maximum number to display before truncating with a "+".
  /// Defaults to 999.
  #[declare(default = 999u32)]
  pub max_count: u32,
  /// The offset to adjust the badge's position relative to the bounding
  /// rectangle of the child widget.
  #[declare(default = Anchor::right_top(0., 0.))]
  pub offset: Anchor,
}

class_names! {
  /// The class name for the badge container.
  BADGE,
  /// The class name for the badge text.
  BADGE_TEXT,
  /// The class name for the small badge (dot).
  BADGE_SMALL,
  /// The class name for the large badge (with content).
  BADGE_LARGE,
}

impl ComposeChild<'static> for Badge {
  type Child = Widget<'static>;

  fn compose_child(this: impl StateWriter<Value = Self>, child: Self::Child) -> Widget<'static> {
    fn_widget! {
      let child = FatObj::new(child);

      let badge = @Text {
        text: pipe!($read(this).content.clone().unwrap_or_default()),
        foreground: Palette::of(BuildCtx::get()).on_error(),
        text_style: TypographyTheme::of(BuildCtx::get()).label_small.text.clone(),
        class: pipe! {
          if $read(this).content.as_ref().map_or(true, |s| s.is_empty()) {
            BADGE_SMALL
          } else {
            BADGE_LARGE
          }
        }
      };

      @Stack {
        @ { child }
        @ InParentLayout {
          @Stack {
            class: BADGE,
            visible: pipe!($read(this).content.is_some()),
            anchor: pipe!($read(this).offset),
            @ { badge }
          }
        }
      }
    }
    .into_widget()
  }
}

impl ComposeChild<'static> for NumBadge {
  type Child = Widget<'static>;

  fn compose_child(this: impl StateWriter<Value = Self>, child: Self::Child) -> Widget<'static> {
    fn_widget! {
      let content = pipe! {
        let this = $read(this);
        this.count.map(|count| {
          if count > this.max_count {
            format!("{}+", this.max_count).into()
          } else {
            count.to_string().into()
          }
        })
      };

      @Badge {
        content,
        offset: pipe!($read(this).offset),
        @ { child }
      }
    }
    .into_widget()
  }
}

#[cfg(test)]
mod tests {
  use ribir_core::test_helper::*;
  use ribir_dev_helper::*;

  use super::*;

  widget_image_tests!(
    badge,
    WidgetTester::new(self::column! {
      @Badge {
        content: Some("".into()),
        @Container { size: Size::new(40., 40.), background: Color::GRAY }
      }
      @Badge {
        content: Some("error!".into()),
        offset: Anchor::right(-14.),
        @Container { size: Size::new(40., 40.)}
      }
      @NumBadge {
        count: 1000,
        max_count: 99_u32,
        @Container { size: Size::new(40., 40.), background: Color::GRAY }
      }
    })
    .with_wnd_size(Size::new(200., 200.))
    .with_comparison(0.0001),
  );
}
