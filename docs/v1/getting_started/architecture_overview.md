# Architecture Overview

Ribir is designed to be modular and non-intrusive. Here is a high-level view of how the pieces fit together.

## Core Layers

1.  **Widgets (`ribir_widgets`)**: The building blocks of your UI (Buttons, Text, Layouts). These are high-level abstractions composed of other widgets or render objects.
2.  **Core (`ribir_core`)**: The engine that drives the framework. It handles:
    *   **Widget Tree**: Manages the hierarchy of widgets.
    *   **State Management**: Tracks data changes and schedules updates.
    *   **Events**: Propagates user interactions (clicks, keys) through the tree.
    *   **Layout**: Calculates the size and position of every element.
3.  **Painter (`ribir_painter`)**: A platform-independent 2D drawing library. It generates drawing commands (paths, brushes, transforms).
4.  **GPU Backend (`ribir_gpu`)**: Takes the commands from the painter and renders them efficiently using the GPU (via `wgpu`).

## The Build Process

When you run a Ribir app:

1.  **Declaration**: You define the UI using `fn_widget!`.
2.  **Build**: Ribir constructs a "Widget Tree" based on your declaration.
3.  **Layout**: The tree is traversed to determine sizes and positions.
4.  **Paint**: The visible widgets issue drawing commands to the Painter.
5.  **Render**: The GPU backend executes these commands to the screen.

## Reactive Updates

Ribir uses a **Point-to-Point** update system.

*   When you modify data wrapped in `Stateful` (using `$write`), Ribir knows exactly which parts of the UI depend on that data.
*   It marks only those specific widgets as "dirty".
*   In the next frame, only the dirty widgets (and their necessary children/parents) are re-built, re-laid out, or re-painted.
*   This avoids re-rendering the entire tree, ensuring high performance.
