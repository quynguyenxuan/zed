use extension_host::wasm_host::wit;
use gpui::{
    AbsoluteLength, AlignContent, AlignItems, AnyElement, App, ClickEvent, Corners, CursorStyle,
    DefiniteLength, Display, Edges, ElementId, Fill, FlexDirection, FlexWrap, FontStyle,
    FontWeight, Hsla, IntoElement, Length, Overflow, Position, SharedString,
    Visibility, Window, div, hsla, px, rems, prelude::*,
};
use theme::ActiveTheme as _;

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
pub type WitUiEvent = wit::since_v0_9_0::gui::UiEvent;
type WitMouseEventData = wit::since_v0_9_0::gui::MouseEventData;

/// Converts a flat `WitUiTree` into a GPUI element hierarchy.
///
/// `on_event(source_id, event, window, cx)` is called when the user interacts
/// with any interactive element. The caller is responsible for forwarding the
/// event to the extension and re-rendering.
pub fn render_ui_tree(
    tree: &WitUiTree,
    on_event: impl Fn(String, WitUiEvent, &mut Window, &mut App) + Clone + 'static,
    cx: &App,
) -> AnyElement {
    if tree.nodes.is_empty() {
        return div().into_any_element();
    }
    render_node(&tree.nodes, tree.root, &on_event, cx)
}

fn render_node(
    nodes: &[WitUiNode],
    idx: u32,
    on_event: &(impl Fn(String, WitUiEvent, &mut Window, &mut App) + Clone + 'static),
    cx: &App,
) -> AnyElement {
    match nodes.get(idx as usize) {
        None => div().into_any_element(),
        Some(WitUiNode::Div(n)) => render_div(nodes, n, idx, on_event, cx),
        Some(WitUiNode::Text(n)) => render_text(n, cx),
        Some(WitUiNode::Svg(_)) => div().into_any_element(),
        Some(WitUiNode::Img(_)) => div().into_any_element(),
        Some(WitUiNode::Input(n)) => render_input(n, cx),
        Some(WitUiNode::UniformList(_)) => div().into_any_element(),
    }
}

fn render_div(
    nodes: &[WitUiNode],
    n: &WitDivNode,
    node_idx: u32,
    on_event: &(impl Fn(String, WitUiEvent, &mut Window, &mut App) + Clone + 'static),
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
        element = element.child(render_node(nodes, child_idx, on_event, cx));
    }

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

fn render_input(n: &WitInputNode, cx: &App) -> AnyElement {
    let mut element = div();
    apply_style(&mut element, &n.style, cx);
    let display_text = if n.value.is_empty() {
        n.placeholder.clone().unwrap_or_default()
    } else {
        n.value.clone()
    };
    element
        .child(SharedString::from(display_text))
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
