//! Virtual element builders for extension GUI panels.
//!
//! These types mirror GPUI's element API. Call [`render_tree`] to serialize
//! the tree across the WIT boundary so Zed can render it with native GPUI elements.
//!
//! # Quick start
//!
//! ```rust,no_run
//! use zed_extension_api::ui::*;
//!
//! fn gui_render(&mut self) -> zed::ui_elements::UiTree {
//!     clear_handlers();
//!     let tree = v_flex()
//!         .p_4()
//!         .gap_2()
//!         .child(Label::new("Hello"))
//!         .child(
//!             div()
//!                 .id("btn")
//!                 .px_3()
//!                 .py_1()
//!                 .bg(color_surface())
//!                 .rounded_md()
//!                 .cursor_pointer()
//!                 .on_click(|_| { /* handle click */ })
//!                 .child(Label::new("Click me")),
//!         );
//!     render_tree(tree)
//! }
//! ```

use std::cell::RefCell;
use std::collections::HashMap;

use crate::wit::zed::extension::gui;
pub use crate::wit::zed::extension::ui_elements::{
    AbsoluteLength, AlignContent, AlignItems, Background, BoxShadow, Color, CornersAbsolute,
    CursorStyle, DefiniteLength, DisplayType, DivNode, EdgesAbsolute, EdgesLength, EventFlags,
    FlexDirection, FlexWrap, FontStyle, FontWeight, Hsla, IconSource, ImgNode, InputNode,
    JustifyContent, Length, OverflowType, PositionType, Style, SvgNode, TextAlign, TextDecoration,
    TextNode, TextOverflow, UiNode, UiTree, UniformListNode, VisibilityType, WhiteSpace,
};

// ── Length constructors ────────────────────────────────────────────────────

/// Exact pixels.
pub fn px(v: f32) -> Length {
    Length::Px(v)
}
/// Relative to root font size.
pub fn rems(v: f32) -> Length {
    Length::Rems(v)
}
/// Fraction of parent size (0.0–1.0).
pub fn relative(v: f32) -> Length {
    Length::Relative(v)
}
/// Auto sizing.
pub fn auto_length() -> Length {
    Length::Auto
}

pub fn def_px(v: f32) -> DefiniteLength {
    DefiniteLength::Px(v)
}
pub fn def_rems(v: f32) -> DefiniteLength {
    DefiniteLength::Rems(v)
}
pub fn abs_px(v: f32) -> AbsoluteLength {
    AbsoluteLength::Px(v)
}
pub fn abs_rems(v: f32) -> AbsoluteLength {
    AbsoluteLength::Rems(v)
}

// ── Geometry helpers ───────────────────────────────────────────────────────

fn edges_all(v: Length) -> EdgesLength {
    EdgesLength { top: v, right: v, bottom: v, left: v }
}
fn abs_edges_all(v: AbsoluteLength) -> EdgesAbsolute {
    EdgesAbsolute { top: v, right: v, bottom: v, left: v }
}
fn corners_all(v: AbsoluteLength) -> CornersAbsolute {
    CornersAbsolute { top_left: v, top_right: v, bottom_right: v, bottom_left: v }
}

// ── Color constructors ─────────────────────────────────────────────────────

/// Create a color from raw HSLA components (each 0.0–1.0).
pub fn hsla(h: f32, s: f32, l: f32, a: f32) -> Color {
    Color::Hsla(Hsla { h, s, l, a })
}

/// Create a color from a packed 0xRRGGBB value (alpha = 1.0).
pub fn rgb(hex: u32) -> Color {
    let r = ((hex >> 16) & 0xff) as f32 / 255.0;
    let g = ((hex >> 8) & 0xff) as f32 / 255.0;
    let b = (hex & 0xff) as f32 / 255.0;
    rgb_to_color(r, g, b)
}

fn rgb_to_color(r: f32, g: f32, b: f32) -> Color {
    let max = r.max(g).max(b);
    let min = r.min(g).min(b);
    let l = (max + min) / 2.0;
    let s = if (max - min).abs() < f32::EPSILON {
        0.0
    } else if l > 0.5 {
        (max - min) / (2.0 - max - min)
    } else {
        (max - min) / (max + min)
    };
    let h = if (max - min).abs() < f32::EPSILON {
        0.0
    } else if (max - r).abs() < f32::EPSILON {
        ((g - b) / (max - min)).rem_euclid(6.0) / 6.0
    } else if (max - g).abs() < f32::EPSILON {
        ((b - r) / (max - min) + 2.0) / 6.0
    } else {
        ((r - g) / (max - min) + 4.0) / 6.0
    };
    Color::Hsla(Hsla { h, s, l, a: 1.0 })
}

// Semantic color shorthands
pub fn color_text() -> Color {
    Color::Text
}
pub fn color_muted() -> Color {
    Color::TextMuted
}
pub fn color_accent() -> Color {
    Color::Accent
}
pub fn color_border() -> Color {
    Color::Border
}
pub fn color_surface() -> Color {
    Color::SurfaceBackground
}
pub fn color_panel() -> Color {
    Color::PanelBackground
}
pub fn color_background() -> Color {
    Color::Background
}
pub fn color_error() -> Color {
    Color::StatusError
}
pub fn color_success() -> Color {
    Color::StatusSuccess
}
pub fn color_warning() -> Color {
    Color::StatusWarning
}
pub fn color_transparent() -> Color {
    Color::Transparent
}

// ── Empty style / event helpers ────────────────────────────────────────────

fn empty_style() -> Style {
    Style {
        display: None,
        position: None,
        visibility: None,
        width: None,
        height: None,
        min_width: None,
        min_height: None,
        max_width: None,
        max_height: None,
        top: None,
        right: None,
        bottom: None,
        left: None,
        flex_direction: None,
        flex_wrap: None,
        align_items: None,
        align_content: None,
        justify_content: None,
        gap: None,
        column_gap: None,
        row_gap: None,
        flex_grow: None,
        flex_shrink: None,
        flex_basis: None,
        align_self: None,
        padding: None,
        margin: None,
        background: None,
        opacity: None,
        border_widths: None,
        border_color: None,
        corner_radii: None,
        box_shadows: vec![],
        text_size: None,
        text_color: None,
        font_weight: None,
        font_style: None,
        font_family: None,
        text_align: None,
        line_height: None,
        letter_spacing: None,
        white_space: None,
        text_overflow: None,
        text_decoration: None,
        overflow_x: None,
        overflow_y: None,
        cursor: None,
        z_index: None,
        pointer_events: None,
    }
}

fn empty_events() -> EventFlags {
    EventFlags {
        on_click: false,
        on_double_click: false,
        on_right_click: false,
        on_hover: false,
        on_mouse_down: false,
        on_mouse_up: false,
        on_mouse_move: false,
        on_scroll_wheel: false,
        on_key_down: false,
        on_key_up: false,
        on_focus_in: false,
        on_focus_out: false,
        on_drag: false,
        on_drop: false,
    }
}

// ── Event handler types ────────────────────────────────────────────────────

pub struct ClickEvent {
    pub x: f32,
    pub y: f32,
    pub button: u8,
}

pub struct KeyEvent {
    pub key: String,
    pub shift: bool,
    pub ctrl: bool,
    pub alt: bool,
    pub meta: bool,
}

pub struct ScrollEvent {
    pub delta_x: f32,
    pub delta_y: f32,
    pub precise: bool,
}

pub enum UiHandler {
    Click(Box<dyn Fn(&ClickEvent)>),
    DoubleClick(Box<dyn Fn(&ClickEvent)>),
    RightClick(Box<dyn Fn(&ClickEvent)>),
    MouseMove(Box<dyn Fn(f32, f32)>),
    Hover(Box<dyn Fn(bool)>),
    ScrollWheel(Box<dyn Fn(&ScrollEvent)>),
    KeyDown(Box<dyn Fn(&KeyEvent)>),
    KeyUp(Box<dyn Fn(&KeyEvent)>),
    FocusGained(Box<dyn Fn()>),
    FocusLost(Box<dyn Fn()>),
    InputChanged(Box<dyn Fn(&str)>),
    Drop(Box<dyn Fn(&str)>),
}

thread_local! {
    static HANDLERS: RefCell<HashMap<String, Vec<UiHandler>>> =
        RefCell::new(HashMap::new());
}

/// Register an event handler for the given element ID.
/// Handlers are keyed by element ID and matched in [`dispatch_event`].
pub fn register_handler(id: impl Into<String>, handler: UiHandler) {
    HANDLERS.with(|h| {
        h.borrow_mut().entry(id.into()).or_default().push(handler);
    });
}

/// Clear all registered event handlers.
/// Call this at the start of every `gui_render` call to avoid stale handlers.
pub fn clear_handlers() {
    HANDLERS.with(|h| h.borrow_mut().clear());
}

/// Dispatch an incoming event to the registered handlers for `source_id`.
/// Called automatically by the generated `gui_on_event` dispatch in
/// `extension_api.rs` when you override `gui_on_event` via `dispatch_event`.
pub fn dispatch_event(source_id: &str, event: &gui::UiEvent) {
    HANDLERS.with(|h| {
        let borrowed = h.borrow();
        let Some(handlers) = borrowed.get(source_id) else {
            return;
        };
        for handler in handlers {
            match (handler, event) {
                (UiHandler::Click(f), gui::UiEvent::Clicked(e)) => {
                    f(&ClickEvent { x: e.x, y: e.y, button: e.button })
                }
                (UiHandler::DoubleClick(f), gui::UiEvent::DoubleClicked(e)) => {
                    f(&ClickEvent { x: e.x, y: e.y, button: e.button })
                }
                (UiHandler::RightClick(f), gui::UiEvent::RightClicked(e)) => {
                    f(&ClickEvent { x: e.x, y: e.y, button: e.button })
                }
                (UiHandler::Hover(f), gui::UiEvent::HoverStart) => f(true),
                (UiHandler::Hover(f), gui::UiEvent::HoverEnd) => f(false),
                (UiHandler::ScrollWheel(f), gui::UiEvent::ScrollWheel(e)) => {
                    f(&ScrollEvent { delta_x: e.delta_x, delta_y: e.delta_y, precise: e.precise })
                }
                (UiHandler::KeyDown(f), gui::UiEvent::KeyDown(e)) => {
                    f(&KeyEvent {
                        key: e.key.clone(),
                        shift: e.shift,
                        ctrl: e.ctrl,
                        alt: e.alt,
                        meta: e.meta,
                    })
                }
                (UiHandler::KeyUp(f), gui::UiEvent::KeyUp(e)) => {
                    f(&KeyEvent {
                        key: e.key.clone(),
                        shift: e.shift,
                        ctrl: e.ctrl,
                        alt: e.alt,
                        meta: e.meta,
                    })
                }
                (UiHandler::MouseMove(f), gui::UiEvent::MouseMoved(e)) => f(e.x, e.y),
                (UiHandler::FocusGained(f), gui::UiEvent::FocusGained) => f(),
                (UiHandler::FocusLost(f), gui::UiEvent::FocusLost) => f(),
                (UiHandler::InputChanged(f), gui::UiEvent::InputChanged(s)) => f(s),
                (UiHandler::Drop(f), gui::UiEvent::Dropped(s)) => f(s),
                _ => {}
            }
        }
    });
}

// ── Flat tree builder ──────────────────────────────────────────────────────

/// Trait for anything that can become a child node in the flat WIT tree.
pub trait IntoAnyNode {
    fn into_any_node(self) -> AnyNode;
}

/// Internal enum holding any builder node type before flattening to `UiTree`.
pub enum AnyNode {
    Div(Div),
    Text(String, Style),
    Img(Option<String>, String, Style, EventFlags),
    Svg(IconSource, Option<Color>, Style),
    Input(String, String, Option<String>, bool, Style),
    UniformList(String, u32, f32, Style, bool),
}

/// Flatten a root `Div` into a `UiTree` suitable for passing over the WIT
/// boundary. Call this from your `gui_render` implementation.
pub fn render_tree(root: Div) -> UiTree {
    let mut nodes: Vec<UiNode> = Vec::new();
    let root_idx = flatten_node(&mut nodes, AnyNode::Div(root));
    UiTree { nodes, root: root_idx }
}

fn flatten_node(nodes: &mut Vec<UiNode>, node: AnyNode) -> u32 {
    let idx = nodes.len() as u32;
    // Reserve slot so children can reference the correct indices.
    nodes.push(UiNode::Text(TextNode {
        content: String::new(),
        style: empty_style(),
    }));
    let wit_node = match node {
        AnyNode::Div(div) => {
            let children: Vec<u32> = div
                .children
                .into_iter()
                .map(|child| flatten_node(nodes, child))
                .collect();
            UiNode::Div(DivNode {
                id: div.id,
                focus_handle_id: div.focus_handle_id,
                style: div.style,
                events: div.events,
                tooltip: div.tooltip,
                children,
            })
        }
        AnyNode::Text(content, style) => UiNode::Text(TextNode { content, style }),
        AnyNode::Img(id, src, style, events) => UiNode::Img(ImgNode { id, src, style, events }),
        AnyNode::Svg(source, color, style) => UiNode::Svg(SvgNode { source, color, style }),
        AnyNode::Input(id, value, placeholder, disabled, style) => {
            UiNode::Input(InputNode { id, value, placeholder, disabled, style })
        }
        AnyNode::UniformList(id, item_count, item_height, style, fill_width) => {
            UiNode::UniformList(UniformListNode {
                id,
                item_count,
                item_height,
                style,
                fill_width,
            })
        }
    };
    nodes[idx as usize] = wit_node;
    idx
}

// ── Div builder ────────────────────────────────────────────────────────────

pub struct Div {
    id: Option<String>,
    focus_handle_id: Option<u32>,
    style: Style,
    events: EventFlags,
    tooltip: Option<String>,
    children: Vec<AnyNode>,
}

/// Create a new block-level div.
pub fn div() -> Div {
    Div {
        id: None,
        focus_handle_id: None,
        style: empty_style(),
        events: empty_events(),
        tooltip: None,
        children: vec![],
    }
}

/// Flex column — mirrors `gpui::v_flex()`.
pub fn v_flex() -> Div {
    div().flex().flex_col()
}

/// Flex row — mirrors `gpui::h_flex()`.
pub fn h_flex() -> Div {
    div().flex().flex_row()
}

impl IntoAnyNode for Div {
    fn into_any_node(self) -> AnyNode {
        AnyNode::Div(self)
    }
}

impl Div {
    // ── Identity ──────────────────────────────────────────────────────────

    pub fn id(mut self, id: impl Into<String>) -> Self {
        self.id = Some(id.into());
        self
    }

    /// Allocate a host-managed focus handle, making this element focusable.
    pub fn focusable(mut self) -> Self {
        let handle_id = gui::create_focus_handle();
        self.focus_handle_id = Some(handle_id);
        self
    }

    pub fn tooltip(mut self, text: impl Into<String>) -> Self {
        self.tooltip = Some(text.into());
        self
    }

    // ── Children ──────────────────────────────────────────────────────────

    pub fn child(mut self, child: impl IntoAnyNode) -> Self {
        self.children.push(child.into_any_node());
        self
    }

    pub fn children(mut self, iter: impl IntoIterator<Item = impl IntoAnyNode>) -> Self {
        self.children.extend(iter.into_iter().map(|c| c.into_any_node()));
        self
    }

    pub fn when(self, condition: bool, f: impl FnOnce(Self) -> Self) -> Self {
        if condition { f(self) } else { self }
    }

    pub fn when_some<T>(self, option: Option<T>, f: impl FnOnce(Self, T) -> Self) -> Self {
        match option {
            Some(v) => f(self, v),
            None => self,
        }
    }

    // ── Event handlers ────────────────────────────────────────────────────

    pub fn on_click(mut self, f: impl Fn(&ClickEvent) + 'static) -> Self {
        self.events.on_click = true;
        if let Some(id) = &self.id {
            register_handler(id.clone(), UiHandler::Click(Box::new(f)));
        }
        self
    }

    pub fn on_double_click(mut self, f: impl Fn(&ClickEvent) + 'static) -> Self {
        self.events.on_double_click = true;
        if let Some(id) = &self.id {
            register_handler(id.clone(), UiHandler::DoubleClick(Box::new(f)));
        }
        self
    }

    pub fn on_right_click(mut self, f: impl Fn(&ClickEvent) + 'static) -> Self {
        self.events.on_right_click = true;
        if let Some(id) = &self.id {
            register_handler(id.clone(), UiHandler::RightClick(Box::new(f)));
        }
        self
    }

    pub fn on_hover(mut self, f: impl Fn(bool) + 'static) -> Self {
        self.events.on_hover = true;
        if let Some(id) = &self.id {
            register_handler(id.clone(), UiHandler::Hover(Box::new(f)));
        }
        self
    }

    pub fn on_scroll_wheel(mut self, f: impl Fn(&ScrollEvent) + 'static) -> Self {
        self.events.on_scroll_wheel = true;
        if let Some(id) = &self.id {
            register_handler(id.clone(), UiHandler::ScrollWheel(Box::new(f)));
        }
        self
    }

    pub fn on_key_down(mut self, f: impl Fn(&KeyEvent) + 'static) -> Self {
        self.events.on_key_down = true;
        if let Some(id) = &self.id {
            register_handler(id.clone(), UiHandler::KeyDown(Box::new(f)));
        }
        self
    }

    // ── Layout ────────────────────────────────────────────────────────────

    pub fn flex(mut self) -> Self {
        self.style.display = Some(DisplayType::Flex);
        self
    }
    pub fn block(mut self) -> Self {
        self.style.display = Some(DisplayType::Block);
        self
    }
    pub fn flex_col(mut self) -> Self {
        self.style.flex_direction = Some(FlexDirection::Column);
        self
    }
    pub fn flex_row(mut self) -> Self {
        self.style.flex_direction = Some(FlexDirection::Row);
        self
    }
    pub fn flex_col_reverse(mut self) -> Self {
        self.style.flex_direction = Some(FlexDirection::ColumnReverse);
        self
    }
    pub fn flex_row_reverse(mut self) -> Self {
        self.style.flex_direction = Some(FlexDirection::RowReverse);
        self
    }
    pub fn flex_wrap(mut self) -> Self {
        self.style.flex_wrap = Some(FlexWrap::Wrap);
        self
    }
    pub fn flex_1(mut self) -> Self {
        self.style.flex_grow = Some(1.0);
        self.style.flex_shrink = Some(1.0);
        self.style.flex_basis = Some(Length::Relative(0.0));
        self
    }
    pub fn flex_grow(mut self) -> Self {
        self.style.flex_grow = Some(1.0);
        self
    }
    pub fn flex_shrink(mut self) -> Self {
        self.style.flex_shrink = Some(1.0);
        self
    }
    pub fn flex_none(mut self) -> Self {
        self.style.flex_grow = Some(0.0);
        self.style.flex_shrink = Some(0.0);
        self
    }
    pub fn relative(mut self) -> Self {
        self.style.position = Some(PositionType::Relative);
        self
    }
    pub fn absolute(mut self) -> Self {
        self.style.position = Some(PositionType::Absolute);
        self
    }
    pub fn inset_0(mut self) -> Self {
        self.style.top = Some(Length::Px(0.0));
        self.style.right = Some(Length::Px(0.0));
        self.style.bottom = Some(Length::Px(0.0));
        self.style.left = Some(Length::Px(0.0));
        self
    }
    pub fn top(mut self, v: Length) -> Self {
        self.style.top = Some(v);
        self
    }
    pub fn bottom(mut self, v: Length) -> Self {
        self.style.bottom = Some(v);
        self
    }
    pub fn left(mut self, v: Length) -> Self {
        self.style.left = Some(v);
        self
    }
    pub fn right(mut self, v: Length) -> Self {
        self.style.right = Some(v);
        self
    }

    pub fn items_start(mut self) -> Self {
        self.style.align_items = Some(AlignItems::Start);
        self
    }
    pub fn items_end(mut self) -> Self {
        self.style.align_items = Some(AlignItems::End);
        self
    }
    pub fn items_center(mut self) -> Self {
        self.style.align_items = Some(AlignItems::Center);
        self
    }
    pub fn items_baseline(mut self) -> Self {
        self.style.align_items = Some(AlignItems::Baseline);
        self
    }
    pub fn items_stretch(mut self) -> Self {
        self.style.align_items = Some(AlignItems::Stretch);
        self
    }
    pub fn justify_start(mut self) -> Self {
        self.style.justify_content = Some(JustifyContent::Start);
        self
    }
    pub fn justify_end(mut self) -> Self {
        self.style.justify_content = Some(JustifyContent::End);
        self
    }
    pub fn justify_center(mut self) -> Self {
        self.style.justify_content = Some(JustifyContent::Center);
        self
    }
    pub fn justify_between(mut self) -> Self {
        self.style.justify_content = Some(JustifyContent::SpaceBetween);
        self
    }
    pub fn justify_around(mut self) -> Self {
        self.style.justify_content = Some(JustifyContent::SpaceAround);
        self
    }

    // ── Size ──────────────────────────────────────────────────────────────

    pub fn w(mut self, v: Length) -> Self {
        self.style.width = Some(v);
        self
    }
    pub fn h(mut self, v: Length) -> Self {
        self.style.height = Some(v);
        self
    }
    pub fn size(mut self, v: Length) -> Self {
        self.style.width = Some(v);
        self.style.height = Some(v);
        self
    }
    pub fn w_full(mut self) -> Self {
        self.style.width = Some(Length::Relative(1.0));
        self
    }
    pub fn h_full(mut self) -> Self {
        self.style.height = Some(Length::Relative(1.0));
        self
    }
    pub fn size_full(mut self) -> Self {
        self.style.width = Some(Length::Relative(1.0));
        self.style.height = Some(Length::Relative(1.0));
        self
    }
    pub fn min_w(mut self, v: DefiniteLength) -> Self {
        self.style.min_width = Some(v);
        self
    }
    pub fn min_h(mut self, v: DefiniteLength) -> Self {
        self.style.min_height = Some(v);
        self
    }
    pub fn max_w(mut self, v: DefiniteLength) -> Self {
        self.style.max_width = Some(v);
        self
    }
    pub fn max_h(mut self, v: DefiniteLength) -> Self {
        self.style.max_height = Some(v);
        self
    }

    // ── Gap ───────────────────────────────────────────────────────────────

    pub fn gap(mut self, v: DefiniteLength) -> Self {
        self.style.gap = Some(v);
        self
    }
    pub fn gap_x(mut self, v: DefiniteLength) -> Self {
        self.style.column_gap = Some(v);
        self
    }
    pub fn gap_y(mut self, v: DefiniteLength) -> Self {
        self.style.row_gap = Some(v);
        self
    }
    pub fn gap_0(self) -> Self { self.gap(DefiniteLength::Px(0.0)) }
    pub fn gap_0_5(self) -> Self { self.gap(DefiniteLength::Rems(0.125)) }
    pub fn gap_1(self) -> Self { self.gap(DefiniteLength::Rems(0.25)) }
    pub fn gap_1_5(self) -> Self { self.gap(DefiniteLength::Rems(0.375)) }
    pub fn gap_2(self) -> Self { self.gap(DefiniteLength::Rems(0.5)) }
    pub fn gap_3(self) -> Self { self.gap(DefiniteLength::Rems(0.75)) }
    pub fn gap_4(self) -> Self { self.gap(DefiniteLength::Rems(1.0)) }
    pub fn gap_5(self) -> Self { self.gap(DefiniteLength::Rems(1.25)) }
    pub fn gap_6(self) -> Self { self.gap(DefiniteLength::Rems(1.5)) }
    pub fn gap_8(self) -> Self { self.gap(DefiniteLength::Rems(2.0)) }

    // ── Padding ───────────────────────────────────────────────────────────

    pub fn p(mut self, v: Length) -> Self {
        self.style.padding = Some(edges_all(v));
        self
    }
    pub fn px(mut self, v: Length) -> Self {
        let current = self.style.padding.take().unwrap_or_else(|| edges_all(Length::Px(0.0)));
        self.style.padding = Some(EdgesLength { left: v, right: v, ..current });
        self
    }
    pub fn py(mut self, v: Length) -> Self {
        let current = self.style.padding.take().unwrap_or_else(|| edges_all(Length::Px(0.0)));
        self.style.padding = Some(EdgesLength { top: v, bottom: v, ..current });
        self
    }
    pub fn pt(mut self, v: Length) -> Self {
        let current = self.style.padding.take().unwrap_or_else(|| edges_all(Length::Px(0.0)));
        self.style.padding = Some(EdgesLength { top: v, ..current });
        self
    }
    pub fn pb(mut self, v: Length) -> Self {
        let current = self.style.padding.take().unwrap_or_else(|| edges_all(Length::Px(0.0)));
        self.style.padding = Some(EdgesLength { bottom: v, ..current });
        self
    }
    pub fn pl(mut self, v: Length) -> Self {
        let current = self.style.padding.take().unwrap_or_else(|| edges_all(Length::Px(0.0)));
        self.style.padding = Some(EdgesLength { left: v, ..current });
        self
    }
    pub fn pr(mut self, v: Length) -> Self {
        let current = self.style.padding.take().unwrap_or_else(|| edges_all(Length::Px(0.0)));
        self.style.padding = Some(EdgesLength { right: v, ..current });
        self
    }

    pub fn p_0(self) -> Self { self.p(Length::Px(0.0)) }
    pub fn p_0_5(self) -> Self { self.p(Length::Rems(0.125)) }
    pub fn p_1(self) -> Self { self.p(Length::Rems(0.25)) }
    pub fn p_1_5(self) -> Self { self.p(Length::Rems(0.375)) }
    pub fn p_2(self) -> Self { self.p(Length::Rems(0.5)) }
    pub fn p_3(self) -> Self { self.p(Length::Rems(0.75)) }
    pub fn p_4(self) -> Self { self.p(Length::Rems(1.0)) }
    pub fn p_5(self) -> Self { self.p(Length::Rems(1.25)) }
    pub fn p_6(self) -> Self { self.p(Length::Rems(1.5)) }
    pub fn p_8(self) -> Self { self.p(Length::Rems(2.0)) }

    pub fn px_0(self) -> Self { self.px(Length::Px(0.0)) }
    pub fn px_0_5(self) -> Self { self.px(Length::Rems(0.125)) }
    pub fn px_1(self) -> Self { self.px(Length::Rems(0.25)) }
    pub fn px_2(self) -> Self { self.px(Length::Rems(0.5)) }
    pub fn px_3(self) -> Self { self.px(Length::Rems(0.75)) }
    pub fn px_4(self) -> Self { self.px(Length::Rems(1.0)) }
    pub fn px_5(self) -> Self { self.px(Length::Rems(1.25)) }
    pub fn px_6(self) -> Self { self.px(Length::Rems(1.5)) }
    pub fn px_8(self) -> Self { self.px(Length::Rems(2.0)) }

    pub fn py_0(self) -> Self { self.py(Length::Px(0.0)) }
    pub fn py_0_5(self) -> Self { self.py(Length::Rems(0.125)) }
    pub fn py_1(self) -> Self { self.py(Length::Rems(0.25)) }
    pub fn py_2(self) -> Self { self.py(Length::Rems(0.5)) }
    pub fn py_3(self) -> Self { self.py(Length::Rems(0.75)) }
    pub fn py_4(self) -> Self { self.py(Length::Rems(1.0)) }

    pub fn pt_1(self) -> Self { self.pt(Length::Rems(0.25)) }
    pub fn pt_2(self) -> Self { self.pt(Length::Rems(0.5)) }
    pub fn pt_3(self) -> Self { self.pt(Length::Rems(0.75)) }
    pub fn pt_4(self) -> Self { self.pt(Length::Rems(1.0)) }
    pub fn pb_1(self) -> Self { self.pb(Length::Rems(0.25)) }
    pub fn pb_2(self) -> Self { self.pb(Length::Rems(0.5)) }
    pub fn pb_4(self) -> Self { self.pb(Length::Rems(1.0)) }
    pub fn pl_1(self) -> Self { self.pl(Length::Rems(0.25)) }
    pub fn pl_2(self) -> Self { self.pl(Length::Rems(0.5)) }
    pub fn pl_4(self) -> Self { self.pl(Length::Rems(1.0)) }
    pub fn pr_1(self) -> Self { self.pr(Length::Rems(0.25)) }
    pub fn pr_2(self) -> Self { self.pr(Length::Rems(0.5)) }
    pub fn pr_4(self) -> Self { self.pr(Length::Rems(1.0)) }

    // ── Margin ────────────────────────────────────────────────────────────

    pub fn m(mut self, v: Length) -> Self {
        self.style.margin = Some(edges_all(v));
        self
    }
    pub fn mx(mut self, v: Length) -> Self {
        let current = self.style.margin.take().unwrap_or_else(|| edges_all(Length::Px(0.0)));
        self.style.margin = Some(EdgesLength { left: v, right: v, ..current });
        self
    }
    pub fn my(mut self, v: Length) -> Self {
        let current = self.style.margin.take().unwrap_or_else(|| edges_all(Length::Px(0.0)));
        self.style.margin = Some(EdgesLength { top: v, bottom: v, ..current });
        self
    }
    pub fn mt(mut self, v: Length) -> Self {
        let current = self.style.margin.take().unwrap_or_else(|| edges_all(Length::Px(0.0)));
        self.style.margin = Some(EdgesLength { top: v, ..current });
        self
    }
    pub fn mb(mut self, v: Length) -> Self {
        let current = self.style.margin.take().unwrap_or_else(|| edges_all(Length::Px(0.0)));
        self.style.margin = Some(EdgesLength { bottom: v, ..current });
        self
    }

    pub fn m_0(self) -> Self { self.m(Length::Px(0.0)) }
    pub fn m_1(self) -> Self { self.m(Length::Rems(0.25)) }
    pub fn m_2(self) -> Self { self.m(Length::Rems(0.5)) }
    pub fn m_4(self) -> Self { self.m(Length::Rems(1.0)) }
    pub fn mx_auto(mut self) -> Self {
        let current = self.style.margin.take().unwrap_or_else(|| edges_all(Length::Px(0.0)));
        self.style.margin = Some(EdgesLength { left: Length::Auto, right: Length::Auto, ..current });
        self
    }
    pub fn mt_0(self) -> Self { self.mt(Length::Px(0.0)) }
    pub fn mt_1(self) -> Self { self.mt(Length::Rems(0.25)) }
    pub fn mt_2(self) -> Self { self.mt(Length::Rems(0.5)) }
    pub fn mt_4(self) -> Self { self.mt(Length::Rems(1.0)) }
    pub fn mb_1(self) -> Self { self.mb(Length::Rems(0.25)) }
    pub fn mb_2(self) -> Self { self.mb(Length::Rems(0.5)) }
    pub fn mb_4(self) -> Self { self.mb(Length::Rems(1.0)) }

    // ── Background / color ────────────────────────────────────────────────

    pub fn bg(mut self, color: Color) -> Self {
        self.style.background = Some(Background::Color(color));
        self
    }
    pub fn opacity(mut self, v: f32) -> Self {
        self.style.opacity = Some(v);
        self
    }

    // ── Border ────────────────────────────────────────────────────────────

    pub fn border(mut self, v: AbsoluteLength) -> Self {
        self.style.border_widths = Some(abs_edges_all(v));
        self
    }
    pub fn border_1(self) -> Self { self.border(AbsoluteLength::Px(1.0)) }
    pub fn border_2(self) -> Self { self.border(AbsoluteLength::Px(2.0)) }
    pub fn border_color(mut self, color: Color) -> Self {
        self.style.border_color = Some(color);
        self
    }

    pub fn rounded(mut self, v: AbsoluteLength) -> Self {
        self.style.corner_radii = Some(corners_all(v));
        self
    }
    pub fn rounded_sm(self) -> Self { self.rounded(AbsoluteLength::Px(2.0)) }
    pub fn rounded_md(self) -> Self { self.rounded(AbsoluteLength::Px(6.0)) }
    pub fn rounded_lg(self) -> Self { self.rounded(AbsoluteLength::Px(8.0)) }
    pub fn rounded_xl(self) -> Self { self.rounded(AbsoluteLength::Px(12.0)) }
    pub fn rounded_full(self) -> Self { self.rounded(AbsoluteLength::Px(9999.0)) }

    // ── Shadow ────────────────────────────────────────────────────────────

    pub fn shadow(mut self, shadow: BoxShadow) -> Self {
        self.style.box_shadows.push(shadow);
        self
    }

    // ── Text ──────────────────────────────────────────────────────────────

    pub fn text_color(mut self, color: Color) -> Self {
        self.style.text_color = Some(color);
        self
    }
    pub fn text_size(mut self, v: AbsoluteLength) -> Self {
        self.style.text_size = Some(v);
        self
    }
    pub fn text_xs(self) -> Self { self.text_size(AbsoluteLength::Rems(0.75)) }
    pub fn text_sm(self) -> Self { self.text_size(AbsoluteLength::Rems(0.875)) }
    pub fn text_base(self) -> Self { self.text_size(AbsoluteLength::Rems(1.0)) }
    pub fn text_lg(self) -> Self { self.text_size(AbsoluteLength::Rems(1.125)) }
    pub fn text_xl(self) -> Self { self.text_size(AbsoluteLength::Rems(1.25)) }
    pub fn text_2xl(self) -> Self { self.text_size(AbsoluteLength::Rems(1.5)) }
    pub fn font_bold(mut self) -> Self {
        self.style.font_weight = Some(FontWeight::Bold);
        self
    }
    pub fn font_medium(mut self) -> Self {
        self.style.font_weight = Some(FontWeight::Medium);
        self
    }
    pub fn font_light(mut self) -> Self {
        self.style.font_weight = Some(FontWeight::Light);
        self
    }
    pub fn italic(mut self) -> Self {
        self.style.font_style = Some(FontStyle::Italic);
        self
    }
    pub fn text_center(mut self) -> Self {
        self.style.text_align = Some(TextAlign::Center);
        self
    }
    pub fn text_ellipsis(mut self) -> Self {
        self.style.text_overflow = Some(TextOverflow::Ellipsis);
        self
    }
    pub fn whitespace_nowrap(mut self) -> Self {
        self.style.white_space = Some(WhiteSpace::Nowrap);
        self
    }
    pub fn underline(mut self) -> Self {
        self.style.text_decoration = Some(TextDecoration::Underline);
        self
    }
    pub fn line_through(mut self) -> Self {
        self.style.text_decoration = Some(TextDecoration::Strikethrough);
        self
    }

    // ── Overflow ──────────────────────────────────────────────────────────

    pub fn overflow_hidden(mut self) -> Self {
        self.style.overflow_x = Some(OverflowType::Hidden);
        self.style.overflow_y = Some(OverflowType::Hidden);
        self
    }
    pub fn overflow_x_hidden(mut self) -> Self {
        self.style.overflow_x = Some(OverflowType::Hidden);
        self
    }
    pub fn overflow_y_hidden(mut self) -> Self {
        self.style.overflow_y = Some(OverflowType::Hidden);
        self
    }
    pub fn overflow_scroll(mut self) -> Self {
        self.style.overflow_x = Some(OverflowType::Scroll);
        self.style.overflow_y = Some(OverflowType::Scroll);
        self
    }
    pub fn overflow_y_scroll(mut self) -> Self {
        self.style.overflow_y = Some(OverflowType::Scroll);
        self
    }

    // ── Misc ──────────────────────────────────────────────────────────────

    pub fn cursor_pointer(mut self) -> Self {
        self.style.cursor = Some(CursorStyle::Pointer);
        self
    }
    pub fn cursor_default(mut self) -> Self {
        self.style.cursor = Some(CursorStyle::Default);
        self
    }
    pub fn cursor_text(mut self) -> Self {
        self.style.cursor = Some(CursorStyle::Text);
        self
    }
    pub fn z_index(mut self, z: u32) -> Self {
        self.style.z_index = Some(z);
        self
    }
    pub fn pointer_events_none(mut self) -> Self {
        self.style.pointer_events = Some(false);
        self
    }
    pub fn invisible(mut self) -> Self {
        self.style.visibility = Some(VisibilityType::Hidden);
        self
    }
}

// ── Label ──────────────────────────────────────────────────────────────────

/// A styled text node, mirrors `gpui::Label`.
pub struct Label {
    text: String,
    style: Style,
}

impl Label {
    pub fn new(text: impl Into<String>) -> Self {
        Self { text: text.into(), style: empty_style() }
    }

    pub fn color(mut self, color: Color) -> Self {
        self.style.text_color = Some(color);
        self
    }
    pub fn size(mut self, v: AbsoluteLength) -> Self {
        self.style.text_size = Some(v);
        self
    }
    pub fn text_sm(mut self) -> Self {
        self.style.text_size = Some(AbsoluteLength::Rems(0.875));
        self
    }
    pub fn text_xs(mut self) -> Self {
        self.style.text_size = Some(AbsoluteLength::Rems(0.75));
        self
    }
    pub fn bold(mut self) -> Self {
        self.style.font_weight = Some(FontWeight::Bold);
        self
    }
    pub fn italic(mut self) -> Self {
        self.style.font_style = Some(FontStyle::Italic);
        self
    }
    pub fn muted(mut self) -> Self {
        self.style.text_color = Some(Color::TextMuted);
        self
    }
    pub fn single_line(mut self) -> Self {
        self.style.white_space = Some(WhiteSpace::Nowrap);
        self.style.text_overflow = Some(TextOverflow::Ellipsis);
        self
    }
}

impl IntoAnyNode for Label {
    fn into_any_node(self) -> AnyNode {
        AnyNode::Text(self.text, self.style)
    }
}

// ── Icon ───────────────────────────────────────────────────────────────────

/// An SVG icon node, mirrors `gpui::Icon`.
pub struct Icon {
    source: IconSource,
    color: Option<Color>,
    style: Style,
}

impl Icon {
    /// Named icon from Zed's built-in icon set (e.g. `"check"`, `"folder"`).
    pub fn named(name: impl Into<String>) -> Self {
        Self { source: IconSource::Named(name.into()), color: None, style: empty_style() }
    }

    /// Custom SVG from the extension's asset bundle.
    pub fn path(path: impl Into<String>) -> Self {
        Self { source: IconSource::Path(path.into()), color: None, style: empty_style() }
    }

    pub fn color(mut self, color: Color) -> Self {
        self.color = Some(color);
        self
    }
    pub fn size(mut self, v: Length) -> Self {
        self.style.width = Some(v);
        self.style.height = Some(v);
        self
    }
}

impl IntoAnyNode for Icon {
    fn into_any_node(self) -> AnyNode {
        AnyNode::Svg(self.source, self.color, self.style)
    }
}

// ── UniformList ────────────────────────────────────────────────────────────

/// A virtualized list — the host calls `gui_render_list_item` per visible item.
pub struct UniformList {
    id: String,
    item_count: u32,
    item_height: f32,
    style: Style,
    fill_width: bool,
}

impl UniformList {
    pub fn new(id: impl Into<String>, item_count: u32, item_height: f32) -> Self {
        Self {
            id: id.into(),
            item_count,
            item_height,
            style: empty_style(),
            fill_width: true,
        }
    }

    pub fn fill_width(mut self, v: bool) -> Self {
        self.fill_width = v;
        self
    }
    pub fn h_full(mut self) -> Self {
        self.style.height = Some(Length::Relative(1.0));
        self
    }
    pub fn w_full(mut self) -> Self {
        self.style.width = Some(Length::Relative(1.0));
        self
    }
}

impl IntoAnyNode for UniformList {
    fn into_any_node(self) -> AnyNode {
        AnyNode::UniformList(
            self.id,
            self.item_count,
            self.item_height,
            self.style,
            self.fill_width,
        )
    }
}

// ── Input ──────────────────────────────────────────────────────────────────

/// A text input field.
pub struct Input {
    id: String,
    value: String,
    placeholder: Option<String>,
    disabled: bool,
    style: Style,
}

impl Input {
    pub fn new(id: impl Into<String>, value: impl Into<String>) -> Self {
        Self {
            id: id.into(),
            value: value.into(),
            placeholder: None,
            disabled: false,
            style: empty_style(),
        }
    }

    pub fn placeholder(mut self, text: impl Into<String>) -> Self {
        self.placeholder = Some(text.into());
        self
    }

    pub fn disabled(mut self, disabled: bool) -> Self {
        self.disabled = disabled;
        self
    }

    pub fn on_input(self, f: impl Fn(&str) + 'static) -> Self {
        register_handler(&self.id, UiHandler::InputChanged(Box::new(f)));
        self
    }

    // Style methods
    pub fn w_full(mut self) -> Self {
        self.style.width = Some(Length::Relative(1.0));
        self
    }
    pub fn px(mut self, v: Length) -> Self {
        let current = self.style.padding.take().unwrap_or_else(|| edges_all(Length::Px(0.0)));
        self.style.padding = Some(EdgesLength { left: v, right: v, ..current });
        self
    }
    pub fn py(mut self, v: Length) -> Self {
        let current = self.style.padding.take().unwrap_or_else(|| edges_all(Length::Px(0.0)));
        self.style.padding = Some(EdgesLength { top: v, bottom: v, ..current });
        self
    }
    pub fn p_2(self) -> Self {
        let mut s = self;
        s.style.padding = Some(edges_all(Length::Rems(0.5)));
        s
    }
    pub fn rounded_md(mut self) -> Self {
        self.style.corner_radii = Some(corners_all(AbsoluteLength::Px(6.0)));
        self
    }
    pub fn border_1(mut self) -> Self {
        self.style.border_widths = Some(abs_edges_all(AbsoluteLength::Px(1.0)));
        self
    }
    pub fn border_color(mut self, color: Color) -> Self {
        self.style.border_color = Some(color);
        self
    }
    pub fn bg(mut self, color: Color) -> Self {
        self.style.background = Some(Background::Color(color));
        self
    }
    pub fn text_sm(mut self) -> Self {
        self.style.text_size = Some(AbsoluteLength::Rems(0.875));
        self
    }
}

impl IntoAnyNode for Input {
    fn into_any_node(self) -> AnyNode {
        AnyNode::Input(self.id, self.value, self.placeholder, self.disabled, self.style)
    }
}

// ── String convenience ─────────────────────────────────────────────────────

impl IntoAnyNode for &str {
    fn into_any_node(self) -> AnyNode {
        AnyNode::Text(self.to_owned(), empty_style())
    }
}

impl IntoAnyNode for String {
    fn into_any_node(self) -> AnyNode {
        AnyNode::Text(self, empty_style())
    }
}
