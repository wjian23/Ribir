# Markdown Incremental Editor Design

This document defines the refactor plan for the markdown example editor so that:

- The left pane becomes a block-based incremental editor instead of a single `TextArea`.
- The right pane becomes a pure projection of the edited text and only handles display.
- Incremental update logic is extracted as a reusable interface instead of being embedded in `MarkdownCoreDoc`.
- Undo/redo is handled in the text editing layer, not in the markdown projection layer.
- Preview content supports selection and copy through `SelectableArea`, including text and images.
- Both panes expose block layout positions for future scroll synchronization.

The current implementation spans:

- `examples/markdown/src/lib.rs`
- `examples/markdown/src/core_doc.rs`
- `examples/markdown/src/incremental_parser.rs`
- `examples/markdown/src/components/*`
- `widgets/src/input/text_editable.rs`
- `widgets/src/rich_text.rs`
- `widgets/src/selectable_area.rs`

## 1. Current Problems

The current example already has incremental parsing and `Reuse`, but the architecture is still too coarse.

### 1.1 Left Pane Problems

- The left pane is a single `TextArea`.
- Text changes are observed through `watch!($read(text_area).text().clone())`.
- The editor does not own its own edit history model.
- Undo/redo is delegated to `MarkdownCoreDoc`, which makes the markdown projection layer own editing concerns.
- There is no block model, so the left pane cannot do block-local rebuild/layout.
- There is no persistent mapping from text ranges to visual blocks.

### 1.2 Right Pane Problems

- The right pane consumes the entire `Vec<DocumentNode>`.
- `IncrementalParser` returns a full node vector after every edit, even though internally it only reparses a slice.
- The view layer knows how to use `Reuse`, but there is no explicit patch protocol.
- Block identity depends on transient `NodeId` allocation, which is not enough for fine-grained reuse across reparsed windows.
- The preview is not yet consistently wrapped by `SelectableArea`.
- `MixedInline` renders rich text plus image overlays, but it is not modeled as a selectable fragment producer.

### 1.3 Cross-Cutting Problems

- Incremental projection logic is markdown-specific and coupled to `MarkdownCoreDoc`.
- Editing and projection are not separated by an explicit delta protocol.
- There is no shared block location registry for future bidirectional scroll sync.

## 2. Refactor Goals

### 2.1 Primary Goals

1. Extract a generic incremental view interface driven by text edit deltas.
2. Move undo/redo into the left-side text editing model.
3. Introduce a shared block abstraction for both editor and preview panes.
4. Limit rebuild/layout to the edited region expanded to stable neighbor blocks.
5. Reuse block widgets through stable keys.
6. Expose block positions for later linked scrolling.
7. Make the preview selectable and copyable for both text and images.

### 2.2 Non-Goals

- This refactor does not need to introduce collaborative editing.
- This refactor does not need to support arbitrary rich text editing in the left pane.
- This refactor does not need to solve viewport virtualization yet.
- This refactor does not need to make `IncrementalViewModel` fully framework-agnostic across every possible document type. It only needs a clean enough interface to support multiple markdown-related projections.

## 3. Target Architecture

The refactored architecture should look like this:

```text
Keyboard / IME / Paste / Selection
    -> MarkdownTextEditor
    -> MarkdownEditBuffer
    -> TextEditDelta
    -> IncrementalViewModel::apply_edit(...)
       -> EditorBlockModel
       -> MarkdownPreviewModel
    -> BlockPatch
    -> left block list / right block list
    -> block layout registry
```

The key rule is:

- The left side owns edit state.
- The right side owns rendering state.
- Both sides consume the same edit delta stream.

## 4. Main Modules

The refactor should split responsibilities into the following modules.

### 4.1 `editor/edit_buffer.rs`

Owns:

- current full text
- current selection
- undo stack
- redo stack
- edit cause
- helpers to apply user edits and history operations

Does not own:

- markdown parsing
- block rendering
- file dialogs
- preview state

### 4.2 `editor/delta.rs`

Defines the shared edit protocol:

- `TextEditDelta`
- `EditCause`
- edit range helpers
- caret mapping helpers

This is the central contract between text editing and incremental projection.

### 4.3 `editor/incremental_view.rs`

Defines reusable traits:

- `IncrementalBlock`
- `IncrementalViewModel`
- `BlockPatch`
- `RebuildWindowPolicy`

Both left and right pane models implement this layer.

### 4.4 `editor/editor_block_model.rs`

Blockizes the raw markdown source for the left pane.

Responsibilities:

- split text into editor blocks
- expand rebuild window to adjacent blocks
- keep stable block keys
- expose text ranges
- expose block-local text slices

### 4.5 `editor/markdown_preview_model.rs`

Wraps markdown incremental parsing behind the shared interface.

Responsibilities:

- consume `TextEditDelta`
- expand reparse window beyond the direct edit
- parse affected markdown blocks
- reconcile keys
- return `BlockPatch<PreviewBlock>`

### 4.6 `editor/block_layout_registry.rs`

Stores block layout positions.

Responsibilities:

- record each block rect after layout
- query block by key
- query nearest block by y-position
- map text position to block

### 4.7 `editor/markdown_text_editor.rs`

Left pane widget built on top of the lower-level text editing primitives.

Responsibilities:

- capture editing input
- produce `TextEditDelta`
- host block widgets for the editor view
- drive incremental updates in the left pane

This should reuse `BasicEditor` design patterns instead of reimplementing selection and IME behavior from scratch.

## 5. Core Data Structures

### 5.1 Edit Delta

```rust
#[derive(Clone, Debug)]
pub enum EditCause {
  Insert,
  Delete,
  Replace,
  Paste,
  Cut,
  Undo,
  Redo,
  SetText,
  Ime,
}

#[derive(Clone, Debug)]
pub struct TextEditDelta {
  pub op: EditOp,
  pub text_after: String,
  pub selection_after: Range<usize>,
  pub cause: EditCause,
}
```

Rules:

- `TextEditDelta` is always based on byte offsets.
- `text_after` is the authoritative full text after the edit.
- `selection_after` is the caret/selection after the edit is applied.
- Undo and redo are represented as normal deltas, not as side channels.

### 5.2 Shared Block Trait

```rust
pub trait IncrementalBlock: Clone {
  type Key: Clone + Eq + std::hash::Hash;

  fn key(&self) -> &Self::Key;
  fn text_range(&self) -> Range<usize>;
}
```

All block types must:

- have a stable reuse key
- map back to source text
- be queryable by text range

### 5.3 Patch Protocol

```rust
#[derive(Clone, Debug)]
pub struct BlockPatch<B> {
  pub replace: Range<usize>,
  pub insert: Vec<B>,
  pub affected_text: Range<usize>,
}
```

Meaning:

- `replace` is the range in the old block list to replace.
- `insert` is the new block slice.
- `affected_text` is the source text window that triggered this patch.

This gives the view layer an explicit patch instead of forcing it to diff two full vectors again.

### 5.4 Incremental View Model

```rust
pub trait IncrementalViewModel {
  type Block: IncrementalBlock;

  fn blocks(&self) -> &[Self::Block];

  fn apply_edit(
    &mut self,
    full_text: &str,
    delta: &TextEditDelta,
  ) -> BlockPatch<Self::Block>;
}
```

Contract:

- The model updates its internal state.
- The model returns exactly one coherent patch.
- The caller applies that patch to UI state.

### 5.5 Rebuild Window Policy

```rust
pub trait RebuildWindowPolicy<B: IncrementalBlock> {
  fn expand(
    &self,
    blocks: &[B],
    edit_range: Range<usize>,
    full_text: &str,
  ) -> Range<usize>;
}
```

This isolates the logic for "expand to upper and lower blocks" from the parser/model itself.

## 6. Left Pane Detailed Design

## 6.1 Why the Left Pane Must Stop Using a Single `TextArea`

The current `TextArea` implementation in `widgets/src/input.rs` and `widgets/src/input/text_editable.rs` is optimized for one continuous text host. That is correct for generic text editing, but it is the wrong rendering unit for this markdown editor refactor because:

- the whole glyph tree is one logical layout object
- rebuild and layout invalidation are coarse
- there is no block identity
- there is no natural place to attach block positions

The new editor should still reuse:

- text input semantics
- selection behavior
- caret behavior
- IME behavior
- clipboard behavior
- change event conventions

But the editor view itself should become block-based.

## 6.2 Editor Block Split Rules

The left pane block model should split source text by the following heuristic:

1. Scan source line by line.
2. Keep accumulating lines into the current block.
3. A block boundary is eligible at a newline.
4. Materialize the boundary only if the accumulated character count is greater than `200`.
5. Also force boundaries around major markdown structures when needed:
   - fenced code block open/close
   - table region
   - list region transitions
   - blank-line paragraph boundaries

This yields:

- fewer overly tiny blocks
- better locality for edit rebuilds
- enough structure for future scroll sync

Suggested config:

```rust
pub struct EditorBlockSplitConfig {
  pub preferred_chars_per_block: usize,
  pub hard_min_chars_per_block: usize,
}
```

Default:

```rust
preferred_chars_per_block = 200
hard_min_chars_per_block = 80
```

`hard_min_chars_per_block` prevents pathological splitting when several short lines appear together.

## 6.3 Editor Block Shape

```rust
#[derive(Clone, Debug)]
pub struct EditorBlock {
  pub key: EditorBlockKey,
  pub text_range: Range<usize>,
  pub text: String,
  pub line_range: Range<usize>,
}
```

`line_range` is optional but useful for diagnostics and later gutter features.

## 6.4 Editor Rebuild Window

On every edit:

1. Locate the block containing `delta.op.position`.
2. Expand the rebuild window to:
   - previous block
   - current block
   - next block
3. If the edit touches a structural markdown boundary, keep expanding until a stable boundary is reached.

Structural expansion cases:

- edit intersects fenced code marker
- edit intersects table separator/header
- edit joins or splits blank-line-separated paragraphs
- edit joins or splits list blocks

Pseudo-code:

```rust
fn expand_editor_window(blocks: &[EditorBlock], edit: Range<usize>, text: &str) -> Range<usize> {
  let mut first = find_block_idx(blocks, edit.start).saturating_sub(1);
  let mut last = find_block_idx(blocks, edit.end).min(blocks.len().saturating_sub(1));
  last = (last + 1).min(blocks.len().saturating_sub(1));

  while first > 0 && crosses_markdown_structure(text, blocks[first].text_range.clone()) {
    first -= 1;
  }
  while last + 1 < blocks.len()
    && crosses_markdown_structure(text, blocks[last].text_range.clone())
  {
    last += 1;
  }

  blocks[first].text_range.start .. blocks[last].text_range.end
}
```

## 6.5 Editor Widget Composition

The left pane widget should be composed as:

```text
MarkdownTextEditor
  -> ScrollableWidget
    -> Column/Flex
      -> Reuse(EditorBlockKey)
        -> BlockEditorHost
```

Each block host:

- renders its own text slice
- reports layout rect
- forwards selection/caret mapping back to document coordinates

Important constraint:

- there is still only one logical document selection
- block widgets are render partitions, not separate documents

This means the left pane needs a document-level selection mapper, not per-block independent selections.

## 6.6 Mapping Selection Across Blocks

Because `BasicEditor` currently assumes a single text host, the left-side refactor should not immediately fork its internals into completely separate per-block editors. Instead, the safer phased design is:

Phase A:

- Keep one authoritative `MarkdownEditBuffer`.
- Use block widgets mainly as rendering partitions.
- Route key input and text mutation through a document-level editor controller.
- Use block-local hit testing and caret rect lookup to map between document position and block position.

Phase B:

- Introduce a reusable document-level selectable/editable host if needed.

This avoids prematurely duplicating `BasicEditor` logic in several block widgets.

## 7. Right Pane Detailed Design

## 7.1 Preview Block Shape

```rust
#[derive(Clone, Debug)]
pub struct PreviewBlock {
  pub key: PreviewBlockKey,
  pub text_range: Range<usize>,
  pub node: MarkdownNode,
}
```

This is close to the existing `DocumentNode`, but the key strategy changes.

## 7.2 Replace `NodeId`-Only Reuse With Key Reconciliation

Current problem:

- `allocate_node_id()` gives a new id to reparsed nodes.
- If a reparsed region contains logically unchanged blocks, they still lose identity.

Target:

- Reconcile keys inside the affected window.

Suggested strategy:

1. When a reparse window is computed, collect old blocks in the window.
2. Parse new blocks for the window.
3. Match new blocks to old blocks by:
   - overlapping text range
   - same `MarkdownNode` variant
   - content fingerprint
4. Reuse old keys for matched blocks.
5. Allocate new keys only for unmatched blocks.

Suggested fingerprint:

```rust
struct BlockFingerprint {
  kind: PreviewBlockKind,
  text_hash: u64,
}
```

This is enough for local reconciliation. It does not need global identity guarantees.

## 7.3 Preview Reparse Window

The preview side also needs expansion beyond the direct edit, because markdown block boundaries can shift.

Rules:

- Always include the directly intersecting blocks.
- Expand to one upper and one lower neighbor by default.
- Keep expanding while the candidate boundary is unstable.

Boundary is unstable when:

- a blank line was inserted or removed
- list prefixes changed
- table separator row changed
- fenced code markers changed
- heading markers changed at boundary lines

Pseudo-code:

```rust
fn apply_edit(&mut self, full_text: &str, delta: &TextEditDelta) -> BlockPatch<PreviewBlock> {
  let old_blocks = self.blocks.clone();
  let text_window = self.window_policy.expand(&old_blocks, delta.op.position..delta.op.position + delta.op.new_text.len(), full_text);
  let replace = block_index_window(&old_blocks, text_window.clone());
  let reparsed = parse_markdown_window(full_text, text_window.clone());
  let reparsed = reconcile_keys(&old_blocks[replace.clone()], reparsed);
  self.blocks.splice(replace.clone(), reparsed.clone());
  BlockPatch { replace, insert: reparsed, affected_text: text_window }
}
```

## 7.4 Right Pane View Composition

```text
SelectableArea
  -> ScrollableWidget
    -> Flex(Column)
      -> Reuse(PreviewBlockKey)
        -> PreviewBlockWidget
```

Each preview block widget:

- renders one markdown block
- reports layout rect
- registers itself with selection infrastructure

## 8. Selection and Copy Design

## 8.1 Outer Structure

The whole preview pane should be wrapped in `SelectableArea`.

This should be the default structure:

```text
SelectableArea
  -> PreviewBlockWidget...
```

This already aligns with the selection coordinator in `widgets/src/selectable_area.rs`.

## 8.2 Text Blocks

Where a preview block is purely text-based, use `RichTextSelectable` instead of plain `RichText`.

This is already supported by `widgets/src/rich_text.rs`.

## 8.3 Image Blocks

Standalone image blocks should use `SelectableImage`.

This is already supported by `widgets/src/selectable_area.rs`.

## 8.4 Mixed Inline Content

`MixedInline` currently builds:

- one `RichText`
- image overlays
- todo overlays
- inline code overlays

This is not enough for selection semantics because:

- the root is not selectable by default
- overlay images are visual children, not selection fragments
- mixed content selection should preserve document order

The target abstraction should be:

```rust
pub struct MixedInlineSelectable {
  inlines: Vec<InlineNode>,
}
```

Responsibilities:

- render text through `RichTextSelectable`
- render inline image overlays
- expose ordered selection fragments

## 8.5 Required `SelectableArea` Upgrade

The current `SelectionEntry::selection_data(&range)` returns one `TextAreaData`.

That is not expressive enough for mixed inline content, because one logical selection inside a block may contain:

- text before an image
- the image itself
- text after the image

To support correct ordered copy/export, the interface should evolve toward:

```rust
pub trait SelectionEntry<Position = TextPosition, Data = TextAreaData> {
  fn selection_fragments(&self, range: &SelectableRange<Position>) -> Vec<Data>;
}
```

Migration-compatible option:

- keep `selection_data()` temporarily
- add `selection_fragments()`
- default implementation can wrap the old single item result

Then `SelectableArea::selection_data()` becomes a flattening collector over fragments.

This is the cleanest path for mixed inline selection.

## 9. Undo/Redo Relocation

Undo/redo should move out of `MarkdownCoreDoc`.

### 9.1 Why

`MarkdownCoreDoc` is conceptually a projection/cache layer:

- source text in
- markdown structure out

History is not projection logic. It belongs to editing state.

### 9.2 New Ownership

`MarkdownEditBuffer` owns:

- `text: String`
- `selection: Range<usize>`
- `undo_redo: UndoRedoStack`

### 9.3 Resulting Flow

```text
user input
  -> MarkdownEditBuffer::apply_user_edit()
  -> TextEditDelta
  -> push undo record
  -> update current text
  -> notify left and right view models
```

Undo flow:

```text
Cmd+Z
  -> MarkdownEditBuffer::undo()
  -> TextEditDelta { cause: Undo, ... }
  -> editor block patch
  -> preview block patch
```

Redo is symmetrical.

## 10. Block Layout Registry and Scroll Sync

This is required now even if scroll sync lands later, because the left and right panes need stable spatial metadata.

Suggested type:

```rust
pub struct BlockLayoutSnapshot<K> {
  pub key: K,
  pub text_range: Range<usize>,
  pub rect: Rect,
}

pub struct BlockLayoutRegistry<K> {
  pub items: HashMap<K, BlockLayoutSnapshot<K>>,
}
```

Capabilities:

- `set_rect`
- `remove`
- `rect`
- `nearest_by_text_pos`
- `nearest_by_y`

Update timing:

- each block writes its rect in `on_performed_layout`
- registry is local to each pane
- a higher-level coordinator maps left and right panes by text range or block key

Scroll sync strategy for later:

1. Observe source pane scroll offset.
2. Find top visible block by y.
3. Convert to text position anchor.
4. Query peer pane for nearest block covering that text position.
5. Scroll peer pane toward that block.

## 11. File-Level Change Plan

### 11.1 New Files

Suggested additions under `examples/markdown/src/editor/`:

- `mod.rs`
- `delta.rs`
- `edit_buffer.rs`
- `incremental_view.rs`
- `editor_block_model.rs`
- `markdown_preview_model.rs`
- `block_layout_registry.rs`
- `markdown_text_editor.rs`

### 11.2 Existing Files To Refactor

`examples/markdown/src/lib.rs`

- split app wiring from view model logic
- replace current `MarkdownEditor` implementation
- let left and right panes consume patches

`examples/markdown/src/core_doc.rs`

- either remove it entirely
- or reduce it to a thin markdown projection wrapper

Recommended direction:

- replace `MarkdownCoreDoc` with `MarkdownPreviewModel`

`examples/markdown/src/incremental_parser.rs`

- stop exposing only full vectors
- expose enough internal helpers to build `BlockPatch`
- add key reconciliation support

`examples/markdown/src/components/mod.rs`

- add selectable variants for text-heavy blocks

`examples/markdown/src/components/mixed_inline.rs`

- evolve to `MixedInlineSelectable`
- integrate `RichTextSelectable`
- model inline image selection fragments

`widgets/src/input/text_editable.rs`

- optionally emit a more precise low-level edit delta event

`widgets/src/selectable_area.rs`

- add fragment-based selection API

## 12. Recommended Implementation Order

### Phase 1: Extract Protocols

1. Add `delta.rs`.
2. Add `incremental_view.rs`.
3. Add tests for `TextEditDelta` and `BlockPatch`.

Outcome:

- shared contracts exist before widget refactor starts.

### Phase 2: Move Undo/Redo

1. Introduce `MarkdownEditBuffer`.
2. Move undo/redo ownership there.
3. Keep the existing `TextArea` temporarily.
4. Adapt app wiring so preview consumes deltas from the buffer.

Outcome:

- editing and preview logic become decoupled even before the left pane is blockized.

### Phase 3: Convert Preview to Patch-Based Model

1. Add `MarkdownPreviewModel`.
2. Move incremental parse orchestration from `MarkdownCoreDoc`.
3. Return `BlockPatch<PreviewBlock>`.
4. Add local key reconciliation.

Outcome:

- right pane becomes a pure incremental projection.

### Phase 4: Add Selectable Preview

1. Wrap preview pane in `SelectableArea`.
2. Convert text rendering to `RichTextSelectable` where appropriate.
3. Convert image blocks to `SelectableImage`.
4. Upgrade mixed inline selection support.

Outcome:

- preview supports select and copy.

### Phase 5: Build Block-Based Left Pane

1. Add `EditorBlockModel`.
2. Introduce `MarkdownTextEditor`.
3. Replace single `TextArea`-only view with block-based editor rendering.
4. Reuse edit buffer and delta protocol.

Outcome:

- left pane gets local rebuild/layout.

### Phase 6: Add Layout Registry

1. Add per-pane block layout registry.
2. Record block rects after layout.
3. Expose lookup API for later scroll sync.

Outcome:

- structural prerequisites for linked scrolling are complete.

## 13. Key Pseudo-Code

### 13.1 Edit Buffer

```rust
impl MarkdownEditBuffer {
  pub fn apply_user_text_change(
    &mut self,
    old_text: &str,
    new_text: &str,
    selection_after: Range<usize>,
    cause: EditCause,
  ) -> Option<TextEditDelta> {
    let op = compute_edit_op(old_text, new_text)?;
    self.undo_redo.push(op.clone());
    self.redo_clear_if_needed();
    self.text = new_text.to_string();
    self.selection = selection_after.clone();
    Some(TextEditDelta {
      op,
      text_after: self.text.clone(),
      selection_after,
      cause,
    })
  }
}
```

### 13.2 Apply Delta to Both Panes

```rust
fn apply_delta(state: &mut AppState, delta: TextEditDelta) {
  let left_patch = state.editor_blocks.apply_edit(&delta.text_after, &delta);
  let right_patch = state.preview_blocks.apply_edit(&delta.text_after, &delta);

  state.left_view.apply_patch(left_patch);
  state.right_view.apply_patch(right_patch);
}
```

### 13.3 Preview Key Reconciliation

```rust
fn reconcile_keys(
  old_blocks: &[PreviewBlock],
  mut new_blocks: Vec<PreviewBlock>,
) -> Vec<PreviewBlock> {
  let mut unmatched_old = old_blocks.to_vec();

  for new_block in &mut new_blocks {
    if let Some((idx, old)) = unmatched_old
      .iter()
      .enumerate()
      .find(|(_, old)| compatible(old, new_block))
    {
      new_block.key = old.key.clone();
      unmatched_old.remove(idx);
    } else {
      new_block.key = PreviewBlockKey::fresh();
    }
  }

  new_blocks
}
```

### 13.4 Layout Registry Update

```rust
fn on_block_layout<K: Clone + Eq + Hash>(
  registry: &Stateful<BlockLayoutRegistry<K>>,
  key: K,
  text_range: Range<usize>,
  rect: Rect,
) {
  registry.write().set_rect(key, text_range, rect);
}
```

## 14. Testing Plan

### 14.1 Unit Tests

For `delta.rs`:

- insert
- delete
- replace
- repeated bytes
- multibyte UTF-8 edits

For `editor_block_model.rs`:

- split by newline over 200 chars
- no excessive tiny blocks
- expand to previous and next block
- expand across markdown structure boundary

For `markdown_preview_model.rs`:

- patch matches full parse
- key reconciliation preserves unchanged neighbors
- blank-line merge/split
- list boundary merge/split
- code fence edits

For `selectable_area.rs` fragment upgrade:

- text only
- image only
- text + image mixed
- ordered fragment preservation

### 14.2 Widget Tests

- preview `SelectableArea` copies selected text
- preview `SelectableArea` copies selected image
- mixed inline selection returns text and image fragments in order
- left pane undo/redo updates preview
- block rebuild only affects nearby blocks

### 14.3 Debug/Instrumentation

Useful temporary counters:

- number of rebuilt editor blocks
- number of reparsed preview blocks
- number of reused preview keys

These should be temporary or debug-only.

## 15. Migration Risks

### 15.1 Highest Risk

- Mapping one document-level selection onto blockized editor rendering.
- Supporting mixed inline selection with inline images in correct order.
- Avoiding unstable reuse keys in reparsed windows.

### 15.2 Lowest Risk

- Moving undo/redo out of `MarkdownCoreDoc`.
- Wrapping right pane in `SelectableArea`.
- Converting plain rich text usages to `RichTextSelectable`.

## 16. Final Recommended Direction

The clean architecture should be:

- `MarkdownEditBuffer` owns editable text and history.
- `TextEditDelta` is the only mutation protocol flowing out of the left pane.
- `IncrementalViewModel` is the abstraction extracted from current markdown incremental update logic.
- `EditorBlockModel` and `MarkdownPreviewModel` are two projections over the same text stream.
- `SelectableArea` owns preview-level selection.
- `MixedInlineSelectable` closes the last gap for text/image mixed copy.
- `BlockLayoutRegistry` provides the geometry foundation for linked scrolling.

If implementation needs to be staged, the correct order is:

1. extract protocols
2. move undo/redo
3. patchify preview
4. make preview selectable
5. blockize left pane
6. add layout registry and scroll sync hooks

That order minimizes churn and preserves a working editor after each phase.
