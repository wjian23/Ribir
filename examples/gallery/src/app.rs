use ribir::prelude::*;

pub fn gallery() -> Widget<'static> {
  icon! {
    x: AnchorX::at_center(),
    y: AnchorY::at_center(),
    text_line_height: 128.,
    @ asset!("../assets/logo.svg", "svg")
  }
  .into_widget()
}
