# Built-in Attributes & FatObj

Ribir provides a powerful built-in attribute system that allows you to add common functionality to any Widget, such as layout control (margin, alignment), visual effects (background, border, opacity, transform), and interaction events (on_tap, on_hover). These features are not implemented individually by each Widget, but are provided through a universal wrapper called `FatObj`.

## What is FatObj?

`FatObj<T>` is a generic struct in Ribir's core library. Its purpose is to temporarily wrap a Widget during the build phase (Build Time) and attach various built-in attributes to it.

When you use the `fn_widget!` or `@` syntax to declare a Widget, Ribir actually uses `FatObj` behind the scenes to handle the `margin`, `background`, `on_tap`, and other attributes you specify.

### The `@` Instantiation Process

When you use the `@` syntax (e.g., `@Text { ... }`), Ribir performs the following steps to construct the widget:

1.  **Get the Builder**: It calls the `declarer()` method from the `Declare` trait to obtain the builder for the widget.
2.  **Initialize Fields**: For each field specified in the `{ ... }` block, it calls the corresponding `with_xxx()` method on the builder (e.g., `with_text(...)`).
3.  **Finish Construction**: Finally, it calls `ObjDeclarer::finish(builder)` to complete the construction and return the declared widget.

### What `#[derive(Declare)]` Does

To support the process above, the `#[derive(Declare)]` macro automatically generates the necessary code for your widget:
*   It creates a **Builder struct** (e.g., `TextBuilder` for `Text`).
*   It implements the **`Declare` trait** for your widget, linking it to the builder.
*   It generates **`with_xxx` methods** for each field, allowing you to set values fluently.
*   It implements **`ObjDeclarer`** for the builder, which handles the final build step `finish()` and returns `FatObj<T>` (where `T` is the type of the widget being built). This ensures the widget is wrapped in `FatObj` so it can support built-in attributes.

**Simple vs Full Declare**: Ribir provides two approaches for declaring widgets:
- `#[derive(Declare)]` - Creates a full-featured declarer that includes `FatObj` wrapping and supports reactive field bindings.
- `#[simple_declare]` - Creates a simpler declarer without `FatObj` wrapping or reactive bindings. This is useful for widgets that don't need built-in attributes or complex state management.

### How It Works

1.  **Wrap**: When you specify built-in attributes for a Widget, that Widget is wrapped by `FatObj`.
2.  **Lazy Initialization**: `FatObj` internally maintains the state of all built-in attributes (like `margin`, `padding`, etc.), but they default to empty. Only when you explicitly use an attribute is the related component initialized. This ensures that unused features don't bring additional performance overhead.
3.  **Compose**: In the final stage of Widget construction, `FatObj` composes the Widget it wraps with the enabled built-in features (like `Padding`, `Container`, `MixBuiltin`, etc.) into the final Widget tree.

`Margin(MixBuiltin(Text))`

## Inner Wrapping Order of Built-in Attributes

`FatObj` wraps built-in attributes in a fixed order. This order determines the structure of the final widget tree and how attributes interact with each other.

The wrapping order from **inner to outer** is as follows (simplified for common attributes):

1.  **Content** (The widget being wrapped)
2.  `padding`
3.  `border`
4.  `background`
5.  `clip_boundary`
6.  `radius`
7.  `scrollable`
8.  `constrained_box` (clamp)
9.  `margin`
10. `cursor`
11. **Events** (`mix_builtin`: `on_tap`, `on_pointer_move`, etc.)
12. `transform`
13. `opacity`
14. `visibility`
15. `h_align` / `v_align`
16. `anchor` / `global_anchor`

### Key Takeaways

*   **Events include Margin**: Since **Events** wrap **Margin**, the interactive area of a widget includes its margin by default.
*   **Transform affects everything**: `transform` wraps most visual and layout attributes, so rotating a widget rotates its margin, background, and border as well.
*   **Visibility hides everything**: `visibility` is near the outermost layer, so setting it to hidden hides the entire widget including its margin.

### How to Override the Order?

Sometimes the default wrapping order doesn't match your requirements. For example, you might want the click area (`on_tap`) to **exclude** the margin.

Since `FatObj` applies attributes in a fixed order, you can achieve this by manually nesting `FatObj`. You can apply the inner attributes first, and then wrap it with another `FatObj` for the outer attributes.

**Example: Click area excluding margin**

If you simply write:
```rust
@FatObj {
    margin: EdgeInsets::all(20.),
    on_tap: |_| println!("Clicked!"),
    @ { w }
}
```
The structure is `MixBuiltin(Margin(w))`, so clicking the margin triggers the event.

To exclude the margin from the click area, you want the structure `Margin(MixBuiltin(w))`. You can do this by:

```rust
fn_widget! {
    // Outer FatObj handles margin
    @FatObj {
        margin: EdgeInsets::all(20.),
        // Inner FatObj handles the click event
        @FatObj {
            on_tap: |_| println!("Clicked inside content (excluding margin)!"),
            @ { w }
        }
    }
}
```

By nesting `FatObj`, you have full control over the composition order of attributes.

## Common Built-in Attributes

Built-in attributes are mainly divided into two categories: **Properties** and **Events**.

### 1. Properties

These attributes are used to control the appearance and layout of Widgets.

*   **Layout**:
    *   `margin`: Sets outer margin.
    *   `padding`: Sets inner padding.
    *   `h_align` / `v_align`: Sets horizontal/vertical alignment.
    *   `anchor`: Used for absolute positioning in `Stack` layout.
    *   `global_anchor_x` / `global_anchor_y`: Used for positioning relative to the global window.
    *   `clamp`: Forces constraints on the Widget's size range (Layout Constraints).
    *   `box_fit`: Controls how child elements fit into container space (like fill, contain, etc.).
    *   `scrollable`: Controls the Widget's scrolling behavior (X-axis, Y-axis, or both).

*   **Visual**:
    *   `background`: Sets background (color or image).
    *   `foreground`: Sets foreground (usually overlays on top of content).
    *   `border`: Sets border.
    *   `radius`: Sets border radius.
    *   `opacity`: Sets opacity.
    *   `visible`: Controls visibility.
    *   `transform`: Applies graphic transformations (translation, rotation, scaling).
    *   `cursor`: Sets cursor style when hovering.
    *   `backdrop_filter`: Applies background filter effects (like blur).
    *   `clip_boundary`: Whether to clip content beyond boundaries.
    *   `painting_style`: Sets painting style (fill or stroke).

*   **Text** (usually inherited by child nodes):
    *   `text_style`: Sets font style.
    *   `text_align`: Sets text alignment.
    *   `text_line_height`: Sets line height.
    *   `font_size`: Sets font size.
    *   `font_face`: Sets font family.

*   **Other**:
    *   `keep_alive`: Keeps Widget state even when removed from view.
    *   `tooltips`: Sets tooltip text.
    *   `disabled`: Disables interaction for Widget and its children.
    *   `class`: Applies style classes.

### 2. Events

These attributes are used to handle user interactions. All event callbacks receive an event object.

*   **Pointer Events**:
    *   `on_pointer_down`: Triggered when a pointer (mouse button, touch contact, pen) is pressed.
    *   `on_pointer_move`: Triggered when a pointer moves.
    *   `on_pointer_up`: Triggered when a pointer is released.
    *   `on_pointer_cancel`: Triggered when a pointer event is cancelled (e.g., touch interruption).
    *   `on_pointer_enter`: Triggered when a pointer enters the widget's area.
    *   `on_pointer_leave`: Triggered when a pointer leaves the widget's area.
    *   `on_tap`: Triggered on a click or tap (press and release sequence).
    *   `on_tap_capture`: Capture phase version of `on_tap`.
    *   `on_double_tap`: Triggered on a double click/tap.
    *   `on_triple_tap`: Triggered on a triple click/tap.
    *   `on_x_times_tap`: Triggered on a specific number of taps.

*   **Wheel Events**:
    *   `on_wheel`: Triggered when the mouse wheel is scrolled.
    *   `on_wheel_capture`: Capture phase version of `on_wheel`.
    *   `on_wheel_changed`: Triggered when the wheel delta changes.

*   **Keyboard Events**:
    *   `on_key_down`: Triggered when a key is pressed.
    *   `on_key_down_capture`: Capture phase version of `on_key_down`.
    *   `on_key_up`: Triggered when a key is released.
    *   `on_key_up_capture`: Capture phase version of `on_key_up`.

*   **Focus Events**:
    *   `on_focus`: Triggered when the widget gains focus.
    *   `on_blur`: Triggered when the widget loses focus.
    *   `on_focus_in`: Triggered when the widget or one of its descendants gains focus (bubbles).
    *   `on_focus_out`: Triggered when the widget or one of its descendants loses focus (bubbles).

*   **Lifecycle Events**:
    *   `on_mounted`: Triggered when the widget is mounted to the widget tree.
    *   `on_performed_layout`: Triggered after the widget has been laid out.
    *   `on_disposed`: Triggered when the widget is removed from the widget tree.

*   **IME Events**:
    *   `on_ime_pre_edit`: Triggered during IME pre-edit (e.g., composing text).
    *   `on_chars`: Triggered when text characters are received.

## Usage Examples

### FatObj Usage

When you want to add built-in attributes to an existing Widget, you can use the `@ FatObj { ... }` syntax directly, which is more concise and readable:

```rust
use ribir::prelude::*;

fn simple_card(w: Widget<'static>) -> Widget<'static> {
    fn_widget! {
        // Wrap the widget with FatObj to add built-in attributes
        @FatObj {
            margin: EdgeInsets::all(10.),
            padding: EdgeInsets::symmetrical(10., 5.),
            h_align: HAlign::Center,
            background: Color::from_u32(0xFFEEAA00),
            border: Border::all(BorderSide::new(2., Color::BLACK.into())),
            radius: Radius::all(4.),
            on_tap: |_: &mut PointerEvent| println!("Card Tapped!"),
            cursor: CursorIcon::Pointer,
            // The child widget
            @ { w }
        }
    }.into_widget()
}
```


1. **`@ FatObj { ... }`** (most recommended): Directly wrap the existing widget with a FatObj declaration. This is the most readable and idiomatic approach.

2. **`FatObj::new(w)` then `@(w) { ... }`**: Create a FatObj variable first, then add attributes to it.

The first approach (`@ FatObj { ... }`) is generally preferred for its clarity and conciseness when you need to enhance an existing widget with built-in attributes.

### Declare Widget wrap with FatObj

Widgets defined with the `#[derive(Declare)]` macro are automatically wrapped with `FatObj` when declared using the `@` syntax in DSL, allowing them to use built-in attributes seamlessly.

The `#[derive(Declare)]` macro generates a builder that implements the `Declare` trait with a `FatObj<()>` field. When you use the widget in the DSL with `@`, it's automatically wrapped with `FatObj` to provide access to built-in attributes.

Mostly, the Widgets defined with the `#[derive(Declare)]` macro, so normally you can use the `@` syntax to declare a widget with built-in attributes.
```rust
use ribir::prelude::*;

fn simple_card_traditional() -> Widget<'static> {
    fn_widget! {
        @Text {
            text: "Hello, Ribir!",
            // Built-in attributes: Layout
            margin: EdgeInsets::all(10.),
            padding: EdgeInsets::symmetrical(10., 5.),
            h_align: HAlign::Center,

            // Built-in attributes: Visual
            background: Color::from_u32(0xFFEEAA00),
            border: Border::all(BorderSide::new(2., Color::BLACK.into())),
            radius: Radius::all(4.),

            // Built-in attributes: Interaction
            on_tap: |_: &mut PointerEvent| println!("Card Tapped!"),
            cursor: CursorIcon::Pointer,
        }
    }
}
```


### Simple Declare Alternative

For widgets that don't need built-in attributes or reactive field bindings, you can use `#[simple_declare]`:

```rust
use ribir::prelude::*;

#[simple_declare]
pub struct SimpleWidget {
    value: i32,
}

impl Compose for SimpleWidget {
    fn compose(this: impl StateWriter<Value = Self>) -> Widget<'static> {
        fn_widget! {
            @Text { text: $read(this).value.to_string() }
        }
    }
}
```

The `#[simple_declare]` attribute creates a simpler declarer without `FatObj` wrapping and without reactive bindings for fields.

## Accessing Property Writers

Built-in attributes (like `opacity`, `background`, `margin`) are properties of the `FatObj` wrapper. In a declarative UI, you typically bind these properties to state during creation. However, if you need to modify them dynamically from code (e.g., inside an event handler), you can do so if you have access to the widget's state.

When you use `Stateful` to manage a widget, you can access its write guard to modify properties.

### Example: Modifying Opacity Dynamically

```rust
use ribir::prelude::*;

fn dynamic_opacity_example() -> Widget<'static> {
    fn_widget! {
        // Create a Stateful widget
        let mut w = @Text {
            text: "Click me to fade!",
            text_style: TypographyTheme::of(BuildCtx::get()).display_large.text.clone(),
            opacity: 1.0, // Initial opacity
        };

        // Modify the widget's properties in an event handler
        // 1. Get write access to the widget (FatObj)
        // 2. Call the property method (e.g., .opacity()) to get the property's writer
        // 3. Get write access to the property value
        @(w) {
            on_tap: move |_| {
                let mut opacity = $write(w.opacity());
                if *opacity > 0.5 {
                    *opacity = 0.5;
                } else {
                    *opacity = 1.0;
                }
            }
        }
    }.into_widget()
}
```

In this example:
1.  `w.opacity()` returns a `StateWriter` for the opacity property.
2.  `$write(...)` on that writer gives mutable access to the actual `f32` value.

This pattern applies to all built-in attributes (e.g., `background()`, `margin()`, etc.). Note that you must use the method (e.g., `opacity()`) rather than accessing the field directly, as the fields are private.

## Summary

`FatObj` is the key to Ribir's flexibility. It allows any Widget to have rich common capabilities while keeping the core Widget definition concise. Through built-in attributes, you can quickly build beautiful and interactive UIs without repeatedly implementing these basic features for each Widget.