use crate::wit_types::*;
use extension_host::wasm_host::wit;
use gpui::{
    App, ClickEvent, Div, InteractiveElement, MouseButton, Stateful, Window, prelude::*,
};

/// Attach mouse event handlers (click, double-click, right-click, hover, mouse_down, mouse_up, mouse_move, scroll_wheel)
pub fn attach_mouse_event_handlers(
    mut element: Stateful<Div>,
    source_id: &Option<String>,
    events: &wit::since_v0_9_0::ui_elements::EventFlags,
    on_event: &(impl Fn(String, WitUiEvent, &mut Window, &mut App) + Clone + 'static),
) -> Stateful<Div> {
    // Handle click and double-click with the same handler
    if events.on_click || events.on_double_click {
        let source_id = source_id.clone().unwrap_or_default();
        let cb = on_event.clone();
        let handle_double = events.on_double_click;
        let handle_single = events.on_click;
        element = element.on_click(move |click, window, cx| {
            let mouse_data = mouse_data_from_click(click);
            if handle_double && mouse_data.click_count >= 2 {
                cb(
                    source_id.clone(),
                    WitUiEvent::DoubleClicked(mouse_data),
                    window,
                    cx,
                );
            } else if handle_single {
                cb(
                    source_id.clone(),
                    WitUiEvent::Clicked(mouse_data),
                    window,
                    cx,
                );
            }
        });
    }

    // Right-click uses on_aux_click
    if events.on_right_click {
        let source_id = source_id.clone().unwrap_or_default();
        let cb = on_event.clone();
        element = element.on_aux_click(move |click, window, cx| {
            cb(
                source_id.clone(),
                WitUiEvent::RightClicked(mouse_data_from_click(click)),
                window,
                cx,
            );
        });
    }

    if events.on_hover {
        let source_id = source_id.clone().unwrap_or_default();
        let cb = on_event.clone();
        element = element.on_hover(move |hovered, window, cx| {
            let event = if *hovered {
                WitUiEvent::HoverStart
            } else {
                WitUiEvent::HoverEnd
            };
            cb(source_id.clone(), event, window, cx);
        });
    }

    if events.on_mouse_down {
        let source_id = source_id.clone().unwrap_or_default();
        let cb = on_event.clone();
        element = element.on_mouse_down(MouseButton::Left, move |event, window, cx| {
            cb(
                source_id.clone(),
                WitUiEvent::MouseDown(WitMouseEventData {
                    x: f32::from(event.position.x),
                    y: f32::from(event.position.y),
                    button: 0,
                    click_count: 0,
                    shift: event.modifiers.shift,
                    ctrl: event.modifiers.control,
                    alt: event.modifiers.alt,
                    meta: event.modifiers.platform,
                }),
                window,
                cx,
            );
        });
    }

    if events.on_mouse_up {
        let source_id = source_id.clone().unwrap_or_default();
        let cb = on_event.clone();
        element = element.on_mouse_up(MouseButton::Left, move |event, window, cx| {
            cb(
                source_id.clone(),
                WitUiEvent::MouseUp(WitMouseEventData {
                    x: f32::from(event.position.x),
                    y: f32::from(event.position.y),
                    button: 0,
                    click_count: 0,
                    shift: event.modifiers.shift,
                    ctrl: event.modifiers.control,
                    alt: event.modifiers.alt,
                    meta: event.modifiers.platform,
                }),
                window,
                cx,
            );
        });
    }

    if events.on_mouse_move {
        let source_id = source_id.clone().unwrap_or_default();
        let cb = on_event.clone();
        element = element.on_mouse_move(move |event, window, cx| {
            cb(
                source_id.clone(),
                WitUiEvent::MouseMoved(WitMouseEventData {
                    x: f32::from(event.position.x),
                    y: f32::from(event.position.y),
                    button: 0,
                    click_count: 0,
                    shift: event.modifiers.shift,
                    ctrl: event.modifiers.control,
                    alt: event.modifiers.alt,
                    meta: event.modifiers.platform,
                }),
                window,
                cx,
            );
        });
    }

    if events.on_scroll_wheel {
        let source_id = source_id.clone().unwrap_or_default();
        let cb = on_event.clone();
        element = element.on_scroll_wheel(move |event, window, cx| {
            let pixel_delta = event.delta.pixel_delta(window.line_height());
            cb(
                source_id.clone(),
                WitUiEvent::ScrollWheel(wit::since_v0_9_0::gui::ScrollEventData {
                    delta_x: f32::from(pixel_delta.x),
                    delta_y: f32::from(pixel_delta.y),
                    precise: event.delta.precise(),
                }),
                window,
                cx,
            );
        });
    }

    element
}

/// Attach keyboard event handlers (key_down, key_up)
pub fn attach_keyboard_event_handlers(
    mut element: Stateful<Div>,
    source_id: &Option<String>,
    events: &wit::since_v0_9_0::ui_elements::EventFlags,
    on_event: &(impl Fn(String, WitUiEvent, &mut Window, &mut App) + Clone + 'static),
) -> Stateful<Div> {
    if events.on_key_down {
        let source_id = source_id.clone().unwrap_or_default();
        let cb = on_event.clone();
        element = element.on_key_down(move |event, window, cx| {
            cb(
                source_id.clone(),
                WitUiEvent::KeyDown(wit::since_v0_9_0::gui::KeyEventData {
                    key: event.keystroke.key.clone(),
                    shift: event.keystroke.modifiers.shift,
                    ctrl: event.keystroke.modifiers.control,
                    alt: event.keystroke.modifiers.alt,
                    meta: event.keystroke.modifiers.platform,
                    repeat: false, // GPUI doesn't expose repeat in KeyDownEvent
                }),
                window,
                cx,
            );
        });
    }

    if events.on_key_up {
        let source_id = source_id.clone().unwrap_or_default();
        let cb = on_event.clone();
        element = element.on_key_up(move |event, window, cx| {
            cb(
                source_id.clone(),
                WitUiEvent::KeyUp(wit::since_v0_9_0::gui::KeyEventData {
                    key: event.keystroke.key.clone(),
                    shift: event.keystroke.modifiers.shift,
                    ctrl: event.keystroke.modifiers.control,
                    alt: event.keystroke.modifiers.alt,
                    meta: event.keystroke.modifiers.platform,
                    repeat: false,
                }),
                window,
                cx,
            );
        });
    }

    element
}

/// Attach drag and drop event handlers
pub fn attach_drag_drop_handlers(
    mut element: Stateful<Div>,
    source_id: &Option<String>,
    events: &wit::since_v0_9_0::ui_elements::EventFlags,
    on_event: &(impl Fn(String, WitUiEvent, &mut Window, &mut App) + Clone + 'static),
) -> Stateful<Div> {
    // Drag events - notify drag started (full drag preview needs complex setup)
    if events.on_drag {
        let source_id = source_id.clone().unwrap_or_default();
        let cb = on_event.clone();
        // Send DragStarted event on mouse down
        // Full drag-and-drop with preview would require Entity<W> constructor
        // which is too complex for WIT boundary - extensions can handle drag manually
        if !events.on_mouse_down {
            element = element.on_mouse_down(MouseButton::Left, move |_event, window, cx| {
                cb(
                    source_id.clone(),
                    WitUiEvent::DragStarted,
                    window,
                    cx,
                );
            });
        }
    }

    // Drop events
    if events.on_drop {
        let source_id = source_id.clone().unwrap_or_default();
        let cb = on_event.clone();
        element = element.on_drop(move |data: &String, window, cx| {
            cb(
                source_id.clone(),
                WitUiEvent::Dropped(data.clone()),
                window,
                cx,
            );
        });
    }

    element
}

/// Attach focus tracking for keyboard navigation
pub fn attach_focus_handler(
    mut element: Stateful<Div>,
    focus_handle_id: Option<u32>,
    focus_handles: &std::sync::Arc<std::sync::Mutex<std::collections::HashMap<u32, gpui::FocusHandle>>>,
) -> Stateful<Div> {
    // Focus handle integration - use pre-created GPUI FocusHandle
    if let Some(handle_id) = focus_handle_id {
        // Get the pre-created FocusHandle for this element
        let focus_handle = {
            let handles = focus_handles.lock().unwrap();
            handles.get(&handle_id).cloned()
        };

        if let Some(focus_handle) = focus_handle {
            // Track focus with GPUI - this enables tab navigation and focus styling
            element = element.track_focus(&focus_handle);

            // Note: GPUI doesn't have on_focus_in/on_focus_out element methods.
            // Focus events need to be subscribed via cx.on_focus_in/cx.on_focus_out
            // which requires a Context, not available during render.
            // The extension can detect focus changes by checking is_focused() on key events
            // or we need to subscribe in ExtensionGuiView::new() and emit events there.
            // For now, we mark the element focusable via track_focus which enables:
            // - Tab navigation between focusable elements
            // - Focus styling (can use .focus() style modifier)
            // - Focus checking in key event handlers
        }
    }

    element
}

pub fn mouse_data_from_click(click: &ClickEvent) -> WitMouseEventData {
    match click {
        ClickEvent::Mouse(m) => WitMouseEventData {
            x: f32::from(m.down.position.x),
            y: f32::from(m.down.position.y),
            button: match m.down.button {
                MouseButton::Left => 0,
                MouseButton::Right => 1,
                MouseButton::Middle => 2,
                _ => 0,
            },
            click_count: m.down.click_count as u32,
            shift: m.down.modifiers.shift,
            ctrl: m.down.modifiers.control,
            alt: m.down.modifiers.alt,
            meta: m.down.modifiers.platform,
        },
        ClickEvent::Keyboard(_) => WitMouseEventData {
            x: 0.0,
            y: 0.0,
            button: 0,
            click_count: 1,
            shift: false,
            ctrl: false,
            alt: false,
            meta: false,
        },
    }
}
