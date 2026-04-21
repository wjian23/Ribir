//! # Markdown Editor
//!
//! A markdown editor with split layout:
//! - Left: TextArea for editing markdown source
//! - Right: Live preview with ReuseScope
//!
//! Architecture:
//! - App: owns doc, manages file path, title updates, file operations
//! - Editor: receives doc reference, handles editing only (no file ops)
//! - Core: MarkdownCoreDoc with command queue and version tracking

pub mod components;
pub mod core_doc;
pub mod image_loader;
pub mod incremental_parser;
pub mod parser;
pub mod syntax_highlight;
pub mod undo_redo;

use std::{cell::RefCell, path::PathBuf, rc::Rc};

pub use components::{MarkdownNode, MixedInline, render_block};
pub use core_doc::MarkdownCoreDoc;
use ribir::prelude::*;

/// Unique ID for each markdown node
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct NodeId(pub usize);

/// A node in the markdown document with unique ID
#[derive(Clone)]
pub struct DocumentNode {
  pub id: NodeId,
  pub node: MarkdownNode,
  /// Actual byte range in the source text
  pub source_range: std::ops::Range<usize>,
}

impl std::fmt::Debug for DocumentNode {
  fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
    f.debug_struct("DocumentNode")
      .field("id", &self.id)
      .field("range", &self.source_range)
      .field("node", &self.node)
      .finish()
  }
}

/// Global ID allocator
static NEXT_NODE_ID: std::sync::atomic::AtomicUsize = std::sync::atomic::AtomicUsize::new(0);

fn allocate_node_id() -> NodeId {
  let id = NEXT_NODE_ID.fetch_add(1, std::sync::atomic::Ordering::Relaxed);
  NodeId(id)
}

/// Complete markdown document
#[derive(Clone)]
pub struct MarkdownDocument {
  pub nodes: Vec<DocumentNode>,
}

impl MarkdownDocument {
  /// Parse markdown with full re-parse
  pub fn parse(text: impl Into<String>) -> Self {
    let nodes = parser::parse_full(&text.into());
    Self { nodes }
  }

  /// Get all nodes
  pub fn nodes(&self) -> &[DocumentNode] { &self.nodes }

  /// Insert a node at the specified index
  pub fn insert_node(&mut self, index: usize, node: MarkdownNode) {
    let id = NodeId(self.nodes.len());
    let index = index.min(self.nodes.len());
    self
      .nodes
      .insert(index, DocumentNode { id, node, source_range: 0..0 });
  }

  /// Remove a node at the specified index
  pub fn remove_node(&mut self, index: usize) -> Option<DocumentNode> {
    if index < self.nodes.len() { Some(self.nodes.remove(index)) } else { None }
  }

  /// Get node count
  pub fn len(&self) -> usize { self.nodes.len() }

  /// Check if empty
  pub fn is_empty(&self) -> bool { self.nodes.is_empty() }
}

/// Markdown editor widget - only handles editing, file ops managed by App layer
pub struct MarkdownEditor {
  doc: Stateful<MarkdownCoreDoc>,
}

impl MarkdownEditor {
  /// Create editor with doc from app layer
  pub fn new(doc: Stateful<MarkdownCoreDoc>) -> Self { Self { doc } }

  /// Get doc for app layer to access
  pub fn doc(&self) -> Stateful<MarkdownCoreDoc> { self.doc.clone_writer() }

  /// Render the editor widget
  pub fn into_widget(self) -> Widget<'static> {
    let doc = self.doc;
    let initial_source = doc.read().source().to_string();

    fn_widget! {
      let mut text_area = @TextArea {};
      $write(text_area).set_text(&initial_source);

      // Track if we're doing undo/redo (to avoid recording it)
      let is_undo_redo = Rc::new(RefCell::new(false));

      // Watch TextArea text changes → push to doc command queue
      let is_undo_redo_for_input = is_undo_redo.clone();
      let input_watch = watch!($read(text_area).text().clone())
        .distinct_until_changed()
        .skip(1) // Skip initial value
        .subscribe(move |text| {
          if !*is_undo_redo_for_input.borrow() {
            $write(doc).push_set_text(text.to_string());
          }
        });

      // Debounced: consume SetText after 200ms
      let doc_for_debounce = doc.clone_writer();

      let debounce_watch = watch!($read(text_area).text().clone())
        .debounce(std::time::Duration::from_millis(200))
        .subscribe(move |_| {
          // Consume pending SetText
          doc_for_debounce.write().consume_set_text();
        });

      @Row {
        on_disposed: move |_| {
          input_watch.unsubscribe();
          debounce_watch.unsubscribe();
        },
        @Container {
          width: 400.0,

          @FatObj {
            @(text_area) {
              clamp: BoxClamp::EXPAND_BOTH,
              on_key_down: {
                let doc_for_key = doc.clone_writer();
                let text_area_for_key = text_area.clone_writer();
                let is_undo_redo_for_key = is_undo_redo.clone();
                move |e| {
                  if !e.with_command_key() {
                    return;
                  }

                  match e.key_code() {
                    PhysicalKey::Code(KeyCode::KeyZ) => {
                      // Undo
                      *is_undo_redo_for_key.borrow_mut() = true;
                      let op = doc_for_key.write().undo();
                      if let Some(edit_op) = op {
                        let text = doc_for_key.read().source().to_string();
                        text_area_for_key.write().set_text(&text);
                        text_area_for_key.write().select(edit_op.position, edit_op.position);
                      }
                      *is_undo_redo_for_key.borrow_mut() = false;
                    }
                    PhysicalKey::Code(KeyCode::KeyY) => {
                      // Redo
                      *is_undo_redo_for_key.borrow_mut() = true;
                      let op = doc_for_key.write().redo();
                      if let Some(edit_op) = op {
                        let text = doc_for_key.read().source().to_string();
                        let caret = edit_op.position + edit_op.new_text.len();
                        text_area_for_key.write().set_text(&text);
                        text_area_for_key.write().select(caret, caret);
                      }
                      *is_undo_redo_for_key.borrow_mut() = false;
                    }
                    // Cmd+S not handled here - App layer handles file operations
                    _ => {}
                  }
                }
              }
            }
          }
        }

        @Container {
          width: 2.0,
          background: Color::from_u32(0xFFCCCCCC),
        }

        @Expanded {
          @ReuseScope {
              @ScrollableWidget {
                scrollable: Scrollable::Y,
                @Flex {
                  direction: Direction::Vertical,
                  item_gap: 12.0,
                  text_overflow: TextOverflow::AutoWrap,
                  @ {
                    pipe!({
                    $read(doc).document_nodes().clone()
                  }).map(|nodes: Vec<DocumentNode>| {
                    nodes.iter().map(|doc_node| {
                      let id = doc_node.id.0;
                      let node = doc_node.node.clone();
                      @Reuse {
                        reuse: ReuseKey::local(id),
                        @ {
                          fn_widget! {
                            render_block(node)
                          }
                        }
                      }
                    }).collect::<Vec<_>>()
                  })
                }
              }
            }
          }
        }
      }
    }
    .into_widget()
  }
}

/// App-level component: owns doc, manages file operations, window title
pub struct MarkdownApp {
  doc: Stateful<MarkdownCoreDoc>,
}

impl MarkdownApp {
  pub fn new(source: impl Into<String>) -> Self {
    Self { doc: Stateful::new(MarkdownCoreDoc::new(source.into())) }
  }

  pub fn widget(this: impl StateWriter<Value = Self>) -> Widget<'static> {
    // App-level state
    let file_path = Stateful::new(Option::<PathBuf>::None);
    let editor_seed = Stateful::new(0u64); // To recreate editor on file open/new
     let saved_version = Stateful::new(1u64); // Version at last save

    fn_widget! {

      // Helper: update window title based on file path and dirty state
      fn update_title(wnd: &Window, path: &Option<PathBuf>, is_dirty: bool) {
        let title = match path {
          Some(p) => {
            let name = p.file_name()
              .map(|n| n.to_string_lossy().to_string())
              .unwrap_or_else(|| p.to_string_lossy().to_string());
            if is_dirty { format!("{}* - Markdown Editor", name) }
            else { format!("{} - Markdown Editor", name) }
          }
          None => {
           "Untitled* - Markdown Editor".to_string()
          }
        };
        wnd.set_title(&title);
      }

      @Container {
        on_key_down: {
          move |e: &mut KeyboardEvent| {
            if !e.with_command_key() {
              return;
            }

            // Cmd+Shift+S: Save As
            if e.with_shift_key() && *e.key_code() == PhysicalKey::Code(KeyCode::KeyS) {
              if let Some(path) = rfd::FileDialog::new()
                .add_filter("Markdown", &["md", "markdown"])
                .add_filter("All Files", &["*"])
                .save_file()
              {
                // Save to new path
                let version = $read(this).doc.write().save(&path.to_string_lossy()).unwrap_or(0);
                *$write(saved_version) = version;
                *$write(file_path) = Some(path.clone());
                update_title(&e.window(), &Some(path), false);
              }
              return;
            }

            match e.key_code() {
              PhysicalKey::Code(KeyCode::KeyO) => {
                // Open file
                if let Some(path) = rfd::FileDialog::new()
                  .add_filter("Markdown", &["md", "markdown"])
                  .add_filter("All Files", &["*"])
                  .pick_file()
                {
                  if let Ok(content) = std::fs::read_to_string(&path) {
                    // Reset doc with new content
                    *$write(editor_seed) += 1;
                    $write(this).doc = Stateful::new(MarkdownCoreDoc::new(content));
                    *$write(file_path) = Some(path.clone());
                    update_title(&e.window(), &Some(path), false);
                  }
                }
              }
              PhysicalKey::Code(KeyCode::KeyN) => {
                // New file
                *$write(editor_seed) += 1;
                $write(this).doc = Stateful::new(MarkdownCoreDoc::new(""));
                *$write(file_path) = None;
                update_title(&e.window(), &None, false);
              }
              PhysicalKey::Code(KeyCode::KeyS) => {
                // Cmd+S: Save
                let path_opt = $read(file_path).clone();
                let path_to_save = if let Some(p) = path_opt {
                  Some(p)
                } else {
                  // No path - show Save As dialog
                  rfd::FileDialog::new()
                    .add_filter("Markdown", &["md", "markdown"])
                    .add_filter("All Files", &["*"])
                    .set_file_name("document.md")
                    .save_file()
                };

                if let Some(path) = path_to_save {
                  let version = $read(this).doc.write().save(&path.to_string_lossy()).unwrap_or(0);
                  *$write(saved_version) = version;
                  if $read(file_path).is_none() {
                    *$write(file_path) = Some(path.clone());
                  }
                  update_title(&e.window(), &Some(path), false);
                }
              }
              _ => {}
            }
          }
        },
        @ {
          // Create editor, recreate when seed changes (new file/open)
          pipe!($read(editor_seed);).map(move |_| {
            fn_widget! {
              let doc = $read(this).doc.clone_writer();
              let wnd = BuildCtx::get().window();
              *$write(saved_version) = doc.read().version();
              let unsub = watch!(*$read(saved_version) != $read(doc).version())
                .distinct_until_changed()
                .subscribe(move |is_dirty| {
                  update_title(&wnd, &*$read(file_path), is_dirty);
                });
              @FatObj {
                on_disposed: move |_| unsub.unsubscribe(),
                @ { MarkdownEditor::new(doc).into_widget() }
              }
            }
          })
        }
      }
    }
    .into_widget()
  }
}

#[cfg_attr(target_arch = "wasm32", wasm_bindgen::prelude::wasm_bindgen)]
pub fn run() {
  #[cfg(target_arch = "wasm32")]
  std::panic::set_hook(Box::new(console_error_panic_hook::hook));

  use ribir::material;

  App::run(|| {
    let app = Stateful::new(MarkdownApp::new(SAMPLE_MARKDOWN));

    fn_widget! {
      @ {
        MarkdownApp::widget(app)
      }
    }
    .into_widget()
  })
  .with_app_theme(material::purple::light)
  .with_size(Size::new(1200., 800.))
  .with_title("Markdown Editor");
}

const SAMPLE_MARKDOWN: &str = r#"
# Welcome to Markdown Reader

This is a **markdown reader and editor** built with [Ribir](https://github.com/RibirX/ribir), a modern reactive UI framework for Rust.

---

## About Ribir

Ribir is a declarative UI framework that follows a **data-centric, non-intrusive** approach:

- **Data-Centric**: UI is a projection of data. Focus on designing data structures first.
- **Non-Intrusive**: Data doesn't need to know about UI. We wrap data in `Stateful<T>` to make it reactive.
- **Precise Updates**: Only the specific part of the UI that depends on a changed field re-renders.

This editor demonstrates Ribir's reactive architecture with live preview, undo/redo, and file operations.

## Keyboard Shortcuts

| Shortcut | Action |
|----------|--------|
| `Cmd/Ctrl + O` | Open markdown file |
| `Cmd/Ctrl + S` | Save file |
| `Cmd/Ctrl + Shift + S` | Save As... |
| `Cmd/Ctrl + N` | New document |
| `Cmd/Ctrl + Z` | Undo |
| `Cmd/Ctrl + Y` | Redo |

## Features

- Live markdown preview
- Syntax highlighting for code blocks
- Undo/Redo support
- File open/save operations
- Split-pane editing

## Code Blocks

Here's an example of Rust code:

```rust
fn main() {
    let name = "Ribir";
    println!("Hello, {}!", name);
}
```

And here's some Python:

```python
def greet(name):
    return f"Hello, {name}!"

print(greet("World"))
```

## Images

Here's an example image:

![Sample Image](https://ribir.org/landing-page/polestar_banner.png)

## Links

Check out these useful links:

- [Ribir GitHub](https://github.com/RibirX/ribir)
- [Rust Programming Language](https://www.rust-lang.org)
- [Markdown Guide](https://www.markdownguide.org)

## Try It!

Add your own markdown content here to see it rendered.

### Unordered List

- Item 1
- Item 2
- Item 3

### Ordered List

1. First item
2. Second item
3. Third item

### Todo List

- [ ] Complete the project documentation
- [x] Implement markdown parsing
- [ ] Add unit tests
- [-] Review the code changes
- [x] Fix layout crash

---

*Happy editing with Ribir!*
"#;
