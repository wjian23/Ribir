# Hello World

Let's build your first Ribir application.

## Basic Structure

Open `src/main.rs` in your new project and add the following code:

```rust
use ribir::prelude::*;

fn main() {
    App::run(fn_widget! {
        @Text { text: "Hello, Ribir!" }
    });
}
```

## Running the App

Run the application using cargo:

```bash
cargo run
```

You should see a window displaying "Hello, Ribir!".

## Explanation

1.  **`use ribir::prelude::*;`**: Imports the essential Ribir macros and types.
2.  **`App::run(...)`**: The entry point that starts the Ribir runtime and opens the window.
3.  **`fn_widget! { ... }`**: A macro that defines the widget tree. It allows you to write declarative UI code.
4.  **`@Text { ... }`**: Creates a Text widget. The `@` syntax is used within the DSL to instantiate widgets.

## A Stateful Example: Counter

Here is a slightly more complex example that involves state (a counter):

```rust
use ribir::prelude::*;

fn main() {
    App::run(fn_widget! {
        // Create a stateful piece of data
        let counter = Stateful::new(0);

        @Row {
            // Display the counter value
            // `pipe!` subscribes to changes in `counter`
            @Text { text: pipe!($read(counter).to_string()) }
            
            // A button to increment the counter
            @Button {
                on_tap: move |_| *$write(counter) += 1,
                @Text { text: "Increment" }
            }
        }
    });
}
```

In this example:
*   `Stateful::new(0)` creates a reactive state.
*   `$read(counter)` allows reading the value.
*   `$write(counter)` allows modifying the value.
*   `pipe!` ensures the UI updates whenever `counter` changes.
