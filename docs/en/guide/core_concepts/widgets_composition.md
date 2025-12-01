# Compose System

Ribir's widget system is built on three core traits: `Render`, `Compose`, and `ComposeChild`. Understanding these traits is key to creating custom widgets and understanding how Ribir constructs the UI tree.

## 1. Render vs Compose

### The `Render` Trait

`Render` is the low-level interface for widgets that actually draw something on the screen or manage layout directly. If a widget is a "leaf" node that paints pixels (like `Text` or `Rectangle`) or a container that calculates the positions of its children (like `Row` or `Column`), it implements `Render`.

Key responsibilities of `Render`:
- **Layout**: Calculating its own size and the position of its children (`perform_layout`).
- **Painting**: Drawing content to the canvas (`paint`).
- **Hit Testing**: Determining if a point interacts with the widget (`hit_test`).

```rust
// Simplified concept of a Render widget
impl Render for MyCustomPainter {
    fn perform_layout(&self, clamp: BoxClamp, ctx: &mut LayoutCtx) -> Size {
        // Calculate size...
        Size::new(100., 100.)
    }

    fn paint(&self, ctx: &mut PaintingCtx) {
        // Draw something...
        let rect = Rect::from_size(ctx.box_rect().unwrap().size);
        ctx.painter().rect(&rect).fill();
    }
}
```

### The `Compose` Trait

`Compose` is for high-level widgets that are built by combining other widgets. They don't draw anything themselves; they just expand into a tree of other widgets. This is similar to a "Component" in React or Vue.

Most application-level widgets (like a `UserProfile` or `LoginForm`) implement `Compose`.

```rust
#[derive(Declare)]
pub struct WelcomeCard;

impl Compose for WelcomeCard {
    fn compose(this: impl StateWriter<Value = Self>) -> Widget<'static> {
        fn_widget! {
            @Column {
                @Text { text: "Welcome!" }
                @Button { @Text { text: "Click me" } }
            }
        }.into_widget()
    }
}
```

## 2. ComposeChild & Child Structure

Ribir uses a strictly typed parent-child relationship system. Not all widgets can accept children, and some accept specific types of children.

### The `ComposeChild` Trait

`ComposeChild` is a variation of `Compose` for widgets that wrap or modify a child widget. It defines how the parent and child are combined.

```rust
pub trait ComposeChild<'c>: Sized {
    type Child;
    fn compose_child(this: impl StateWriter<Value = Self>, child: Self::Child) -> Widget<'static>;
}
```

### SingleChild vs MultiChild

`SingleChild` and `MultiChild` traits are used to identify the type of widget that accepts the number of children, usually used for layout.

- **SingleChild**: Widgets that accept exactly one child.
  - Example: `SizedBox`, `Padding`, `Container`.
  - In DSL: `@Container { @Text { ... } }`

- **MultiChild**: Widgets that accept a list of children.
  - Example: `Row`, `Column`, `Stack`.
  - In DSL:
    ```rust
    @Column {
        @Text { ... }
        @Text { ... }
    }
    ```

Usually you just need to specify it when defining the type:
```rust
#[derive(SingleChild, Declare)]
pub struct Container;
```

```rust
#[derive(MultiChild, Declare)]
pub struct Row;
```


## 3. Widget Usage

When you use the `fn_widget!` macro and the `@WidgetName { ... }` syntax, you are in the **Declare Phase**.

**Important**: The `@` operator and the `$read`, `$write` operators are **DSL-specific** and only work within macros that support the Ribir DSL syntax, such as `fn_widget!` and `rdl!`. These operators are not valid Rust syntax outside of these macros and will cause compilation errors if used in regular Rust code or nested within third-party macros.

1. **Builder Pattern**: The syntax `@Text { text: "Hi" }` roughly translates to a builder pattern that constructs the widget.
2. **State Creation**: If you use `Stateful` widgets or `pipe!`, the framework sets up the reactive graph during this phase.
3. **FatObj**: Ribir wraps widgets in a `FatObj` to provide built-in attributes (like `margin`, `background`, `on_tap`) to every widget, even if the underlying widget (like `Text`) doesn't explicitly define them.

```rust
// In the macro:
@Text {
    text: "Hello",
    margin: EdgeInsets::all(10.), // Built-in attribute
}

// Conceptually desugars to something like:
let txt = Text { text: "Hello" };
let fat_obj = FatObj::new(txt).with_margin(EdgeInsets::all(10.));
```

This composition happens statically where possible, but the `Compose` logic runs dynamically when the widget is actually needed in the tree.