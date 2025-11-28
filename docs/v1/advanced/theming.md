# Theming

Ribir provides a powerful and flexible theming system designed to help you build consistent and beautiful UIs. The system is heavily inspired by Material Design principles but is fully customizable to fit any design language.

At its core, a `Theme` in Ribir is a collection of:
- **Palette**: A comprehensive color system.
- **Typography**: A set of semantic text styles.
- **Icons**: A registry for application icons.
- **Classes**: A powerful mechanism to separate style logic from widget structure.

## Accessing the Theme

The current theme is always available in the build context. You can access it using `Theme::of(ctx)`.

```rust
use ribir::prelude::*;

fn example() -> Widget<'static> {
    fn_widget! {
        let palette = Palette::of(BuildCtx::get());
        @Container {
            size: Size::new(100., 100.),
            background: palette.primary(), // Accessing the primary color
        }
    }.into_widget()
}
```

## 1. Palette (Colors)

The `Palette` struct defines the color scheme for your application. It supports both light and dark modes via the `Brightness` enum.

Ribir's palette uses semantic naming (like `primary`, `secondary`, `surface`, `error`) rather than descriptive naming (like `red`, `blue`). This ensures that your UI adapts correctly when the theme changes (e.g., switching from light to dark mode).

### Using Colors

```rust
use ribir::prelude::*;

fn color_example() -> Widget<'static> {
    fn_widget! {
        let palette = Palette::of(BuildCtx::get());
        
        @Text {
            text: "Error Message",
            style: TextStyle {
                color: palette.error(), // Semantic color for errors
                ..TypographyTheme::of(BuildCtx::get()).body_medium.text.clone()
            }
        }
    }.into_widget()
}
```

## 2. Typography (Text Styles)

Ribir's typography system organizes text styles into semantic categories like `Display`, `Headline`, `Title`, `Label`, and `Body`. Each category has `Large`, `Medium`, and `Small` variations.

This structure allows you to define a consistent typographic hierarchy across your application.

### Accessing Text Styles

You can access the typography theme using `TypographyTheme::of(ctx)`.

```rust
use ribir::prelude::*;

fn text_style_example() -> Widget<'static> {
    fn_widget! {
        let typography = TypographyTheme::of(BuildCtx::get());
        
        @Column {
            @Text {
                text: "Main Title",
                text_style: typography.display_large.text.clone(),
            }
            @Text {
                text: "Subtitle",
                text_style: typography.title_medium.text.clone(),
            }
            @Text {
                text: "Body text goes here...",
                text_style: typography.body_medium.text.clone(),
            }
        }
    }.into_widget()
}
```

## 3. Icons

Ribir provides two ways to manage icons:

1. **`IconTheme`**: Part of the main `Theme` struct, using numerical IDs.
2. **`named_svgs`**: A global registry using string keys (Recommended for most cases).

### Using Named SVGs

You can register SVGs globally and retrieve them by name.

```rust
use ribir::prelude::*;

fn register_icons() {
    // Register an icon
    named_svgs::register("my_icon", |color| {
        // Return an SVG string, optionally using the color
        format!(r#"<svg ... fill="{:?}" ...></svg>"#, color)
    });
}

fn icon_example() -> Widget<'static> {
    fn_widget! {
        // Use the registered icon
        @Svgs::from_name("my_icon")
    }.into_widget()
}
```

## 4. Classes (Styles)

One of Ribir's most powerful features is its `Class` system. A `Class` in Ribir is not just a collection of properties (like CSS classes); it is a **function that transforms a widget**.

This allows a class to:
- Set properties (e.g., color, padding).
- Wrap the widget (e.g., add a border or background container).
- Add behavior (e.g., event listeners).

### Defining Styles with `style_class!`

For simple property updates, Ribir provides the `style_class!` macro.

```rust
use ribir::prelude::*;

// Define a style class
let my_button_style = style_class! {
    // Apply these properties to the widget
    .background(Color::BLUE)
    .padding(EdgeInsets::all(10.))
    .radius(10.)
};
```

### Applying Styles

You can apply styles to a widget using the `class` property (if exposed) or by manually applying the transformation.

Ribir's built-in widgets interact with the `Theme`'s `classes` map to automatically apply styles based on `ClassName`.

```rust
use ribir::prelude::*;

fn styled_widget() -> Widget<'static> {
    fn_widget! {
        @Text {
            text: "Styled Text",
            // Manually applying a style class
            class: style_class! {
                .foreground(Color::RED)
                .text_line_height(2.0)
            }
        }
    }.into_widget()
}
```

## 5. Theming Your Own Widgets

You can make your own custom widgets themeable by defining "Theming Classes". This allows users of your widget to customize its appearance via the `Theme` without modifying the widget's code.

### Step 1: Define Class Names

Use the `class_names!` macro to define globally unique keys for your widget's styles.

```rust
use ribir::prelude::*;

class_names! {
    /// The default class for MyCard
    MY_CARD,
    /// The class for MyCard's title
    MY_CARD_TITLE
}
```

### Step 2: Use Classes in Your Widget

In your widget's `compose` method, use these class names to apply styles from the current theme.

```rust
#[derive(Declare)]
pub struct MyCard;

impl Compose for MyCard {
    fn compose(this: impl StateWriter<Value = Self>) -> Widget<'static> {
        fn_widget! {
            @Container {
                // Apply the main card style from the theme
                class: MY_CARD,
                @Text {
                    text: "Card Title",
                    // Apply the title style from the theme
                    class: MY_CARD_TITLE,
                }
            }
        }.into_widget()
    }
}
```

### Step 3: Provide Default Styles

When setting up your application's theme, provide default implementations for these classes.

```rust
fn main() {
    let mut theme = Theme::default();
    
    // Register the default styles for MyCard
    theme.classes.insert(MY_CARD, style_class! {
        .background(Color::WHITE)
        .padding(EdgeInsets::all(16.))
        .radius(8.)
        .shadow(Shadow::default())
    });

    theme.classes.insert(MY_CARD_TITLE, style_class! {
        .text_style(TypographyTheme::of(BuildCtx::get()).title_large.text.clone())
        .foreground(Color::BLACK)
    });

    App::run(fn_widget! {
        @ThemeProvider {
            theme: std::rc::Rc::new(theme),
            @MyCard {}
        }
    });
}
```

## Customizing the Theme

You can create a custom theme and provide it to your application. This is usually done at the root of your app.

```rust
use ribir::prelude::*;

fn main() {
    // Create a custom palette
    let palette = Palette {
        brightness: Brightness::Light,
        primary: Color::from_rgb(0, 120, 215), // Custom blue
        ..Palette::light() // Fallback to default light palette
    };

    // Create a custom theme
    let theme = Theme {
        palette,
        ..Theme::default()
    };

    // Run the app with the custom theme
    App::run(fn_widget! {
        @ThemeProvider {
            theme: std::rc::Rc::new(theme),
            @Text { text: "I use the custom theme!" }
        }
    });
}
```

### Overriding Parts of the Theme

You can also nest `ThemeProvider` widgets to override the theme for a specific section of the UI.

```rust
use ribir::prelude::*;

fn section_override() -> Widget<'static> {
    fn_widget! {
        @Column {
            @Text { text: "Normal Theme" }
            
            @ThemeProvider {
                theme: {
                    let mut t = Theme::of(BuildCtx::get()).as_ref().clone();
                    t.palette.primary = Color::RED; // Override primary color
                    std::rc::Rc::new(t)
                },
                @Container {
                    // This container and its children will use the red primary color
                    background: Palette::of(BuildCtx::get()).primary(), 
                }
            }
        }
    }.into_widget()
}
```
