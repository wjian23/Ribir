# Troubleshooting

Common issues and solutions when developing with Ribir.

## Runtime Errors

### "BuildCtx::get() inside pipe!"

**Error:** Runtime panic when accessing `BuildCtx::get()` inside a `pipe!` macro.

**Cause:** `pipe!` expressions are re-evaluated whenever their dependencies change. `BuildCtx` is only valid during the initial build phase of the widget. When the pipe re-runs later (e.g., due to a state change), the build context is no longer available.

**Solution:** Capture the necessary values from `BuildCtx` *before* the `pipe!` block.

**Incorrect:**
```rust
// ❌ PANIC: BuildCtx::get() is called during updates
text: pipe!($read(count).to_string() + &BuildCtx::get().window().id().to_string())
```

**Correct:**
```rust
// ✅ Capture the value first
let window_id = BuildCtx::get().window().id();
text: pipe!($read(count).to_string() + &window_id.to_string())
```

## Animation Issues

### "Animation running but widget not updating"

**Symptom:** You see the animation running (e.g., via logs), but the widget on screen doesn't change.

**Cause:** You might be animating a standalone `Stateful` variable and trying to read it with `pipe!` in the widget property. Ribir's `Animate` widget uses a performance optimization called "shallow updates" which updates the value but skips the full reactive notification chain that `pipe!` relies on.

**Solution:** Bind the animation directly to the widget's property writer.

**Incorrect:**
```rust
let opacity = Stateful::new(0.);
// ... Animate 'opacity' ...
@SizedBox { opacity: pipe!(*$read(opacity)) } // ❌ Won't update smoothly
```

**Correct:**
```rust
let mut box_widget = @SizedBox { ... };
// ... Animate 'box_widget.opacity()' ...
@ { box_widget } // ✅ Updates directly
```
