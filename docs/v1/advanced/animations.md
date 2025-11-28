# Animations

Ribir provides a powerful animation system that allows you to create smooth, interactive UIs. The animation system is built around the `@Animate` widget and various transition mechanisms that enable you to animate any state in your application.

## The Animate Widget

The primary way to create animations in Ribir is through the `@Animate` widget. This widget allows you to animate state changes over a specified duration using different easing functions.

### Basic Animation

The `@Animate` widget requires three main properties:
- `state`: The state you want to animate
- `from`: The starting value of the animation
- `transition`: How the animation should progress over time

> **Important**: For the best performance and correctness, it is recommended to bind the animation directly to the widget's state (e.g., using `map_writer` or property writers like `.opacity()`) rather than a standalone `Stateful` variable used in a `pipe!`. `Animate` uses "shallow" updates which are optimized for widget properties but might not trigger all `pipe!` updates for standalone states.

```rust
use ribir::prelude::*;

fn basic_animation_example() -> Widget<'static> {
    fn_widget! {
        let mut box_widget = @SizedBox {
            size: Size::new(100., 100.),
            background: Color::BLUE,
        };

        let animate = @Animate {
            // Bind directly to the widget's state property
            state: box_widget.map_writer(|w| PartMut::new(&mut w.size)),
            from: Size::new(100., 100.),
            transition: EasingTransition {
                duration: Duration::from_millis(500),
                easing: easing::EASE_IN_OUT,
            }.box_it()
        };

        box_widget.on_tap(move |_| {
            // Update the target value using $write helper
            // This automatically handles cloning the writer for the closure
            *$write(box_widget.map_writer(|w| PartMut::new(&mut w.size))) = Size::new(200., 200.);
            animate.run();
        });

        @ { box_widget }
    }.into_widget()
}
```

### Animation Transitions

Transitions define how an animation progresses over time. Ribir provides several built-in transition types:

- `EasingTransition`: Uses easing functions to control the rate of change
- `DelayTransition`: Delays the start of an animation
- `RepeatTransition`: Repeats an animation a specified number of times

```rust
use ribir::prelude::*;

fn transition_example() -> Widget<'static> {
    fn_widget! {
        let mut box_widget = @SizedBox {
            size: Size::new(100., 100.),
            background: Color::BLUE,
            opacity: 0.,
        };

        let animate = @Animate {
            // Bind directly to the widget's opacity
            state: box_widget.opacity(),
            from: 0.,
            transition: EasingTransition {
                duration: Duration::from_millis(1000),
                easing: easing::EASE_IN_OUT,
            }.box_it()
        };

        box_widget.on_tap(move |_| {
            *$write(box_widget.opacity()) = 1.0;
            animate.run();
        });

        @ { box_widget }
    }.into_widget()
}
```

### Common Pitfall: Animating Intermediate State

A common mistake is to create a standalone `Stateful` variable, animate it, and bind it to a widget using `pipe!`.

```rust
// ❌ WRONG: Do not animate intermediate state
let opacity_state = Stateful::new(0.0);
let animate = @Animate {
    state: opacity_state.clone_writer(), // Animating standalone state
    ...
};

@SizedBox {
    opacity: pipe!(*$read(opacity_state)), // Pipe won't update during animation!
    ...
}
```

This fails because `Animate` uses `shallow()` updates for performance, which update the value but do not trigger `pipe!` listeners. Always bind the animation directly to the widget's property writer as shown in the examples above.

## Easing Functions

Easing functions control the rate of change during an animation, making animations feel more natural and expressive. Ribir provides several predefined easing functions:

- `easing::LINEAR`: Animates at a constant speed
- `easing::EASE_IN`: Starts slowly, accelerates toward the end
- `easing::EASE_OUT`: Starts quickly, decelerates toward the end
- `easing::EASE_IN_OUT`: Starts slowly, accelerates in the middle, then decelerates

### Cubic Bezier Easing

You can also create custom cubic Bezier easing functions by specifying two control points:

```rust
use ribir::prelude::*;

fn custom_easing_example() -> Widget<'static> {
    fn_widget! {
        let mut moving_box = @SizedBox {
            size: Size::new(50., 50.),
            background: Color::RED,
            margin: EdgeInsets::horizontal(0.),
        };

        let animate = @Animate {
            state: moving_box.margin(),
            from: EdgeInsets::horizontal(0.),
            transition: EasingTransition {
                duration: Duration::from_millis(1000),
                easing: easing::CubicBezierEasing::new(0.68, -0.55, 0.265, 1.55), // Bounce effect
            }.box_it()
        };

        moving_box.on_tap(move |_| {
            *$write(moving_box.margin()) = EdgeInsets::horizontal(200.);
            animate.run();
        });

        @SizedBox {
            size: Size::new(250., 100.),
            @ { moving_box }
        }
    }.into_widget()
}
```

## Keyframe Animations

Keyframes allow you to specify intermediate steps in an animation, providing fine-grained control over complex animations.

```rust
use ribir::prelude::*;

fn keyframes_example() -> Widget<'static> {
    fn_widget! {
        let mut box_widget = @SizedBox {
            size: Size::new(50., 50.),
            background: Color::GREEN,
        };

        let animate = @Animate {
            state: box_widget.map_writer(|w| PartMut::new(&mut w.size)),
            from: Size::new(50., 50.),
            transition: EasingTransition {
                duration: Duration::from_millis(1000),
                easing: easing::EASE_IN_OUT,
            }.box_it()
        };

        box_widget.on_tap(move |_| {
            // Create keyframes for complex animation
            let keyframe_state = keyframes! {
                state: box_widget.map_writer(|w| PartMut::new(&mut w.size)),
                0.25 => Size::new(100., 50.),  // Stretch horizontally at 25% progress
                0.5 => Size::new(100., 100.),  // Stretch vertically at 50% progress
                0.75 => Size::new(50., 100.),  // Shrink horizontally at 75% progress
                1.0 => Size::new(50., 50.),    // Return to original at 100% progress
            };

            animate.run();
        });
        
        @ { box_widget }
    }.into_widget()
}
```

Keyframes can be defined using either decimal values (0.0 to 1.0) or percentages:

```rust
// Using percentage syntax
let keyframe_state = keyframes! {
    state: opacity_writer,
    20% => 0.2,
    50% => 0.5,
    80% => 0.8,
};
```

## Animation Lifecycle and Runtime Behavior

Understanding the animation lifecycle is crucial for effective animation implementation. The Ribir animation system follows specific patterns for how animations interact with state and the rendering pipeline:

1. **Each frame draw**: During rendering, the animation modifies state values to reflect the current progress of the animation. On each frame, the animation system calculates the interpolated value based on the current time and transition function, and temporarily updates the state with that value.

2. **End of drawing**: After the frame is rendered, the animation system restores the original state values. This ensures that the underlying data model remains unchanged once the animation completes.

3. **Silent updates**: State changes during animation run silently, meaning they don't trigger additional reactive updates that could cause infinite loops. This is accomplished by the animation system's special handling of state modifications during animation frames.

4. **State propagation**: During animation, the state changes are propagated through the reactive system using the `shallow()` method of the StateWriter. This method updates the state and notifies the widget system for efficient repainting, but it **does not** trigger a full reactive notification for all listeners (like those in `pipe!` blocks) to avoid performance overhead and potential infinite loops.
   
   This is why it is critical to bind `Animate` to the **Widget's writer** (e.g. `widget.map_writer(...)`) rather than a standalone `Stateful` variable. When bound to a widget's writer, the `shallow()` update correctly notifies the widget to redraw with the new interpolated value.

This behavior ensures that:
- Animations run smoothly at high frame rates
- State changes during animation don't cause UI reflows or unnecessary rebuilds
- The original state value is preserved after animation completion
- Animation performance is optimized by avoiding redundant reactive updates

The animation runtime also follows this pattern:

- When `animate.run()` is called, the animation begins tracking frame updates
- On each frame, the animation calculates the current progress and updates the bound state temporarily using `shallow()` to propagate the value through reactive bindings
- All registered frame subscribers (like `pipe!` expressions) are notified and update their visual properties accordingly
- When the animation completes or is stopped, the original state is restored

## Complex Animations with Stagger

For coordinating multiple animations, Ribir provides the `Stagger` animation controller. This allows you to create sequences where animations start at timed intervals, creating sophisticated visual effects:

```rust
use ribir::prelude::*;

fn stagger_example() -> Widget<'static> {
    fn_widget! {
        let stagger = Stagger::new(
            Duration::from_millis(200), // 200ms between each animation start
            EasingTransition {
                duration: Duration::from_millis(500),
                easing: easing::EASE_IN_OUT,
            },
        );

        let mut text1 = @Text { text: "One", opacity: 0. };
        let mut text2 = @Text { text: "Two", opacity: 0. };
        let mut text3 = @Text { text: "Three", opacity: 0. };

        // Add animations to the stagger
        stagger.write().push_state(text1.opacity(), 0.);
        stagger.write().push_state(text2.opacity(), 0.);
        stagger.write().push_state(text3.opacity(), 0.);

        @Column {
            on_mounted: move |_| stagger.run(),
            @{ [text1, text2, text3] }
        }
    }.into_widget()
}
```

### Advanced Stagger Features

Stagger animations provide additional control options:

- **Different staggers**: Use `push_animation_with()` to specify different time intervals for each animation
- **Mixed animations**: Combine state-based animations with complete `@Animate` widgets in the same sequence
- **Runtime control**: Access stagger status using methods like `is_running()`, `run_times()`, and `has_ever_run()`

```rust
use ribir::prelude::*;

fn advanced_stagger_example() -> Widget<'static> {
    fn_widget! {
        let stagger = Stagger::new(
            Duration::from_millis(100),
            EasingTransition {
                duration: Duration::from_millis(300),
                easing: easing::EASE_IN_OUT,
            }
        );

        let mut box1 = @SizedBox { size: Size::new(50., 50.), background: Color::RED, opacity: 0. };
        let mut box2 = @SizedBox { size: Size::new(50., 50.), background: Color::GREEN, opacity: 0. };
        let mut box3 = @SizedBox { size: Size::new(50., 50.), background: Color::BLUE, opacity: 0. };

        // Add boxes with different stagger intervals
        stagger.write().push_state(box1.opacity(), 0.);
        stagger.write().push_state_with(Duration::from_millis(200), box2.opacity(), 0.); // Wait 200ms
        stagger.write().push_animation({
            let animate = @Animate {
                state: box3.opacity(),
                from: 0.,
                transition: EasingTransition {
                    duration: Duration::from_millis(300),
                    easing: easing::EASE_IN_OUT,
                }.box_it()
            };
            animate
        });

        @Row {
            on_mounted: move |_| stagger.run(),
            @{ [box1, box2, box3] }
        }
    }.into_widget()
}
```

## Animation Control

Animations can be controlled programmatically using the animation instance:

- `run()`: Starts or restarts the animation
- `stop()`: Stops the animation and restores the state to its final value
- `is_running()`: Checks if the animation is currently running

```rust
use ribir::prelude::*;

fn animation_control_example() -> Widget<'static> {
    fn_widget! {
        let mut box_widget = @SizedBox {
            size: Size::new(100., 100.),
            background: Color::PURPLE,
            opacity: 0.0,
        };

        let tap_animation = @Animate {
            state: box_widget.opacity(),
            from: 0.,
            transition: EasingTransition {
                duration: Duration::from_millis(2000),
                easing: easing::EASE_IN_OUT,
            }.box_it()
        };
        
        let start_animation = @Animate {
            state: box_widget.opacity(),
            from: 0.,  // Start from current value is done dynamically
            transition: EasingTransition {
                duration: Duration::from_millis(2000),
                easing: easing::EASE_IN_OUT,
            }.box_it()
        };

        box_widget.on_tap(move |_| {
            *$write(box_widget.opacity()) = 1.0;
            tap_animation.run();
        });

        @Column {
            @ { box_widget }
            @Row {
                @Button {
                    on_tap: move |_| {
                        *$write(box_widget.opacity()) = 1.0;
                        start_animation.run();
                    },
                    @Text { text: "Start" }
                }
                @Button {
                    on_tap: move |_| {
                        start_animation.stop();
                    },
                    @Text { text: "Stop" }
                }
            }
        }
    }.into_widget()
}
```

## Animation Composition

Animations can be combined and layered to create complex effects. You can:
- Run multiple animations in parallel using different state values
- Sequence animations using stagger controllers
- Layer custom easing functions for unique effects

```rust
use ribir::prelude::*;

fn composition_example() -> Widget<'static> {
    fn_widget! {
        let mut box_widget = @SizedBox {
            size: Size::new(50., 50.),
            background: Color::BLUE,
            opacity: 0.,
            transform: Transform::identity(),
        };

        let opacity_anim = @Animate {
            state: box_widget.opacity(),
            from: 0.,
            transition: EasingTransition {
                duration: Duration::from_millis(1000),
                easing: easing::EASE_IN_OUT,
            }.box_it()
        };

        let size_anim = @Animate {
            state: box_widget.map_writer(|w| PartMut::new(&mut w.size)),
            from: Size::new(50., 50.),
            transition: EasingTransition {
                duration: Duration::from_millis(800),
                easing: easing::EASE_OUT,
            }.box_it()
        };

        let rotation_anim = @Animate {
            state: box_widget.map_writer(|w| PartMut::new(&mut w.transform)),
            from: Transform::identity(),
            transition: EasingTransition {
                duration: Duration::from_millis(2000),
                easing: easing::LINEAR,
            }.box_it()
        };

        box_widget.on_tap(move |_| {
            // Run multiple animations in parallel
            // Use split_writer when needing to mutate multiple fields of the same widget
            // or just write to them individually if they are separate states (like opacity/transform here)
            // Note: box_widget is a Stateful<FatObj<SizedBox>>
            
            // For separate properties like opacity and transform which are separate states in FatObj:
            *$write(box_widget.opacity()) = 1.0;
            
            // For size (in SizedBox), it's part of the widget data
             *$write(box_widget.map_writer(|w| PartMut::new(&mut w.size))) = Size::new(100., 100.);

            *$write(box_widget.map_writer(|w| PartMut::new(&mut w.transform))) = Transform::identity()
                .pre_rotate(Angle::radians(std::f32::consts::PI * 2.));

            opacity_anim.run();
            size_anim.run();
            rotation_anim.run();
        });

        @ { box_widget }
    }.into_widget()
}
```

## Advanced Animation Patterns

### Repeating Animations

Animations can be set to repeat automatically using the `repeat` transition modifier:

```rust
use ribir::prelude::*;

fn repeating_animation_example() -> Widget<'static> {
    fn_widget! {
        let mut box_widget = @SizedBox {
            size: Size::new(100., 100.),
            background: Color::YELLOW,
            transform: Transform::identity(),
        };

        let animate = @Animate {
            state: box_widget.map_writer(|w| PartMut::new(&mut w.transform)),
            from: Transform::identity(),
            transition: EasingTransition {
                duration: Duration::from_millis(1000),
                easing: easing::LINEAR,
            }.repeat(f32::MAX).box_it() // Repeat infinitely
        };

        box_widget.on_tap(move |_| {
            *$write(box_widget.map_writer(|w| PartMut::new(&mut w.transform))) = 
                Transform::identity().pre_rotate(Angle::radians(std::f32::consts::PI * 2.));
            animate.run();
        });
        
        @ { box_widget }
    }.into_widget()
}
```

### Delayed Animations

Animations can be delayed using the `delay` transition modifier:

```rust
use ribir::prelude::*;

fn delayed_animation_example() -> Widget<'static> {
    fn_widget! {
        let mut box_widget = @SizedBox {
            size: Size::new(100., 100.),
            background: Color::CYAN,
            opacity: 0.,
        };

        let animate = @Animate {
            state: box_widget.opacity(),
            from: 0.,
            transition: EasingTransition {
                duration: Duration::from_millis(1000),
                easing: easing::EASE_IN_OUT,
            }
            .delay(Duration::from_millis(500)) // Wait 500ms before starting
            .box_it()
        };

        box_widget.on_tap(move |_| {
            *$write(box_widget.opacity()) = 1.0;
            animate.run();
        });

        @ { box_widget }
    }.into_widget()
}
```

Animations are a powerful tool for creating engaging, intuitive user experiences. By mastering the animation system in Ribir, you can create smooth, responsive applications that feel alive and interactive.