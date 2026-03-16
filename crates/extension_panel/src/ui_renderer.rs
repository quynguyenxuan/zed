use ui_input::InputField;
use extension_host::wasm_host::wit;
use gpui::{
    AbsoluteLength, AlignContent, AlignItems, AnyElement, App, ClickEvent, Corners, CursorStyle,
    DefiniteLength, Display, Edges, ElementId, Entity, Fill, FlexDirection, FlexWrap, FocusHandle, FontStyle,
    FontWeight, Hsla, IntoElement, Length, Overflow, Position, SharedString,
    Visibility, Window, div, hsla, px, rems, prelude::*,
};
use std::sync::{Arc, Mutex};
use theme::ActiveTheme as _;
use ui;

type WitUiTree = wit::since_v0_9_0::ui_elements::UiTree;
type WitUiNode = wit::since_v0_9_0::ui_elements::UiNode;
type WitStyle = wit::since_v0_9_0::ui_elements::Style;
type WitColor = wit::since_v0_9_0::ui_elements::Color;
type WitLength = wit::since_v0_9_0::ui_elements::Length;
type WitDefiniteLength = wit::since_v0_9_0::ui_elements::DefiniteLength;
type WitAbsoluteLength = wit::since_v0_9_0::ui_elements::AbsoluteLength;
type WitBackground = wit::since_v0_9_0::ui_elements::Background;
type WitEdgesLength = wit::since_v0_9_0::ui_elements::EdgesLength;
type WitEdgesAbsolute = wit::since_v0_9_0::ui_elements::EdgesAbsolute;
type WitCornersAbsolute = wit::since_v0_9_0::ui_elements::CornersAbsolute;
type WitDivNode = wit::since_v0_9_0::ui_elements::DivNode;
type WitTextNode = wit::since_v0_9_0::ui_elements::TextNode;
type WitInputNode = wit::since_v0_9_0::ui_elements::InputNode;
type WitSvgNode = wit::since_v0_9_0::ui_elements::SvgNode;
type WitImgNode = wit::since_v0_9_0::ui_elements::ImgNode;
type WitIconSource = wit::since_v0_9_0::ui_elements::IconSource;
pub type WitUiEvent = wit::since_v0_9_0::gui::UiEvent;
type WitMouseEventData = wit::since_v0_9_0::gui::MouseEventData;

// ── Input Selection State ─────────────────────────────────────────────────

// Input state management removed - InputField handles this internally

// ── UI Tree Rendering ──────────────────────────────────────────────────────

/// Converts a flat `WitUiTree` into a GPUI element hierarchy.
///
/// `on_event(source_id, event, window, cx)` is called when the user interacts
/// with any interactive element. The caller is responsible for forwarding the
/// event to the extension and re-rendering.
///
/// `focus_handles` maps WIT focus handle IDs to GPUI FocusHandles.
pub fn render_ui_tree(
    tree: &WitUiTree,
    on_event: impl Fn(String, WitUiEvent, &mut Window, &mut App) + Clone + 'static,
    focus_handles: Arc<Mutex<std::collections::HashMap<u32, FocusHandle>>>,
    text_input_fields: &std::collections::HashMap<String, Entity<InputField>>,
    window: &mut Window,
    cx: &App,
) -> AnyElement {
    if tree.nodes.is_empty() {
        return div().into_any_element();
    }
    render_node(&tree.nodes, tree.root, &on_event, &focus_handles, text_input_fields, window, cx)
}

fn render_node(
    nodes: &[WitUiNode],
    idx: u32,
    on_event: &(impl Fn(String, WitUiEvent, &mut Window, &mut App) + Clone + 'static),
    focus_handles: &Arc<Mutex<std::collections::HashMap<u32, FocusHandle>>>,
    text_input_fields: &std::collections::HashMap<String, Entity<InputField>>,
    window: &mut Window,
    cx: &App,
) -> AnyElement {
    match nodes.get(idx as usize) {
        None => div().into_any_element(),
        Some(WitUiNode::Div(n)) => render_div(nodes, n, idx, on_event, focus_handles, text_input_fields, window, cx),
        Some(WitUiNode::Text(n)) => render_text(n, cx),
        Some(WitUiNode::Svg(n)) => render_svg(n, cx),
        Some(WitUiNode::Img(n)) => render_img(n, on_event, cx),
        Some(WitUiNode::Input(n)) => render_input(n, text_input_fields, cx),
        Some(WitUiNode::UniformList(n)) => render_uniform_list(n, on_event, focus_handles, window, cx),
    }
}

fn render_div(
    nodes: &[WitUiNode],
    n: &WitDivNode,
    node_idx: u32,
    on_event: &(impl Fn(String, WitUiEvent, &mut Window, &mut App) + Clone + 'static),
    focus_handles: &Arc<Mutex<std::collections::HashMap<u32, FocusHandle>>>,
    text_input_fields: &std::collections::HashMap<String, Entity<InputField>>,
    window: &mut Window,
    cx: &App,
) -> AnyElement {
    let element_id = n
        .id
        .as_ref()
        .map(|s| ElementId::Name(s.clone().into()))
        .unwrap_or(ElementId::Integer(node_idx as u64));

    let mut element = div().id(element_id);
    apply_style(&mut element, &n.style, cx);

    for &child_idx in &n.children {
        element = element.child(render_node(nodes, child_idx, on_event, focus_handles, text_input_fields, window, cx));
    }

    // Handle click and double-click with the same handler
    if n.events.on_click || n.events.on_double_click {
        let source_id = n.id.clone().unwrap_or_default();
        let cb = on_event.clone();
        let handle_double = n.events.on_double_click;
        let handle_single = n.events.on_click;
        element = element.on_click(move |click, window, cx| {
            let mouse_data = mouse_data_from_click(click);
            // Check click count for double-click
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
    if n.events.on_right_click {
        let source_id = n.id.clone().unwrap_or_default();
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

    if n.events.on_hover {
        let source_id = n.id.clone().unwrap_or_default();
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

    if n.events.on_mouse_down {
        let source_id = n.id.clone().unwrap_or_default();
        let cb = on_event.clone();
        element = element.on_mouse_down(gpui::MouseButton::Left, move |event, window, cx| {
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

    if n.events.on_mouse_up {
        let source_id = n.id.clone().unwrap_or_default();
        let cb = on_event.clone();
        element = element.on_mouse_up(gpui::MouseButton::Left, move |event, window, cx| {
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

    if n.events.on_mouse_move {
        let source_id = n.id.clone().unwrap_or_default();
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

    if n.events.on_scroll_wheel {
        let source_id = n.id.clone().unwrap_or_default();
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

    if n.events.on_key_down {
        let source_id = n.id.clone().unwrap_or_default();
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

    if n.events.on_key_up {
        let source_id = n.id.clone().unwrap_or_default();
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

    // Focus handle integration - use pre-created GPUI FocusHandle
    if let Some(handle_id) = n.focus_handle_id {
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

    // Drag events - notify drag started (full drag preview needs complex setup)
    if n.events.on_drag {
        let source_id = n.id.clone().unwrap_or_default();
        let cb = on_event.clone();
        // Send DragStarted event on mouse down
        // Full drag-and-drop with preview would require Entity<W> constructor
        // which is too complex for WIT boundary - extensions can handle drag manually
        if !n.events.on_mouse_down {
            element = element.on_mouse_down(gpui::MouseButton::Left, move |_event, window, cx| {
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
    if n.events.on_drop {
        let source_id = n.id.clone().unwrap_or_default();
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

    if let Some(tooltip_text) = n.tooltip.clone() {
        element = element.tooltip(ui::Tooltip::text(tooltip_text));
    }

    element.into_any_element()
}

fn render_text(n: &WitTextNode, cx: &App) -> AnyElement {
    let mut element = div();
    apply_style(&mut element, &n.style, cx);
    element
        .child(SharedString::from(n.content.clone()))
        .into_any_element()
}

fn render_input(
    n: &WitInputNode,
    text_input_fields: &std::collections::HashMap<String, Entity<InputField>>,
    cx: &App,
) -> AnyElement {
    let input_id = n.id.clone();

    // If we have an InputField entity for this input, render it (real focus/IME/clipboard!)
    if let Some(input_field) = text_input_fields.get(&input_id) {
        // Don't wrap in div - render InputField directly with style applied as flex container
        let mut element = div()
            .id(ElementId::Name(input_id.into()));
        apply_style(&mut element, &n.style, cx);
        // Ensure the wrapper doesn't block input events
        return element
            .flex()
            .child(input_field.clone())
            .into_any_element();
    }

    // Fallback: simple placeholder text if InputField not created yet
    let mut element = div()
        .id(ElementId::Name(input_id.into()))
        .cursor(CursorStyle::IBeam)
        .flex()
        .items_center();

    apply_style(&mut element, &n.style, cx);

    let display_text = if n.value.is_empty() {
        n.placeholder.clone().unwrap_or_default()
    } else {
        n.value.clone()
    };

    let is_empty = n.value.is_empty();
    let text_color = if is_empty {
        cx.theme().colors().text_placeholder
    } else {
        cx.theme().colors().text
    };

    element
        .text_color(text_color)
        .child(display_text)
        .into_any_element()
}


fn render_svg(n: &WitSvgNode, cx: &App) -> AnyElement {
    let mut element = div();
    apply_style(&mut element, &n.style, cx);

    let icon = match &n.source {
        WitIconSource::Named(name) => {
            // Try to map to IconName, fallback to path
            ui::Icon::from_path(format!("icons/{}.svg", name))
        }
        WitIconSource::Path(path) => {
            ui::Icon::from_path(path.clone())
        }
    };

    let icon = if let Some(hsla) = n.color.as_ref().and_then(|c| resolve_color(c, cx)) {
        icon.color(ui::Color::Custom(hsla))
    } else {
        icon
    };

    element.child(icon).into_any_element()
}

fn render_img(n: &WitImgNode, on_event: &(impl Fn(String, WitUiEvent, &mut Window, &mut App) + Clone + 'static), cx: &App) -> AnyElement {
    let element_id = n
        .id
        .as_ref()
        .map(|s| ElementId::Name(s.clone().into()))
        .unwrap_or(ElementId::Integer(0));

    let mut element = div().id(element_id);
    apply_style(&mut element, &n.style, cx);

    // Handle image events
    if n.events.on_click {
        let source_id = n.id.clone().unwrap_or_default();
        let cb = on_event.clone();
        element = element.on_click(move |click, window, cx| {
            cb(
                source_id.clone(),
                WitUiEvent::Clicked(mouse_data_from_click(click)),
                window,
                cx,
            );
        });
    }

    // Load image from path
    use gpui::{img, ImageSource, Resource};
    use std::path::Path;

    let image_path = Arc::from(Path::new(&n.src));
    let image_source = ImageSource::Resource(Resource::Path(image_path));

    element
        .child(img(image_source))
        .into_any_element()
}

// ── Style application ──────────────────────────────────────────────────────

fn apply_style(element: &mut impl Styled, s: &WitStyle, cx: &App) {
    use wit::since_v0_9_0::ui_elements::{
        AlignContent as WitAC, AlignItems as WitAI, CursorStyle as WitCursor, DisplayType,
        FlexDirection as WitFD, FlexWrap as WitFW, FontStyle as WitFS,
        FontWeight as WitFontWeight, OverflowType, PositionType, VisibilityType,
    };

    let sr = element.style();

    if let Some(d) = &s.display {
        sr.display = Some(match d {
            DisplayType::Block => Display::Block,
            DisplayType::Flex => Display::Flex,
        });
    }
    if let Some(p) = &s.position {
        sr.position = Some(match p {
            PositionType::Relative => Position::Relative,
            PositionType::Absolute => Position::Absolute,
        });
    }
    if let Some(v) = &s.visibility {
        sr.visibility = Some(match v {
            VisibilityType::Visible => Visibility::Visible,
            VisibilityType::Hidden => Visibility::Hidden,
        });
    }

    if let Some(w) = &s.width {
        sr.size.width = Some(cvt_length(w));
    }
    if let Some(h) = &s.height {
        sr.size.height = Some(cvt_length(h));
    }
    if let Some(w) = &s.min_width {
        sr.min_size.width = Some(Length::Definite(cvt_definite(w)));
    }
    if let Some(h) = &s.min_height {
        sr.min_size.height = Some(Length::Definite(cvt_definite(h)));
    }
    if let Some(w) = &s.max_width {
        sr.max_size.width = Some(Length::Definite(cvt_definite(w)));
    }
    if let Some(h) = &s.max_height {
        sr.max_size.height = Some(Length::Definite(cvt_definite(h)));
    }

    if let Some(t) = &s.top {
        sr.inset.top = Some(cvt_length(t));
    }
    if let Some(r) = &s.right {
        sr.inset.right = Some(cvt_length(r));
    }
    if let Some(b) = &s.bottom {
        sr.inset.bottom = Some(cvt_length(b));
    }
    if let Some(l) = &s.left {
        sr.inset.left = Some(cvt_length(l));
    }

    if let Some(fd) = &s.flex_direction {
        sr.flex_direction = Some(match fd {
            WitFD::Row => FlexDirection::Row,
            WitFD::Column => FlexDirection::Column,
            WitFD::RowReverse => FlexDirection::RowReverse,
            WitFD::ColumnReverse => FlexDirection::ColumnReverse,
        });
    }
    if let Some(fw) = &s.flex_wrap {
        sr.flex_wrap = Some(match fw {
            WitFW::NoWrap => FlexWrap::NoWrap,
            WitFW::Wrap => FlexWrap::Wrap,
            WitFW::WrapReverse => FlexWrap::WrapReverse,
        });
    }
    if let Some(ai) = &s.align_items {
        sr.align_items = Some(cvt_align_items(ai, &WitAI::Stretch));
    }
    if let Some(ac) = &s.align_content {
        sr.align_content = Some(cvt_align_content(ac, &WitAC::Stretch));
    }
    if let Some(jc) = &s.justify_content {
        sr.justify_content = Some(cvt_justify_content(jc));
    }
    if let Some(g) = &s.gap {
        let v = cvt_definite(g);
        sr.gap.width = Some(v);
        sr.gap.height = Some(v);
    }
    if let Some(g) = &s.column_gap {
        sr.gap.width = Some(cvt_definite(g));
    }
    if let Some(g) = &s.row_gap {
        sr.gap.height = Some(cvt_definite(g));
    }

    if let Some(fg) = s.flex_grow {
        sr.flex_grow = Some(fg);
    }
    if let Some(fs) = s.flex_shrink {
        sr.flex_shrink = Some(fs);
    }
    if let Some(fb) = &s.flex_basis {
        sr.flex_basis = Some(cvt_length(fb));
    }
    if let Some(aself) = &s.align_self {
        sr.align_self = Some(cvt_align_items(aself, &WitAI::Stretch));
    }

    if let Some(p) = &s.padding {
        let e = cvt_edges_definite(p);
        sr.padding.top = Some(e.top);
        sr.padding.right = Some(e.right);
        sr.padding.bottom = Some(e.bottom);
        sr.padding.left = Some(e.left);
    }
    if let Some(m) = &s.margin {
        let e = cvt_edges_length(m);
        sr.margin.top = Some(e.top);
        sr.margin.right = Some(e.right);
        sr.margin.bottom = Some(e.bottom);
        sr.margin.left = Some(e.left);
    }

    if let Some(bg) = &s.background {
        if let Some(color) = resolve_background(bg, cx) {
            sr.background = Some(Fill::from(color));
        }
    }
    if let Some(op) = s.opacity {
        sr.opacity = Some(op);
    }

    if let Some(bw) = &s.border_widths {
        let e = cvt_edges_absolute(bw);
        sr.border_widths.top = Some(e.top);
        sr.border_widths.right = Some(e.right);
        sr.border_widths.bottom = Some(e.bottom);
        sr.border_widths.left = Some(e.left);
    }
    if let Some(bc) = &s.border_color {
        sr.border_color = resolve_color(bc, cx);
    }
    if let Some(cr) = &s.corner_radii {
        let c = cvt_corners(cr);
        sr.corner_radii.top_left = Some(c.top_left);
        sr.corner_radii.top_right = Some(c.top_right);
        sr.corner_radii.bottom_right = Some(c.bottom_right);
        sr.corner_radii.bottom_left = Some(c.bottom_left);
    }

    if !s.box_shadows.is_empty() {
        sr.box_shadow = Some(
            s.box_shadows
                .iter()
                .map(|shadow| gpui::BoxShadow {
                    color: hsla(
                        shadow.color.h,
                        shadow.color.s,
                        shadow.color.l,
                        shadow.color.a,
                    ),
                    offset: gpui::Point {
                        x: px(shadow.offset.x),
                        y: px(shadow.offset.y),
                    },
                    blur_radius: px(shadow.blur),
                    spread_radius: px(shadow.spread),
                })
                .collect(),
        );
    }

    if let Some(ts) = &s.text_size {
        sr.text.font_size = Some(cvt_absolute(ts));
    }
    if let Some(tc) = &s.text_color {
        sr.text.color = resolve_color(tc, cx);
    }
    if let Some(fw) = &s.font_weight {
        sr.text.font_weight = Some(match fw {
            WitFontWeight::Thin => FontWeight::THIN,
            WitFontWeight::ExtraLight => FontWeight::EXTRA_LIGHT,
            WitFontWeight::Light => FontWeight::LIGHT,
            WitFontWeight::Normal => FontWeight::NORMAL,
            WitFontWeight::Medium => FontWeight::MEDIUM,
            WitFontWeight::SemiBold => FontWeight::SEMIBOLD,
            WitFontWeight::Bold => FontWeight::BOLD,
            WitFontWeight::ExtraBold => FontWeight::EXTRA_BOLD,
            WitFontWeight::Black => FontWeight::BLACK,
        });
    }
    if let Some(fs) = &s.font_style {
        sr.text.font_style = Some(match fs {
            WitFS::Normal => FontStyle::Normal,
            WitFS::Italic => FontStyle::Italic,
        });
    }
    if let Some(family) = &s.font_family {
        sr.text.font_family = Some(SharedString::from(family.clone()));
    }

    if let Some(ox) = &s.overflow_x {
        sr.overflow.x = Some(match ox {
            OverflowType::Visible => Overflow::Visible,
            OverflowType::Hidden => Overflow::Hidden,
            OverflowType::Scroll => Overflow::Scroll,
        });
    }
    if let Some(oy) = &s.overflow_y {
        sr.overflow.y = Some(match oy {
            OverflowType::Visible => Overflow::Visible,
            OverflowType::Hidden => Overflow::Hidden,
            OverflowType::Scroll => Overflow::Scroll,
        });
    }

    if let Some(cursor) = &s.cursor {
        sr.mouse_cursor = Some(match cursor {
            WitCursor::Default => CursorStyle::Arrow,
            WitCursor::Pointer => CursorStyle::PointingHand,
            WitCursor::Text => CursorStyle::IBeam,
            WitCursor::Crosshair => CursorStyle::Crosshair,
            WitCursor::NotAllowed => CursorStyle::OperationNotAllowed,
            WitCursor::ResizeRow | WitCursor::ResizeNs => CursorStyle::ResizeUpDown,
            WitCursor::ResizeCol | WitCursor::ResizeEw => CursorStyle::ResizeLeftRight,
            WitCursor::ResizeNesw => CursorStyle::ResizeUpRightDownLeft,
            WitCursor::ResizeNwse => CursorStyle::ResizeUpLeftDownRight,
            WitCursor::Grab => CursorStyle::OpenHand,
            WitCursor::Grabbing => CursorStyle::ClosedHand,
            WitCursor::ZoomIn | WitCursor::ZoomOut => CursorStyle::Arrow,
            WitCursor::Wait | WitCursor::Progress => CursorStyle::Arrow,
        });
    }
}

// ── Type converters ────────────────────────────────────────────────────────

fn cvt_length(l: &WitLength) -> Length {
    match l {
        WitLength::Px(v) => Length::Definite(DefiniteLength::Absolute(AbsoluteLength::Pixels(
            px(*v),
        ))),
        WitLength::Rems(v) => {
            Length::Definite(DefiniteLength::Absolute(AbsoluteLength::Rems(rems(*v))))
        }
        WitLength::Relative(v) => Length::Definite(DefiniteLength::Fraction(*v)),
        WitLength::Auto => Length::Auto,
        WitLength::Fr(_) => Length::Auto,
    }
}

fn cvt_definite(l: &WitDefiniteLength) -> DefiniteLength {
    match l {
        WitDefiniteLength::Px(v) => {
            DefiniteLength::Absolute(AbsoluteLength::Pixels(px(*v)))
        }
        WitDefiniteLength::Rems(v) => {
            DefiniteLength::Absolute(AbsoluteLength::Rems(rems(*v)))
        }
        WitDefiniteLength::Relative(v) => DefiniteLength::Fraction(*v),
    }
}

fn cvt_absolute(l: &WitAbsoluteLength) -> AbsoluteLength {
    match l {
        WitAbsoluteLength::Px(v) => AbsoluteLength::Pixels(px(*v)),
        WitAbsoluteLength::Rems(v) => AbsoluteLength::Rems(rems(*v)),
    }
}

fn cvt_edges_length(e: &WitEdgesLength) -> Edges<Length> {
    Edges {
        top: cvt_length(&e.top),
        right: cvt_length(&e.right),
        bottom: cvt_length(&e.bottom),
        left: cvt_length(&e.left),
    }
}

fn cvt_edges_definite(e: &WitEdgesLength) -> Edges<DefiniteLength> {
    let auto_zero = DefiniteLength::Absolute(AbsoluteLength::Pixels(px(0.0)));
    let to_def = |l: &WitLength| match cvt_length(l) {
        Length::Definite(d) => d,
        Length::Auto => auto_zero,
    };
    Edges {
        top: to_def(&e.top),
        right: to_def(&e.right),
        bottom: to_def(&e.bottom),
        left: to_def(&e.left),
    }
}

fn cvt_edges_absolute(e: &WitEdgesAbsolute) -> Edges<AbsoluteLength> {
    Edges {
        top: cvt_absolute(&e.top),
        right: cvt_absolute(&e.right),
        bottom: cvt_absolute(&e.bottom),
        left: cvt_absolute(&e.left),
    }
}

fn cvt_corners(c: &WitCornersAbsolute) -> Corners<AbsoluteLength> {
    Corners {
        top_left: cvt_absolute(&c.top_left),
        top_right: cvt_absolute(&c.top_right),
        bottom_right: cvt_absolute(&c.bottom_right),
        bottom_left: cvt_absolute(&c.bottom_left),
    }
}

fn cvt_align_items(a: &wit::since_v0_9_0::ui_elements::AlignItems, _default: &wit::since_v0_9_0::ui_elements::AlignItems) -> AlignItems {
    use wit::since_v0_9_0::ui_elements::AlignItems as WitAI;
    match a {
        WitAI::Start => AlignItems::Start,
        WitAI::End => AlignItems::End,
        WitAI::FlexStart => AlignItems::FlexStart,
        WitAI::FlexEnd => AlignItems::FlexEnd,
        WitAI::Center => AlignItems::Center,
        WitAI::Baseline => AlignItems::Baseline,
        WitAI::Stretch => AlignItems::Stretch,
    }
}

fn cvt_align_content(a: &wit::since_v0_9_0::ui_elements::AlignContent, _default: &wit::since_v0_9_0::ui_elements::AlignContent) -> AlignContent {
    use wit::since_v0_9_0::ui_elements::AlignContent as WitAC;
    match a {
        WitAC::Start => AlignContent::Start,
        WitAC::End => AlignContent::End,
        WitAC::FlexStart => AlignContent::FlexStart,
        WitAC::FlexEnd => AlignContent::FlexEnd,
        WitAC::Center => AlignContent::Center,
        WitAC::Stretch => AlignContent::Stretch,
        WitAC::SpaceBetween => AlignContent::SpaceBetween,
        WitAC::SpaceAround => AlignContent::SpaceAround,
        WitAC::SpaceEvenly => AlignContent::SpaceEvenly,
    }
}

fn cvt_justify_content(a: &wit::since_v0_9_0::ui_elements::JustifyContent) -> AlignContent {
    use wit::since_v0_9_0::ui_elements::JustifyContent as WitJC;
    match a {
        WitJC::Start => AlignContent::Start,
        WitJC::End => AlignContent::End,
        WitJC::FlexStart => AlignContent::FlexStart,
        WitJC::FlexEnd => AlignContent::FlexEnd,
        WitJC::Center => AlignContent::Center,
        WitJC::Stretch => AlignContent::Stretch,
        WitJC::SpaceBetween => AlignContent::SpaceBetween,
        WitJC::SpaceAround => AlignContent::SpaceAround,
        WitJC::SpaceEvenly => AlignContent::SpaceEvenly,
    }
}

fn resolve_color(c: &WitColor, cx: &App) -> Option<Hsla> {
    use wit::since_v0_9_0::ui_elements::Color;
    let colors = cx.theme().colors();
    let status = cx.theme().status();
    Some(match c {
        Color::Hsla(h) => hsla(h.h, h.s, h.l, h.a),
        Color::Transparent => return None,
        Color::Text => colors.text,
        Color::TextMuted => colors.text_muted,
        Color::TextDisabled => colors.text_disabled,
        Color::TextAccent => colors.text_accent,
        Color::TextPlaceholder => colors.text_placeholder,
        Color::Background => colors.background,
        Color::SurfaceBackground => colors.surface_background,
        Color::ElevatedSurfaceBackground => colors.elevated_surface_background,
        Color::PanelBackground => colors.panel_background,
        Color::Border => colors.border,
        Color::BorderMuted => colors.border_variant,
        Color::BorderFocused => colors.border_focused,
        Color::BorderTransparent => return None,
        Color::ElementBackground => colors.element_background,
        Color::ElementHover => colors.element_hover,
        Color::ElementSelected => colors.element_selected,
        Color::ElementActive => colors.element_active,
        Color::ElementDisabled => colors.element_disabled,
        Color::GhostElementBackground => colors.ghost_element_background,
        Color::GhostElementHover => colors.ghost_element_hover,
        Color::GhostElementSelected => colors.ghost_element_selected,
        Color::StatusError => status.error,
        Color::StatusWarning => status.warning,
        Color::StatusSuccess => status.success,
        Color::StatusInfo => status.info,
        Color::EditorBackground => colors.editor_background,
        Color::Accent => colors.text_accent,
    })
}

fn resolve_background(bg: &WitBackground, cx: &App) -> Option<Hsla> {
    use wit::since_v0_9_0::ui_elements::Background;
    match bg {
        Background::Color(c) => resolve_color(c, cx),
    }
}

fn render_uniform_list(
    n: &wit::since_v0_9_0::ui_elements::UniformListNode,
    _on_event: &(impl Fn(String, WitUiEvent, &mut Window, &mut App) + Clone + 'static),
    _focus_handles: &Arc<Mutex<std::collections::HashMap<u32, FocusHandle>>>,
    _window: &mut Window,
    _cx: &App,
) -> AnyElement {
    // UniformList rendering needs the WASM extension but we don't have it in this context
    // For now, render a placeholder - proper implementation would need architecture changes
    let _ = n;
    div()
        .child("UniformList not yet supported with focus handles")
        .into_any_element()
}

fn mouse_data_from_click(click: &ClickEvent) -> WitMouseEventData {
    match click {
        ClickEvent::Mouse(m) => WitMouseEventData {
            x: f32::from(m.down.position.x),
            y: f32::from(m.down.position.y),
            button: match m.down.button {
                gpui::MouseButton::Left => 0,
                gpui::MouseButton::Right => 1,
                gpui::MouseButton::Middle => 2,
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
