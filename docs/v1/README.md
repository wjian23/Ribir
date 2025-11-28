# Ribir Documentation v1

Welcome to the Ribir v1 documentation. Ribir is a non-intrusive GUI framework for Rust that allows you to build multi-platform applications from a single codebase.

## Documentation Structure

This documentation is organized into the following sections:

### 1. Getting Started
Everything you need to set up your environment and build your first Ribir application.
- **[Installation](./getting_started/installation.md)**: Setup Rust and Ribir dependencies.
- **[Hello World](./getting_started/hello_world.md)**: Your first Ribir application.
- **[Architecture Overview](./getting_started/architecture_overview.md)**: High-level understanding of how Ribir works.

### 2. Core Concepts
Deep dive into Ribir's fundamental principles.
- **[Declarative UI](./core_concepts/declarative_ui.md)**: Introduction to DSL syntax, using the `fn_widget!` macro, and how to statically compose defined components.
- **[Built-in Attributes & FatObj](./core_concepts/built_in_attributes_and_fat_obj.md)**: Explains how to use built-in attributes and how `FatObj` provides common capabilities.
- **[State Management](./core_concepts/state_management.md)**: Explains `Stateful` objects and how to read/write state and create reactive bindings in views using `$read`, `$write`, `pipe!`, and `$watch`.
- **[Data Sharing & Events](./core_concepts/data_sharing_and_events.md)**: How to use `Provider` to share data top-down through the component tree, and use `Custom Event` for bottom-up event bubbling.
- **[Widget System](./core_concepts/widgets_composition.md)**: Explains the differences between `Render`, `Compose`, and `ComposeChild`, the `SingleChild` vs `MultiChild` structure, and the principles of the Widget Declare phase.
- **[Layout System](./core_concepts/layout.md)**: Layout constraint propagation mechanism and how the `clamp` attribute can intervene in layout sizing.

### 3. Advanced Topics
For developers who want to extend Ribir or understand deeper mechanics.
- **[Custom Widgets](./advanced/custom_widgets.md)**: Creating your own reusable widgets.
- **[Widgets](./advanced/widgets.md)**: introduce the Widget component that the framework has supported.
- **[Animations](./advanced/animations.md)**: Adding motion to your UI.
- **[Theming](./advanced/theming.md)**: Customizing the look and feel, including key concepts like the class system, common color providers, theme providers, and theming classes.

---

## Contributing
Ribir is open source! If you find issues in the documentation or want to improve it, please check our [Contributing Guide](../../CONTRIBUTING.md).
