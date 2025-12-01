# 声明式 UI

Ribir 使用基于 Rust 宏的声明式 DSL（领域特定语言）来定义用户界面。这让您可以描述 UI *应该是什么样子*,而不是 *如何* 一步步构建它。

`fn_widget!` 宏是这个 DSL 的核心。

## `fn_widget!` 宏

`fn_widget!` 是编写 Ribir UI 代码的入口点。它将 DSL 语法转换为构建 Widget 树的实际 Rust 代码。

```rust
use ribir::prelude::*;

fn main() {
    App::run(fn_widget! {
        @Text { text: "Hello!" }
    });
}
```

## 使用 `@` 创建 Widget

要实例化一个 Widget，请使用 `@` 符号后跟 Widget 结构名称。属性在花括号 `{}` 内使用标准的 Rust 结构初始化语法 `key: value` 定义。

当 `@` 直接跟随一个类型时，它会调用相应的构建器来构造对象。这个构建器通常由 `#[derive(Declare)]` 宏生成，从而支持内置属性的使用。我们将在 [内置属性和 FatObj](./built_in_attributes_and_fat_obj.md) 部分详细探讨这一机制。

**重要**: `@` 运算符是 **DSL 专用的**，只在支持 Ribir DSL 语法的宏中有效，如 `fn_widget!` 和 `rdl!`。在这些宏之外，该运算符不是有效的 Rust 语法，如果在常规 Rust 代码或第三方宏中使用，将导致编译错误。

```rust
use ribir::prelude::*;

fn example() -> Widget<'static> {
    fn_widget! {
        @Text { text: "I am a Text widget" }
    }.into_widget()
}
```

## 父子组合

Ribir 将 UI 表示为一棵树。您可以通过嵌套来组合它们。支持子项的 Widget 允许您直接在它们的块内声明它们。

```rust
use ribir::prelude::*;

fn composition_example() -> Widget<'static> {
    fn_widget! {
        @Column {
            @Text { text: "Item 1" }
            @Text { text: "Item 2" }
            @Button {
                @Text { text: "Click Me" }
            }
        }
    }.into_widget()
}
```

这里 `Column` 是一个支持多个孩子的 Widget（`MultiChild`，参见 [Widget 系统](./widgets_composition.md)），它允许您直接在它的块内声明子项。

## 复用 Widget（静态组合）

由于 `fn_widget!` 返回一个 `Widget`（或求值为 Widget 的代码），您可以将其赋值给变量或从函数返回，然后在另一个 `fn_widget!` 块中使用。这有助于提高代码的可复用性。

要将已有的 Widget 变量或将一个 Widget 的表达式声明到 DSL 中，请使用 `@ { expression }` 语法。

```rust
use ribir::prelude::*;

fn header() -> Widget<'static> {
    fn_widget! {
        @Text { text: "My App Header" }
    }.into_widget()
}

fn app() -> Widget<'static> {
    let footer = fn_widget! {
        @Text { text: "Footer Content" }
    };

    fn_widget! {
        @Column {
            @ { header() } // 嵌入返回 Widget 的函数
            @Text { text: "Main Content" }
            @ { footer }   // 嵌入 Widget 变量
        }
    }.into_widget()
}
```

## 动态 Widget

Ribir 允许您创建能够在数据变化时自动更新的 Widget。`pipe!` 宏是实现这一功能的关键工具,它会创建一个值流,并可将其转换为 Widget。

要创建动态 Widget,您可以使用 `pipe!` 宏,并通过 `@ { ... }` 语法将其嵌入到 UI 中。

```rust
use ribir::prelude::*;

fn dynamic_widget_example() -> Widget<'static> {
    let count = Stateful::new(0);

    fn_widget! {
        @Column {
            // 这个 Text Widget 将在 `count` 变化时更新
            @ {
                pipe! {
                    let c = *$read(count);
                    @Text { text: format!("Count: {}", c) }
                }
            }
            @Button {
                on_tap: move |_| *$write(count) += 1,
                @Text { text: "Increment" }
            }
        }
    }.into_widget()
}
```

当您在 `pipe!` 中包装一个表达式时，Ribir 会监控该表达式中使用的状态变量（如 `$read(count)`）。当其中任何一个发生变化时，表达式会重新计算，Widget 也会随之更新。

您还可以在管道上使用 `.map()` 在构建 Widget 之前转换数据（推荐）：

```rust
@ {
    pipe!(*$read(count))
        .map(move |c| @Text { text: format!("Count is {}", c) })
}
```

如果您的管道需要根据条件返回不同类型的 Widget,可以使用 `.into_widget()` 将它们统一为单一的 `Widget` 类型：

```rust
@ {
    pipe!(*$read(count)).map(move |c| {
        if c % 2 == 0 {
            @Text { text: "Even" }.into_widget()
        } else {
            @Button { @Text { text: "Odd" } }.into_widget()
        }
    })
}
```