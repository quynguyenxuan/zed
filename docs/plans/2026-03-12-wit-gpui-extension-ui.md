# WIT Structured Types — GPUI-Compatible Extension UI

**Date:** 2026-03-12
**Branch:** feature/gui-extension

## Goal

Allow Zed extensions to define UI using an API that mirrors GPUI's native element system
(`div()`, `v_flex()`, `Label::new()`, `Button::new()`, style method chaining). The extension
runs as `wasm32-wasi` inside wasmtime. The host receives a WIT-serialized element tree and
renders it using real GPUI — no pixel transfer, no subprocess, native look-and-feel, automatic
theme support.

---

## Architecture

```
Extension (wasm32-wasi, wasmtime)
  zed_extension_api::ui
    div().flex_col().p_4().gap_2()
      .child(Label::new("Hello"))
      .child(Button::new("btn", "Click").on_click(|_| { ... }))
    → VirtualElement tree (in-memory Rust structs)
    → serialize to WIT UiNode types
    → gui::set-view-tree(root: ui-node)  [WIT import]

                    │ WIT boundary (same process, memory copy)
                    ▼

Host (Zed, extension_panel crate)
  GuiPanelMessage::SetViewTree(UiNode)
    → ui_renderer::render_node(&node, cx)
    → real GPUI elements with real theme colors
    → ExtensionGuiView::render() returns them
    → GPUI renders natively at 60fps

Events (GPUI → WASM):
  user clicks element with id="btn"
    → GPUI on_click handler fires
    → host calls wasm_extension.call_gui_on_event("btn", UiEvent::Clicked)
    → extension dispatches to registered closure
    → closure mutates state
    → extension calls set_view_tree(new_root)   [pull: host calls gui-render export]
```

---

## Render model: Push vs Pull

**Push (extension calls set-view-tree):**
Extension calls `set_view_tree()` explicitly after state changes. Simple but requires
extension to know when to re-render.

**Pull (host calls gui-render export after each event):**
Host calls `gui-render() -> ui-node` after every event. Extension always returns current
state. Cleaner reactive model.

**Decision: hybrid.** Extension can call `set_view_tree()` at any time (e.g. on init,
after async data arrives). Host also calls `gui-render()` after every `gui-on-event`.
This covers both imperative and reactive patterns.

---

## Phase overview

| Phase | Scope | Key deliverable |
|-------|-------|-----------------|
| 1 | WIT types: style + basic elements | `ui-elements.wit` |
| 2 | Semantic color system | `Color` variant in WIT, host maps to theme |
| 3 | Focus system | `create-focus-handle` host function, per-element focus |
| 4 | Full event system | Expanded `UiEvent`, typed event payloads |
| 5 | Extension API (`zed_extension_api::ui`) | Virtual element builders mirroring GPUI |
| 6 | Host renderer | WIT → GPUI element conversion |
| 7 | Virtual list | `uniform-list-node`, `gui-render-list-item` export |
| 8 | UI component library | `Icon`, `Divider`, `Toggle`, `Tooltip`, `Progress` |
| 9 | State management | Pull model, `gui-render` export |
| 10 | gui-test migration | Update extension to use new API |
| 11 | Testing & verification | Full build, clippy, manual smoke test |

---

## Phase 1: WIT Types — Style + Basic Elements

### Files
- New: `crates/extension_api/wit/since_v0.9.0/ui-elements.wit`
- Modify: `crates/extension_api/wit/since_v0.9.0/gui.wit`
- Modify: `crates/extension_api/wit/since_v0.9.0/panel-ui.wit`

### 1.1 — `ui-elements.wit`

```wit
package zed:extension;

interface ui-elements {

    // ── Lengths ───────────────────────────────────────────────────────────
    // Mirrors gpui::Length
    variant length {
        px(f32),
        rems(f32),
        relative(f32),
        auto,
        fr(f32),
    }

    // Mirrors gpui::DefiniteLength (no Auto or Fr)
    variant definite-length {
        px(f32),
        rems(f32),
        relative(f32),
    }

    // Mirrors gpui::AbsoluteLength (px only, used for borders/shadows)
    variant absolute-length {
        px(f32),
        rems(f32),
    }

    // ── Geometry ──────────────────────────────────────────────────────────
    record edges-length {
        top:    length,
        right:  length,
        bottom: length,
        left:   length,
    }

    record edges-absolute {
        top:    absolute-length,
        right:  absolute-length,
        bottom: absolute-length,
        left:   absolute-length,
    }

    record corners-absolute {
        top-left:     absolute-length,
        top-right:    absolute-length,
        bottom-right: absolute-length,
        bottom-left:  absolute-length,
    }

    record point-f32 { x: f32, y: f32 }

    // ── Color ─────────────────────────────────────────────────────────────
    record hsla { h: f32, s: f32, l: f32, a: f32 }

    // Mirrors gpui::Color (semantic) + raw Hsla fallback.
    // Semantic variants map to cx.theme().colors().* at render time.
    variant color {
        // Raw
        hsla(hsla),
        // Semantic — follow Zed theme automatically
        text,
        text-muted,
        text-disabled,
        text-accent,
        text-placeholder,
        background,
        surface-background,
        elevated-surface-background,
        panel-background,
        border,
        border-muted,
        border-focused,
        border-transparent,
        element-background,
        element-hover,
        element-selected,
        element-active,
        element-disabled,
        ghost-element-background,
        ghost-element-hover,
        ghost-element-selected,
        status-error,
        status-warning,
        status-success,
        status-info,
        editor-background,
        accent,
        // Transparent
        transparent,
    }

    // ── Background ────────────────────────────────────────────────────────
    variant background {
        color(color),
        // future: linear-gradient, radial-gradient
    }

    // ── Box shadow ────────────────────────────────────────────────────────
    record box-shadow {
        color:        hsla,
        offset:       point-f32,
        blur:         f32,
        spread:       f32,
    }

    // ── Enums mirroring GPUI ──────────────────────────────────────────────
    enum display-type   { block, flex }
    enum position-type  { relative, absolute }

    enum flex-direction {
        row, column, row-reverse, column-reverse
    }
    enum flex-wrap { no-wrap, wrap, wrap-reverse }

    enum align-items {
        start, end, flex-start, flex-end,
        center, baseline, stretch
    }
    enum align-content {
        start, end, flex-start, flex-end,
        center, stretch, space-between, space-around, space-evenly
    }
    enum justify-content {
        start, end, flex-start, flex-end,
        center, stretch, space-between, space-around, space-evenly
    }

    enum overflow-type  { visible, hidden, scroll }
    enum visibility-type { visible, hidden }

    enum cursor-style {
        default, pointer, text, crosshair,
        not-allowed, resize-row, resize-col,
        resize-ew, resize-ns, resize-nesw, resize-nwse,
        grab, grabbing, zoom-in, zoom-out, wait, progress,
    }

    enum font-weight {
        thin, extra-light, light, normal,
        medium, semi-bold, bold, extra-bold, black
    }
    enum font-style  { normal, italic }
    enum text-align  { left, center, right, justify }
    enum white-space { normal, nowrap, pre, pre-wrap }
    enum text-overflow { clip, ellipsis }
    enum text-decoration { none, underline, strikethrough }

    // ── Style ─────────────────────────────────────────────────────────────
    // Mirrors gpui::StyleRefinement — all fields are option so only
    // specified values are transmitted.
    record style {
        // Layout
        display:          option<display-type>,
        position:         option<position-type>,
        visibility:       option<visibility-type>,
        // Size
        width:            option<length>,
        height:           option<length>,
        min-width:        option<definite-length>,
        min-height:       option<definite-length>,
        max-width:        option<definite-length>,
        max-height:       option<definite-length>,
        // Absolute positioning offsets
        top:              option<length>,
        right:            option<length>,
        bottom:           option<length>,
        left:             option<length>,
        // Flex container
        flex-direction:   option<flex-direction>,
        flex-wrap:        option<flex-wrap>,
        align-items:      option<align-items>,
        align-content:    option<align-content>,
        justify-content:  option<justify-content>,
        gap:              option<definite-length>,
        column-gap:       option<definite-length>,
        row-gap:          option<definite-length>,
        // Flex item
        flex-grow:        option<f32>,
        flex-shrink:      option<f32>,
        flex-basis:       option<length>,
        align-self:       option<align-items>,
        // Spacing
        padding:          option<edges-length>,
        margin:           option<edges-length>,
        // Background + color
        background:       option<background>,
        opacity:          option<f32>,
        // Border
        border-widths:    option<edges-absolute>,
        border-color:     option<color>,
        corner-radii:     option<corners-absolute>,
        // Shadow
        box-shadow:       list<box-shadow>,
        // Text
        text-size:        option<absolute-length>,
        text-color:       option<color>,
        font-weight:      option<font-weight>,
        font-style:       option<font-style>,
        font-family:      option<string>,
        text-align:       option<text-align>,
        line-height:      option<definite-length>,
        letter-spacing:   option<absolute-length>,
        white-space:      option<white-space>,
        text-overflow:    option<text-overflow>,
        text-decoration:  option<text-decoration>,
        // Overflow + scroll
        overflow-x:       option<overflow-type>,
        overflow-y:       option<overflow-type>,
        // Misc
        cursor:           option<cursor-style>,
        z-index:          option<u32>,
        pointer-events:   option<bool>,      // false = pass-through
    }

    // ── Events ────────────────────────────────────────────────────────────
    record event-flags {
        on-click:          bool,
        on-double-click:   bool,
        on-right-click:    bool,
        on-hover:          bool,
        on-mouse-down:     bool,
        on-mouse-up:       bool,
        on-mouse-move:     bool,
        on-scroll-wheel:   bool,
        on-key-down:       bool,
        on-key-up:         bool,
        on-focus-in:       bool,
        on-focus-out:      bool,
        on-drag:           bool,
        on-drop:           bool,
    }

    // ── Icon ──────────────────────────────────────────────────────────────
    // Named icon from Zed's built-in icon set (mirrors IconName enum).
    // Use icon-name-to-path() to get SVG asset path.
    variant icon-source {
        named(string),      // e.g. "check", "folder", "arrow-up"
        path(string),       // asset path to custom SVG
    }

    // ── Element nodes ─────────────────────────────────────────────────────

    record div-node {
        id:              option<string>,
        focus-handle-id: option<u32>,
        style:           style,
        events:          event-flags,
        tooltip:         option<string>,        // simple text tooltip
        children:        list<ui-node>,
    }

    record text-node {
        content: string,
        style:   style,
    }

    record img-node {
        id:     option<string>,
        src:    string,
        style:  style,
        events: event-flags,
    }

    record svg-node {
        source: icon-source,
        color:  option<color>,
        style:  style,
    }

    record input-node {
        id:          string,
        value:       string,
        placeholder: option<string>,
        disabled:    bool,
        style:       style,
    }

    // Virtual list — host renders visible items on-demand via
    // the `gui-render-list-item` export.
    record uniform-list-node {
        id:          string,
        item-count:  u32,
        item-height: f32,       // pixels, uniform height required
        style:       style,
        // Horizontal sizing: shrink or fill
        fill-width:  bool,
    }

    variant ui-node {
        div(div-node),
        text(text-node),
        img(img-node),
        svg(svg-node),
        input(input-node),
        uniform-list(uniform-list-node),
    }
}
```

### 1.2 — Update `gui.wit`

Add after existing host functions:

```wit
use ui-elements.{ui-node};

/// Replace the extension panel's element tree.
/// Host renders the tree using native GPUI elements.
/// Calling this is optional — the host also calls
/// `gui-render` after each event to pull the latest tree.
set-view-tree: func(root: ui-node);

/// Create a focus handle managed by the host.
/// Returns an opaque integer ID. Pass it in div-node.focus-handle-id
/// to make that element focusable and keyboard-navigable.
create-focus-handle: func() -> u32;

/// Request keyboard focus for the given focus handle.
request-focus: func(handle-id: u32);

/// Release focus handle resources when no longer needed.
drop-focus-handle: func(handle-id: u32);
```

### 1.3 — Update `panel-ui.wit`

```wit
world panel-ui {
    include extension;
    import gui;
    import ui-elements;

    use gui.{theme, ui-event};
    use ui-elements.{ui-node};

    export gui-init:             func();
    export gui-on-theme-change:  func(theme: theme);
    export gui-on-data:          func(key: string, value: string);
    export gui-on-event:         func(source-id: string, event: ui-event);

    /// Host calls this after every gui-on-event to get the updated tree.
    /// Also called on first display and after theme changes.
    /// Extension returns its full current element tree.
    export gui-render:           func() -> ui-node;

    /// Host calls this to render a single item in a uniform-list.
    /// list-id is the id of the uniform-list-node.
    export gui-render-list-item: func(list-id: string, index: u32) -> ui-node;
}
```

### 1.4 — Check

```bash
cargo check -p extension_api 2>&1 | grep "^error"
```

---

## Phase 2: Expanded `UiEvent` — Full Event Payloads

### File
- Modify: `crates/extension_api/wit/since_v0.9.0/gui.wit`

Replace current `ui-event` variant with:

```wit
record mouse-event-data {
    x:              f32,
    y:              f32,
    button:         u8,       // 0=left, 1=right, 2=middle
    click-count:    u32,
    shift:          bool,
    ctrl:           bool,
    alt:            bool,
    meta:           bool,
}

record key-event-data {
    key:            string,   // e.g. "Enter", "a", "ArrowDown"
    shift:          bool,
    ctrl:           bool,
    alt:            bool,
    meta:           bool,
    repeat:         bool,
}

record scroll-event-data {
    delta-x:        f32,
    delta-y:        f32,
    precise:        bool,     // trackpad vs wheel
}

variant ui-event {
    // Mouse
    clicked(mouse-event-data),
    double-clicked(mouse-event-data),
    right-clicked(mouse-event-data),
    mouse-down(mouse-event-data),
    mouse-up(mouse-event-data),
    mouse-moved(mouse-event-data),
    // Hover
    hover-start,
    hover-end,
    // Scroll
    scroll-wheel(scroll-event-data),
    // Keyboard
    key-down(key-event-data),
    key-up(key-event-data),
    // Focus
    focus-gained,
    focus-lost,
    // Input
    input-changed(string),
    // Drag/drop
    drag-started,
    dropped(string),          // serialized payload
}
```

### Check

```bash
cargo check -p extension_api 2>&1 | grep "^error"
```

---

## Phase 3: `WasmExtension` — New Host Functions + WASM Exports

### Files
- Modify: `crates/extension_host/src/wasm_host/wit/since_v0_9_0.rs`
- Modify: `crates/extension_host/src/wasm_host.rs`
- Modify: `crates/extension_host/src/wasm_host/wit.rs`

### 3.1 — `GuiPanelMessage` new variants

```rust
// wasm_host.rs
pub enum GuiPanelMessage {
    SetView(String),
    SetViewTree(UiNode),                    // ← new
    Emit { name: String, data: String },
    RequestData(String),
    Call { key: String, method: String, params: String },
}
```

### 3.2 — Implement host functions in `since_v0_9_0.rs`

```rust
impl gui::Host for WasmState {
    // existing impls ...

    async fn set_view_tree(&mut self, root: UiNode) -> wasmtime::Result<()> {
        if let Some(tx) = &self.gui_panel_tx {
            tx.unbounded_send(GuiPanelMessage::SetViewTree(root)).ok();
        }
        Ok(())
    }

    async fn create_focus_handle(&mut self) -> wasmtime::Result<u32> {
        let id = self.next_focus_handle_id;
        self.next_focus_handle_id += 1;
        Ok(id)
    }

    async fn request_focus(&mut self, handle_id: u32) -> wasmtime::Result<()> {
        if let Some(tx) = &self.gui_panel_tx {
            tx.unbounded_send(GuiPanelMessage::RequestFocus(handle_id)).ok();
        }
        Ok(())
    }

    async fn drop_focus_handle(&mut self, handle_id: u32) -> wasmtime::Result<()> {
        self.focus_handles.remove(&handle_id);
        Ok(())
    }
}
```

Add `next_focus_handle_id: u32` field to `WasmState`.

### 3.3 — New `WasmExtension` async methods

```rust
// wasm_host.rs
pub async fn call_gui_render(&self) -> Result<UiNode> {
    self.call(move |ext, store| {
        async move {
            match ext {
                Extension::V0_9_0(e) => e.call_gui_render(store).await,
                _ => Ok(UiNode::Div(DivNode::default())),
            }
        }
        .boxed()
    })
    .await
}

pub async fn call_gui_render_list_item(
    &self,
    list_id: String,
    index: u32,
) -> Result<UiNode> {
    self.call(move |ext, store| {
        async move {
            match ext {
                Extension::V0_9_0(e) =>
                    e.call_gui_render_list_item(store, &list_id, index).await,
                _ => Ok(UiNode::Div(DivNode::default())),
            }
        }
        .boxed()
    })
    .await
}
```

### 3.4 — Check

```bash
cargo check -p extension_host 2>&1 | grep "^error"
```

---

## Phase 4: Extension API — `zed_extension_api::ui`

### Files
- New: `crates/extension_api/src/ui.rs`
- Modify: `crates/extension_api/src/extension_api.rs` (add `pub mod ui`)

The extension-side API mirrors GPUI. Extension authors who know GPUI should feel at home.
All types are virtual (produce WIT-serializable data), not real GPUI elements.

### 4.1 — Core types and helpers

```rust
// crates/extension_api/src/ui.rs

use std::cell::RefCell;
use std::collections::HashMap;
use crate::wit::since_v0_9_0::ui_elements::*;
use crate::wit::since_v0_9_0::gui::UiEvent;

// Re-export for convenience
pub use crate::wit::since_v0_9_0::ui_elements::{
    Style, Length, DefiniteLength, AbsoluteLength,
    EdgesLength, EdgesAbsolute, CornersAbsolute,
    Hsla, Color, Background, BoxShadow, DisplayType,
    FlexDirection, AlignItems, AlignContent, JustifyContent,
    OverflowType, CursorStyle, FontWeight, FontStyle,
    TextAlign, WhiteSpace, TextOverflow, TextDecoration,
    EventFlags, UiNode, DivNode, TextNode, ImgNode,
    SvgNode, InputNode, UniformListNode, IconSource,
};

// ── Length constructors (mirrors gpui) ────────────────────────────────────
pub fn px(v: f32)       -> Length         { Length::Px(v) }
pub fn rems(v: f32)     -> Length         { Length::Rems(v) }
pub fn relative(v: f32) -> Length         { Length::Relative(v) }
pub fn auto()           -> Length         { Length::Auto }
pub fn fr(v: f32)       -> Length         { Length::Fr(v) }

pub fn def_px(v: f32)   -> DefiniteLength { DefiniteLength::Px(v) }
pub fn def_rems(v: f32) -> DefiniteLength { DefiniteLength::Rems(v) }
pub fn abs_px(v: f32)   -> AbsoluteLength { AbsoluteLength::Px(v) }

// ── Color constructors ────────────────────────────────────────────────────
pub fn hsla(h: f32, s: f32, l: f32, a: f32) -> Color {
    Color::Hsla(Hsla { h, s, l, a })
}
pub fn rgba(r: f32, g: f32, b: f32, a: f32) -> Color {
    // Convert RGB to HSL
    let max = r.max(g).max(b);
    let min = r.min(g).min(b);
    let l = (max + min) / 2.0;
    let s = if max == min { 0.0 } else {
        let d = max - min;
        if l > 0.5 { d / (2.0 - max - min) } else { d / (max + min) }
    };
    let h = if max == min { 0.0 }
    else if max == r { ((g - b) / (max - min)) % 6.0 / 6.0 }
    else if max == g { ((b - r) / (max - min) + 2.0) / 6.0 }
    else             { ((r - g) / (max - min) + 4.0) / 6.0 };
    Color::Hsla(Hsla { h: h.max(0.0), s, l, a })
}

// Semantic color shorthands
pub fn color_text()         -> Color { Color::Text }
pub fn color_muted()        -> Color { Color::TextMuted }
pub fn color_accent()       -> Color { Color::Accent }
pub fn color_border()       -> Color { Color::Border }
pub fn color_surface()      -> Color { Color::SurfaceBackground }
pub fn color_background()   -> Color { Color::Background }
pub fn color_error()        -> Color { Color::StatusError }
pub fn color_success()      -> Color { Color::StatusSuccess }
pub fn color_warning()      -> Color { Color::StatusWarning }
pub fn color_transparent()  -> Color { Color::Transparent }
```

### 4.2 — Event handler registry

```rust
// Handler storage — thread_local because WASM is single-threaded
pub struct ClickEvent   { pub x: f32, pub y: f32, pub button: u8 }
pub struct HoverEvent   { pub hovered: bool }
pub struct KeyEvent     { pub key: String, pub shift: bool, pub ctrl: bool }
pub struct ScrollEvent  { pub delta_x: f32, pub delta_y: f32 }

pub enum UiHandler {
    Click(Box<dyn Fn(&ClickEvent)>),
    DoubleClick(Box<dyn Fn(&ClickEvent)>),
    RightClick(Box<dyn Fn(&ClickEvent)>),
    MouseDown(Box<dyn Fn(&ClickEvent)>),
    MouseUp(Box<dyn Fn(&ClickEvent)>),
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

pub fn register_handler(id: impl Into<String>, handler: UiHandler) {
    HANDLERS.with(|h| {
        h.borrow_mut()
            .entry(id.into())
            .or_default()
            .push(handler);
    });
}

pub fn clear_handlers() {
    HANDLERS.with(|h| h.borrow_mut().clear());
}

/// Called by the WIT-generated gui-on-event dispatch.
pub fn dispatch_event(source_id: &str, event: &UiEvent) {
    HANDLERS.with(|h| {
        if let Some(handlers) = h.borrow().get(source_id) {
            for handler in handlers {
                match (handler, event) {
                    (UiHandler::Click(f), UiEvent::Clicked(e)) =>
                        f(&ClickEvent { x: e.x, y: e.y, button: e.button }),
                    (UiHandler::Hover(f), UiEvent::HoverStart) => f(true),
                    (UiHandler::Hover(f), UiEvent::HoverEnd) => f(false),
                    (UiHandler::KeyDown(f), UiEvent::KeyDown(e)) =>
                        f(&KeyEvent { key: e.key.clone(),
                                      shift: e.shift, ctrl: e.ctrl }),
                    (UiHandler::InputChanged(f), UiEvent::InputChanged(s)) =>
                        f(s),
                    _ => {}
                }
            }
        }
    });
}
```

### 4.3 — `Div` virtual element builder

```rust
pub struct Div {
    node: DivNode,
}

pub fn div() -> Div {
    Div { node: DivNode {
        id: None,
        focus_handle_id: None,
        style: Style::default(),
        events: EventFlags::default(),
        tooltip: None,
        children: vec![],
    }}
}

/// flex column — shorthand matching gpui::v_flex()
pub fn v_flex() -> Div {
    div().flex().flex_col()
}

/// flex row — shorthand matching gpui::h_flex()
pub fn h_flex() -> Div {
    div().flex().flex_row()
}

impl Div {
    // ── Identity ──────────────────────────────────────────────────────────
    pub fn id(mut self, id: impl Into<String>) -> Self {
        self.node.id = Some(id.into()); self
    }

    pub fn focusable(mut self) -> Self {
        // Allocate a focus handle via host call
        let handle_id = crate::wit::since_v0_9_0::gui::create_focus_handle();
        self.node.focus_handle_id = Some(handle_id);
        self
    }

    // ── Children ──────────────────────────────────────────────────────────
    pub fn child(mut self, child: impl IntoUiNode) -> Self {
        self.node.children.push(child.into_ui_node()); self
    }

    pub fn children(mut self, iter: impl IntoIterator<Item = impl IntoUiNode>) -> Self {
        self.node.children.extend(iter.into_iter().map(|c| c.into_ui_node())); self
    }

    pub fn when(self, condition: bool, f: impl FnOnce(Self) -> Self) -> Self {
        if condition { f(self) } else { self }
    }

    pub fn when_some<T>(self, option: Option<T>, f: impl FnOnce(Self, T) -> Self) -> Self {
        match option { Some(v) => f(self, v), None => self }
    }

    // ── Tooltip ───────────────────────────────────────────────────────────
    pub fn tooltip(mut self, text: impl Into<String>) -> Self {
        self.node.tooltip = Some(text.into()); self
    }

    // ── Event handlers ────────────────────────────────────────────────────
    pub fn on_click(mut self, f: impl Fn(&ClickEvent) + 'static) -> Self {
        self.node.events.on_click = true;
        if let Some(id) = &self.node.id {
            register_handler(id.clone(), UiHandler::Click(Box::new(f)));
        }
        self
    }

    pub fn on_double_click(mut self, f: impl Fn(&ClickEvent) + 'static) -> Self {
        self.node.events.on_double_click = true;
        if let Some(id) = &self.node.id { register_handler(id.clone(), UiHandler::DoubleClick(Box::new(f))); }
        self
    }

    pub fn on_right_click(mut self, f: impl Fn(&ClickEvent) + 'static) -> Self {
        self.node.events.on_right_click = true;
        if let Some(id) = &self.node.id { register_handler(id.clone(), UiHandler::RightClick(Box::new(f))); }
        self
    }

    pub fn on_hover(mut self, f: impl Fn(bool) + 'static) -> Self {
        self.node.events.on_hover = true;
        if let Some(id) = &self.node.id { register_handler(id.clone(), UiHandler::Hover(Box::new(f))); }
        self
    }

    pub fn on_mouse_move(mut self, f: impl Fn(f32, f32) + 'static) -> Self {
        self.node.events.on_mouse_move = true;
        if let Some(id) = &self.node.id { register_handler(id.clone(), UiHandler::MouseMove(Box::new(f))); }
        self
    }

    pub fn on_scroll_wheel(mut self, f: impl Fn(&ScrollEvent) + 'static) -> Self {
        self.node.events.on_scroll_wheel = true;
        if let Some(id) = &self.node.id { register_handler(id.clone(), UiHandler::ScrollWheel(Box::new(f))); }
        self
    }

    pub fn on_key_down(mut self, f: impl Fn(&KeyEvent) + 'static) -> Self {
        self.node.events.on_key_down = true;
        if let Some(id) = &self.node.id { register_handler(id.clone(), UiHandler::KeyDown(Box::new(f))); }
        self
    }

    pub fn on_focus(mut self, gained: impl Fn() + 'static, lost: impl Fn() + 'static) -> Self {
        self.node.events.on_focus_in = true;
        self.node.events.on_focus_out = true;
        if let Some(id) = &self.node.id {
            register_handler(id.clone(), UiHandler::FocusGained(Box::new(gained)));
            register_handler(id.clone(), UiHandler::FocusLost(Box::new(lost)));
        }
        self
    }

    // ── Layout ────────────────────────────────────────────────────────────
    pub fn flex(mut self) -> Self {
        self.node.style.display = Some(DisplayType::Flex); self
    }
    pub fn block(mut self) -> Self {
        self.node.style.display = Some(DisplayType::Block); self
    }
    pub fn flex_col(mut self) -> Self {
        self.node.style.flex_direction = Some(FlexDirection::Column); self
    }
    pub fn flex_row(mut self) -> Self {
        self.node.style.flex_direction = Some(FlexDirection::Row); self
    }
    pub fn flex_col_reverse(mut self) -> Self {
        self.node.style.flex_direction = Some(FlexDirection::ColumnReverse); self
    }
    pub fn flex_row_reverse(mut self) -> Self {
        self.node.style.flex_direction = Some(FlexDirection::RowReverse); self
    }
    pub fn flex_wrap(mut self) -> Self {
        self.node.style.flex_wrap = Some(FlexWrap::Wrap); self
    }
    pub fn flex_grow(mut self) -> Self {
        self.node.style.flex_grow = Some(1.0); self
    }
    pub fn flex_shrink(mut self) -> Self {
        self.node.style.flex_shrink = Some(1.0); self
    }
    pub fn flex_none(mut self) -> Self {
        self.node.style.flex_grow = Some(0.0);
        self.node.style.flex_shrink = Some(0.0);
        self
    }

    // Position
    pub fn relative(mut self) -> Self {
        self.node.style.position = Some(PositionType::Relative); self
    }
    pub fn absolute(mut self) -> Self {
        self.node.style.position = Some(PositionType::Absolute); self
    }
    pub fn inset_0(mut self) -> Self {
        self.node.style.top    = Some(px(0.0));
        self.node.style.right  = Some(px(0.0));
        self.node.style.bottom = Some(px(0.0));
        self.node.style.left   = Some(px(0.0));
        self
    }
    pub fn top(mut self, v: Length) -> Self {
        self.node.style.top = Some(v); self
    }
    pub fn bottom(mut self, v: Length) -> Self {
        self.node.style.bottom = Some(v); self
    }
    pub fn left(mut self, v: Length) -> Self {
        self.node.style.left = Some(v); self
    }
    pub fn right(mut self, v: Length) -> Self {
        self.node.style.right = Some(v); self
    }

    // Align / Justify
    pub fn items_start(mut self) -> Self {
        self.node.style.align_items = Some(AlignItems::Start); self
    }
    pub fn items_end(mut self) -> Self {
        self.node.style.align_items = Some(AlignItems::End); self
    }
    pub fn items_center(mut self) -> Self {
        self.node.style.align_items = Some(AlignItems::Center); self
    }
    pub fn items_baseline(mut self) -> Self {
        self.node.style.align_items = Some(AlignItems::Baseline); self
    }
    pub fn items_stretch(mut self) -> Self {
        self.node.style.align_items = Some(AlignItems::Stretch); self
    }
    pub fn justify_start(mut self) -> Self {
        self.node.style.justify_content = Some(JustifyContent::Start); self
    }
    pub fn justify_end(mut self) -> Self {
        self.node.style.justify_content = Some(JustifyContent::End); self
    }
    pub fn justify_center(mut self) -> Self {
        self.node.style.justify_content = Some(JustifyContent::Center); self
    }
    pub fn justify_between(mut self) -> Self {
        self.node.style.justify_content = Some(JustifyContent::SpaceBetween); self
    }
    pub fn justify_around(mut self) -> Self {
        self.node.style.justify_content = Some(JustifyContent::SpaceAround); self
    }

    // ── Size ──────────────────────────────────────────────────────────────
    pub fn w(mut self, v: Length) -> Self { self.node.style.width = Some(v); self }
    pub fn h(mut self, v: Length) -> Self { self.node.style.height = Some(v); self }
    pub fn size(self, v: Length) -> Self { self.w(v.clone()).h(v) }
    pub fn w_full(self) -> Self  { self.w(relative(1.0)) }
    pub fn h_full(self) -> Self  { self.h(relative(1.0)) }
    pub fn size_full(self) -> Self { self.w_full().h_full() }
    pub fn w_auto(self) -> Self  { self.w(auto()) }
    pub fn h_auto(self) -> Self  { self.h(auto()) }

    pub fn min_w(mut self, v: DefiniteLength) -> Self { self.node.style.min_width = Some(v); self }
    pub fn min_h(mut self, v: DefiniteLength) -> Self { self.node.style.min_height = Some(v); self }
    pub fn max_w(mut self, v: DefiniteLength) -> Self { self.node.style.max_width = Some(v); self }
    pub fn max_h(mut self, v: DefiniteLength) -> Self { self.node.style.max_height = Some(v); self }

    // ── Spacing ───────────────────────────────────────────────────────────
    fn uniform_edges(v: Length) -> EdgesLength {
        EdgesLength { top: v.clone(), right: v.clone(), bottom: v.clone(), left: v }
    }

    pub fn p(mut self, v: Length) -> Self {
        self.node.style.padding = Some(Self::uniform_edges(v)); self
    }
    pub fn px_(mut self, v: Length) -> Self {
        let e = self.node.style.padding.clone().unwrap_or_default();
        self.node.style.padding = Some(EdgesLength { left: v.clone(), right: v, ..e }); self
    }
    pub fn py_(mut self, v: Length) -> Self {
        let e = self.node.style.padding.clone().unwrap_or_default();
        self.node.style.padding = Some(EdgesLength { top: v.clone(), bottom: v, ..e }); self
    }
    pub fn pt(mut self, v: Length) -> Self {
        let mut e = self.node.style.padding.clone().unwrap_or_default();
        e.top = v; self.node.style.padding = Some(e); self
    }
    pub fn pb(mut self, v: Length) -> Self {
        let mut e = self.node.style.padding.clone().unwrap_or_default();
        e.bottom = v; self.node.style.padding = Some(e); self
    }
    pub fn pl(mut self, v: Length) -> Self {
        let mut e = self.node.style.padding.clone().unwrap_or_default();
        e.left = v; self.node.style.padding = Some(e); self
    }
    pub fn pr(mut self, v: Length) -> Self {
        let mut e = self.node.style.padding.clone().unwrap_or_default();
        e.right = v; self.node.style.padding = Some(e); self
    }

    // Tailwind-scale padding helpers
    pub fn p_0(self) -> Self  { self.p(px(0.0)) }
    pub fn p_0p5(self) -> Self { self.p(rems(0.125)) }
    pub fn p_1(self) -> Self  { self.p(rems(0.25)) }
    pub fn p_1p5(self) -> Self { self.p(rems(0.375)) }
    pub fn p_2(self) -> Self  { self.p(rems(0.5)) }
    pub fn p_2p5(self) -> Self { self.p(rems(0.625)) }
    pub fn p_3(self) -> Self  { self.p(rems(0.75)) }
    pub fn p_4(self) -> Self  { self.p(rems(1.0)) }
    pub fn p_5(self) -> Self  { self.p(rems(1.25)) }
    pub fn p_6(self) -> Self  { self.p(rems(1.5)) }
    pub fn p_8(self) -> Self  { self.p(rems(2.0)) }
    pub fn p_10(self) -> Self { self.p(rems(2.5)) }
    pub fn p_12(self) -> Self { self.p(rems(3.0)) }

    pub fn m(mut self, v: Length) -> Self {
        self.node.style.margin = Some(Self::uniform_edges(v)); self
    }
    pub fn mx_auto(mut self) -> Self {
        let e = EdgesLength { left: auto(), right: auto(),
                              top: auto(), bottom: auto() };
        self.node.style.margin = Some(e); self
    }

    // Gap
    pub fn gap(mut self, v: DefiniteLength) -> Self {
        self.node.style.gap = Some(v); self
    }
    pub fn gap_x(mut self, v: DefiniteLength) -> Self {
        self.node.style.column_gap = Some(v); self
    }
    pub fn gap_y(mut self, v: DefiniteLength) -> Self {
        self.node.style.row_gap = Some(v); self
    }
    pub fn gap_0(self) -> Self    { self.gap(def_px(0.0)) }
    pub fn gap_0p5(self) -> Self  { self.gap(def_rems(0.125)) }
    pub fn gap_1(self) -> Self    { self.gap(def_rems(0.25)) }
    pub fn gap_1p5(self) -> Self  { self.gap(def_rems(0.375)) }
    pub fn gap_2(self) -> Self    { self.gap(def_rems(0.5)) }
    pub fn gap_3(self) -> Self    { self.gap(def_rems(0.75)) }
    pub fn gap_4(self) -> Self    { self.gap(def_rems(1.0)) }
    pub fn gap_6(self) -> Self    { self.gap(def_rems(1.5)) }
    pub fn gap_8(self) -> Self    { self.gap(def_rems(2.0)) }

    // ── Background / Color ────────────────────────────────────────────────
    pub fn bg(mut self, color: Color) -> Self {
        self.node.style.background = Some(Background::Color(color)); self
    }
    pub fn opacity(mut self, v: f32) -> Self {
        self.node.style.opacity = Some(v); self
    }

    // ── Border ────────────────────────────────────────────────────────────
    fn uniform_abs(v: f32) -> EdgesAbsolute {
        let l = abs_px(v);
        EdgesAbsolute { top: l.clone(), right: l.clone(), bottom: l.clone(), left: l }
    }

    pub fn border(mut self, v: f32) -> Self {
        self.node.style.border_widths = Some(Self::uniform_abs(v)); self
    }
    pub fn border_1(self) -> Self  { self.border(1.0) }
    pub fn border_2(self) -> Self  { self.border(2.0) }
    pub fn border_t(mut self) -> Self {
        let mut e = self.node.style.border_widths.clone().unwrap_or_default();
        e.top = abs_px(1.0); self.node.style.border_widths = Some(e); self
    }
    pub fn border_b(mut self) -> Self {
        let mut e = self.node.style.border_widths.clone().unwrap_or_default();
        e.bottom = abs_px(1.0); self.node.style.border_widths = Some(e); self
    }
    pub fn border_l(mut self) -> Self {
        let mut e = self.node.style.border_widths.clone().unwrap_or_default();
        e.left = abs_px(1.0); self.node.style.border_widths = Some(e); self
    }
    pub fn border_r(mut self) -> Self {
        let mut e = self.node.style.border_widths.clone().unwrap_or_default();
        e.right = abs_px(1.0); self.node.style.border_widths = Some(e); self
    }
    pub fn border_color(mut self, color: Color) -> Self {
        self.node.style.border_color = Some(color); self
    }

    // Corner radius
    fn uniform_corner(v: f32) -> CornersAbsolute {
        let l = abs_px(v);
        CornersAbsolute { top_left: l.clone(), top_right: l.clone(),
                          bottom_right: l.clone(), bottom_left: l }
    }
    pub fn rounded(self, v: f32) -> Self {
        let mut s = self;
        s.node.style.corner_radii = Some(Self::uniform_corner(v)); s
    }
    pub fn rounded_none(self) -> Self   { self.rounded(0.0)    }
    pub fn rounded_sm(self) -> Self     { self.rounded(2.0)    }
    pub fn rounded_md(self) -> Self     { self.rounded(6.0)    }
    pub fn rounded_lg(self) -> Self     { self.rounded(8.0)    }
    pub fn rounded_xl(self) -> Self     { self.rounded(12.0)   }
    pub fn rounded_2xl(self) -> Self    { self.rounded(16.0)   }
    pub fn rounded_full(self) -> Self   { self.rounded(9999.0) }

    // Shadow
    pub fn shadow_sm(mut self) -> Self {
        self.node.style.box_shadow = vec![
            BoxShadow { color: Hsla { h: 0.0, s: 0.0, l: 0.0, a: 0.1 },
                        offset: PointF32 { x: 0.0, y: 1.0 },
                        blur: 2.0, spread: 0.0 }
        ];
        self
    }
    pub fn shadow_md(mut self) -> Self {
        self.node.style.box_shadow = vec![
            BoxShadow { color: Hsla { h: 0.0, s: 0.0, l: 0.0, a: 0.1 },
                        offset: PointF32 { x: 0.0, y: 4.0 },
                        blur: 6.0, spread: -1.0 },
            BoxShadow { color: Hsla { h: 0.0, s: 0.0, l: 0.0, a: 0.06 },
                        offset: PointF32 { x: 0.0, y: 2.0 },
                        blur: 4.0, spread: -1.0 },
        ];
        self
    }

    // ── Text styles (applied to text in children) ─────────────────────────
    pub fn text_color(mut self, color: Color) -> Self {
        self.node.style.text_color = Some(color); self
    }
    pub fn text_size(mut self, v: AbsoluteLength) -> Self {
        self.node.style.text_size = Some(v); self
    }
    pub fn text_xs(self) -> Self    { self.text_size(abs_px(10.0)) }
    pub fn text_sm(self) -> Self    { self.text_size(abs_px(12.0)) }
    pub fn text_base(self) -> Self  { self.text_size(abs_px(14.0)) }
    pub fn text_lg(self) -> Self    { self.text_size(abs_px(16.0)) }
    pub fn text_xl(self) -> Self    { self.text_size(abs_px(20.0)) }
    pub fn text_2xl(self) -> Self   { self.text_size(abs_px(24.0)) }
    pub fn font_bold(mut self) -> Self {
        self.node.style.font_weight = Some(FontWeight::Bold); self
    }
    pub fn font_medium(mut self) -> Self {
        self.node.style.font_weight = Some(FontWeight::Medium); self
    }
    pub fn font_normal(mut self) -> Self {
        self.node.style.font_weight = Some(FontWeight::Normal); self
    }
    pub fn italic(mut self) -> Self {
        self.node.style.font_style = Some(FontStyle::Italic); self
    }
    pub fn truncate(mut self) -> Self {
        self.node.style.white_space  = Some(WhiteSpace::Nowrap);
        self.node.style.text_overflow = Some(TextOverflow::Ellipsis);
        self.node.style.overflow_x   = Some(OverflowType::Hidden);
        self
    }
    pub fn line_clamp(self, _lines: u32) -> Self {
        // simplified: just truncate at overflow
        self.truncate()
    }
    pub fn underline(mut self) -> Self {
        self.node.style.text_decoration = Some(TextDecoration::Underline); self
    }
    pub fn line_through(mut self) -> Self {
        self.node.style.text_decoration = Some(TextDecoration::Strikethrough); self
    }
    pub fn text_left(mut self) -> Self {
        self.node.style.text_align = Some(TextAlign::Left); self
    }
    pub fn text_center(mut self) -> Self {
        self.node.style.text_align = Some(TextAlign::Center); self
    }
    pub fn text_right(mut self) -> Self {
        self.node.style.text_align = Some(TextAlign::Right); self
    }
    pub fn whitespace_nowrap(mut self) -> Self {
        self.node.style.white_space = Some(WhiteSpace::Nowrap); self
    }

    // ── Overflow ──────────────────────────────────────────────────────────
    pub fn overflow_hidden(mut self) -> Self {
        self.node.style.overflow_x = Some(OverflowType::Hidden);
        self.node.style.overflow_y = Some(OverflowType::Hidden);
        self
    }
    pub fn overflow_scroll(mut self) -> Self {
        self.node.style.overflow_y = Some(OverflowType::Scroll); self
    }
    pub fn overflow_x_hidden(mut self) -> Self {
        self.node.style.overflow_x = Some(OverflowType::Hidden); self
    }
    pub fn overflow_x_scroll(mut self) -> Self {
        self.node.style.overflow_x = Some(OverflowType::Scroll); self
    }

    // ── Misc ──────────────────────────────────────────────────────────────
    pub fn cursor_pointer(mut self) -> Self {
        self.node.style.cursor = Some(CursorStyle::Pointer); self
    }
    pub fn cursor_text(mut self) -> Self {
        self.node.style.cursor = Some(CursorStyle::Text); self
    }
    pub fn cursor_not_allowed(mut self) -> Self {
        self.node.style.cursor = Some(CursorStyle::NotAllowed); self
    }
    pub fn z_index(mut self, v: u32) -> Self {
        self.node.style.z_index = Some(v); self
    }
    pub fn pointer_events_none(mut self) -> Self {
        self.node.style.pointer_events = Some(false); self
    }
    pub fn invisible(mut self) -> Self {
        self.node.style.visibility = Some(VisibilityType::Hidden); self
    }
}

impl IntoUiNode for Div {
    fn into_ui_node(self) -> UiNode { UiNode::Div(self.node) }
}
```

### 4.4 — `Label` virtual element

```rust
pub struct Label {
    node: TextNode,
}

impl Label {
    pub fn new(text: impl Into<String>) -> Self {
        Self { node: TextNode { content: text.into(), style: Style::default() } }
    }

    pub fn color(mut self, color: Color) -> Self {
        self.node.style.text_color = Some(color); self
    }
    pub fn size(mut self, size: AbsoluteLength) -> Self {
        self.node.style.text_size = Some(size); self
    }
    pub fn weight(mut self, w: FontWeight) -> Self {
        self.node.style.font_weight = Some(w); self
    }
    pub fn italic(mut self) -> Self {
        self.node.style.font_style = Some(FontStyle::Italic); self
    }
    pub fn truncate(mut self) -> Self {
        self.node.style.white_space  = Some(WhiteSpace::Nowrap);
        self.node.style.text_overflow = Some(TextOverflow::Ellipsis);
        self.node.style.overflow_x   = Some(OverflowType::Hidden);
        self
    }
    pub fn muted(self) -> Self { self.color(color_muted()) }
    pub fn accent(self) -> Self { self.color(color_accent()) }
}

impl IntoUiNode for Label {
    fn into_ui_node(self) -> UiNode { UiNode::Text(self.node) }
}
```

### 4.5 — `Icon` virtual element

```rust
pub struct Icon {
    source: IconSource,
    color:  Option<Color>,
    size:   f32,
}

impl Icon {
    /// Use a named built-in Zed icon (e.g. "check", "folder", "x")
    pub fn named(name: impl Into<String>) -> Self {
        Self { source: IconSource::Named(name.into()), color: None, size: 16.0 }
    }

    pub fn from_path(path: impl Into<String>) -> Self {
        Self { source: IconSource::Path(path.into()), color: None, size: 16.0 }
    }

    pub fn color(mut self, color: Color) -> Self { self.color = Some(color); self }
    pub fn muted(self) -> Self { self.color(color_muted()) }
    pub fn accent(self) -> Self { self.color(color_accent()) }
    pub fn size(mut self, size: f32) -> Self { self.size = size; self }
    pub fn small(self) -> Self { self.size(12.0) }
    pub fn medium(self) -> Self { self.size(16.0) }
    pub fn large(self) -> Self { self.size(20.0) }
}

impl IntoUiNode for Icon {
    fn into_ui_node(self) -> UiNode {
        UiNode::Svg(SvgNode {
            source: self.source,
            color: self.color,
            style: Style {
                width:  Some(px(self.size)),
                height: Some(px(self.size)),
                ..Default::default()
            },
        })
    }
}
```

### 4.6 — `Button` component

```rust
pub enum ButtonVariant { Default, Ghost, Outlined, Destructive }
pub enum ButtonSize   { Xs, Sm, Md, Lg }

pub struct Button {
    id:      String,
    label:   Option<String>,
    icon:    Option<Icon>,
    variant: ButtonVariant,
    size:    ButtonSize,
    disabled: bool,
    toggle:  Option<bool>,
    handler: Option<Box<dyn Fn(&ClickEvent)>>,
}

impl Button {
    pub fn new(id: impl Into<String>, label: impl Into<String>) -> Self {
        Self { id: id.into(), label: Some(label.into()), icon: None,
               variant: ButtonVariant::Default, size: ButtonSize::Md,
               disabled: false, toggle: None, handler: None }
    }

    pub fn icon_only(id: impl Into<String>, icon: Icon) -> Self {
        Self { id: id.into(), label: None, icon: Some(icon),
               variant: ButtonVariant::Ghost, size: ButtonSize::Md,
               disabled: false, toggle: None, handler: None }
    }

    pub fn on_click(mut self, f: impl Fn(&ClickEvent) + 'static) -> Self {
        self.handler = Some(Box::new(f)); self
    }

    pub fn variant(mut self, v: ButtonVariant) -> Self { self.variant = v; self }
    pub fn ghost(self) -> Self { self.variant(ButtonVariant::Ghost) }
    pub fn outlined(self) -> Self { self.variant(ButtonVariant::Outlined) }
    pub fn destructive(self) -> Self { self.variant(ButtonVariant::Destructive) }

    pub fn size(mut self, s: ButtonSize) -> Self { self.size = s; self }
    pub fn small(self) -> Self { self.size(ButtonSize::Sm) }
    pub fn large(self) -> Self { self.size(ButtonSize::Lg) }

    pub fn disabled(mut self, v: bool) -> Self { self.disabled = v; self }
    pub fn toggle_state(mut self, on: bool) -> Self { self.toggle = Some(on); self }
    pub fn icon(mut self, icon: Icon) -> Self { self.icon = Some(icon); self }
}

impl IntoUiNode for Button {
    fn into_ui_node(self) -> UiNode {
        if let Some(handler) = self.handler {
            register_handler(self.id.clone(), UiHandler::Click(handler));
        }

        let (px_size, py_size, text_size) = match self.size {
            ButtonSize::Xs => (rems(0.25), rems(0.125), abs_px(11.0)),
            ButtonSize::Sm => (rems(0.5),  rems(0.25),  abs_px(12.0)),
            ButtonSize::Md => (rems(0.75), rems(0.375), abs_px(13.0)),
            ButtonSize::Lg => (rems(1.0),  rems(0.5),   abs_px(14.0)),
        };

        let (bg, text_c, border_c) = match self.variant {
            ButtonVariant::Default     =>
                (Color::ElementBackground, Color::Text, Color::Transparent),
            ButtonVariant::Ghost       =>
                (Color::Transparent, Color::Text, Color::Transparent),
            ButtonVariant::Outlined    =>
                (Color::Transparent, Color::Text, Color::Border),
            ButtonVariant::Destructive =>
                (Color::StatusError, Color::Background, Color::Transparent),
        };

        let selected_bg = self.toggle
            .and_then(|on| on.then_some(Color::ElementSelected));

        let mut el = div()
            .id(self.id)
            .flex()
            .flex_row()
            .items_center()
            .gap_1()
            .px_(px_size)
            .py_(py_size)
            .rounded_md()
            .bg(selected_bg.unwrap_or(bg))
            .text_size(text_size)
            .text_color(text_c)
            .border_1()
            .border_color(border_c)
            .cursor_pointer();

        if self.disabled {
            el = el.opacity(0.5).cursor_not_allowed();
        }

        if let Some(icon) = self.icon {
            el = el.child(icon);
        }

        if let Some(label) = self.label {
            el = el.child(Label::new(label).color(text_c));
        }

        if !self.disabled {
            el.node.events.on_click = true;
        }

        el.into_ui_node()
    }
}
```

### 4.7 — Additional components

#### `Divider`

```rust
pub struct Divider { horizontal: bool }

pub fn divider_h() -> Divider { Divider { horizontal: true } }
pub fn divider_v() -> Divider { Divider { horizontal: false } }

impl IntoUiNode for Divider {
    fn into_ui_node(self) -> UiNode {
        if self.horizontal {
            div().w_full().h(px(1.0)).bg(color_border()).into_ui_node()
        } else {
            div().w(px(1.0)).h_full().bg(color_border()).into_ui_node()
        }
    }
}
```

#### `Toggle` (checkbox)

```rust
pub struct Toggle {
    id:      String,
    checked: bool,
    handler: Option<Box<dyn Fn(bool)>>,
}

impl Toggle {
    pub fn new(id: impl Into<String>, checked: bool) -> Self {
        Self { id: id.into(), checked, handler: None }
    }
    pub fn on_change(mut self, f: impl Fn(bool) + 'static) -> Self {
        self.handler = Some(Box::new(f)); self
    }
}
```

#### `ProgressBar`

```rust
pub struct ProgressBar { value: f32 }  // 0.0–1.0

impl ProgressBar {
    pub fn new(value: f32) -> Self { Self { value: value.clamp(0.0, 1.0) } }
}

impl IntoUiNode for ProgressBar {
    fn into_ui_node(self) -> UiNode {
        div()
            .w_full()
            .h(px(4.0))
            .rounded_full()
            .bg(Color::ElementBackground)
            .child(
                div()
                    .h_full()
                    .rounded_full()
                    .bg(Color::Accent)
                    .w(relative(self.value)),
            )
            .into_ui_node()
    }
}
```

#### `UniformList`

```rust
pub struct UniformList {
    id:          String,
    item_count:  u32,
    item_height: f32,
    style:       Style,
}

impl UniformList {
    pub fn new(id: impl Into<String>, item_count: u32, item_height: f32) -> Self {
        Self { id: id.into(), item_count, item_height, style: Style::default() }
    }
    pub fn w_full(mut self) -> Self {
        self.style.width = Some(relative(1.0)); self
    }
    pub fn h_full(mut self) -> Self {
        self.style.height = Some(relative(1.0)); self
    }
}

impl IntoUiNode for UniformList {
    fn into_ui_node(self) -> UiNode {
        UiNode::UniformList(UniformListNode {
            id:          self.id,
            item_count:  self.item_count,
            item_height: self.item_height,
            style:       self.style,
            fill_width:  true,
        })
    }
}
```

### 4.8 — `set_view` and `IntoUiNode` trait

```rust
pub trait IntoUiNode {
    fn into_ui_node(self) -> UiNode;
}

impl IntoUiNode for UiNode {
    fn into_ui_node(self) -> UiNode { self }
}

impl IntoUiNode for String {
    fn into_ui_node(self) -> UiNode {
        UiNode::Text(TextNode { content: self, style: Style::default() })
    }
}

impl IntoUiNode for &str {
    fn into_ui_node(self) -> UiNode { self.to_string().into_ui_node() }
}

/// Push a new element tree to the host.
/// The host re-renders on the next GPUI frame.
pub fn set_view(root: impl IntoUiNode) {
    crate::wit::since_v0_9_0::gui::set_view_tree(root.into_ui_node());
}
```

### 4.9 — `gui-render` default impl in `extension_api.rs`

```rust
// In impl zed::extension::Guest for Extension dispatch:
fn gui_render() -> UiNode {
    T::gui_render()   // call extension's impl
}

fn gui_render_list_item(list_id: String, index: u32) -> UiNode {
    T::gui_render_list_item(&list_id, index)
}

// Default impls on the Extension trait:
fn gui_render() -> UiNode {
    UiNode::Div(DivNode::default())
}

fn gui_render_list_item(_list_id: &str, _index: u32) -> UiNode {
    UiNode::Div(DivNode::default())
}
```

---

## Phase 5: Host-side Renderer — WIT → GPUI

### Files
- New: `crates/extension_panel/src/ui_renderer.rs`
- Modify: `crates/extension_panel/src/extension_panel.rs`

### 5.1 — Semantic color mapping

```rust
// ui_renderer.rs
use gpui::*;
use ui::prelude::*;

fn resolve_color(color: &WitColor, cx: &App) -> Hsla {
    let colors = cx.theme().colors();
    match color {
        WitColor::Hsla(h)                    => gpui::hsla(h.h, h.s, h.l, h.a),
        WitColor::Text                       => colors.text,
        WitColor::TextMuted                  => colors.text_muted,
        WitColor::TextDisabled               => colors.text_disabled,
        WitColor::TextAccent                 => colors.text_accent,
        WitColor::Background                 => colors.background,
        WitColor::SurfaceBackground          => colors.surface_background,
        WitColor::ElevatedSurfaceBackground  => colors.elevated_surface_background,
        WitColor::PanelBackground            => colors.panel_background,
        WitColor::Border                     => colors.border,
        WitColor::BorderMuted                => colors.border_muted,
        WitColor::BorderFocused              => colors.border_focused,
        WitColor::ElementBackground          => colors.element_background,
        WitColor::ElementHover               => colors.element_hover,
        WitColor::ElementSelected            => colors.element_selected,
        WitColor::ElementActive              => colors.element_active,
        WitColor::ElementDisabled            => colors.element_disabled,
        WitColor::Accent                     => colors.text_accent,
        WitColor::StatusError                => colors.status_error(),
        WitColor::StatusWarning              => colors.status_warning(),
        WitColor::StatusSuccess              => colors.status_success(),
        WitColor::StatusInfo                 => colors.status_info(),
        WitColor::Transparent                => gpui::transparent_black(),
        _                                    => colors.text,
    }
}
```

### 5.2 — `render_node` — main dispatch

```rust
pub fn render_node(node: &UiNode, cx: &App) -> AnyElement {
    match node {
        UiNode::Div(n)         => render_div(n, cx).into_any_element(),
        UiNode::Text(n)        => render_text(n, cx).into_any_element(),
        UiNode::Img(n)         => render_img(n, cx).into_any_element(),
        UiNode::Svg(n)         => render_svg(n, cx).into_any_element(),
        UiNode::Input(n)       => render_input(n, cx).into_any_element(),
        UiNode::UniformList(n) => render_uniform_list(n, cx).into_any_element(),
    }
}
```

### 5.3 — `render_div`

```rust
fn render_div(n: &DivNode, cx: &App) -> Div {
    let mut el = div();
    apply_style(&mut el, &n.style, cx);
    for child in &n.children {
        el = el.child(render_node(child, cx));
    }
    el
}
```

`apply_style` maps each WIT style field to GPUI's `StyleRefinement` via the builder methods.
For each `option<T>` field: if `Some`, call the corresponding GPUI method; if `None`, skip.
Length conversion follows the mapping: `WitLength::Px(v)` → `gpui::px(v)`, etc.

### 5.4 — Focus handle mapping

The host maintains a `HashMap<u32, FocusHandle>` per `ExtensionGuiView`. When a `DivNode`
has `focus_handle_id: Some(id)`, look up the handle and call `.track_focus(&handle)` on the
GPUI `Div`.

```rust
// ExtensionGuiView
focus_handles: HashMap<u32, FocusHandle>,

// In render_div:
if let Some(id) = n.focus_handle_id {
    if let Some(handle) = view.focus_handles.get(&id) {
        el = el.track_focus(handle);
    }
}
```

When `GuiPanelMessage::RequestFocus(id)` arrives, call `handle.focus(window)`.
When `GuiPanelMessage::CreateFocusHandle(id)` arrives, create new `cx.focus_handle()` and
store in map.

### 5.5 — Event wiring (host → WASM)

For each event flag on a `DivNode`, attach the corresponding GPUI event handler.
Each handler sends `wasm_extension.call_gui_on_event(element_id, UiEvent::*)` then
awaits the updated tree via `call_gui_render()`.

### 5.6 — Uniform list rendering

```rust
fn render_uniform_list(n: &UniformListNode, cx: &App) -> impl IntoElement {
    let list_id = n.id.clone();
    let wasm = wasm_extension.clone();  // captured from context

    uniform_list(
        cx.entity_id(),
        n.id.clone(),
        n.item_count as usize,
        move |range, window, cx| {
            range.map(|index| {
                // Synchronously render: block on WASM call
                // This is called from GPUI's render thread.
                // Use cached items when possible.
                let node = cached_list_items
                    .get(&(list_id.clone(), index as u32))
                    .cloned()
                    .unwrap_or(UiNode::Div(DivNode::default()));
                render_node(&node, cx)
            }).collect()
        },
    )
    .w_full()
    .h_full()
}
```

For `uniform_list`, items are fetched asynchronously: after `gui-render` is called, the host
also pre-fetches visible items via `call_gui_render_list_item()` and caches them. On scroll,
new items are fetched in a background task.

---

## Phase 6: `ExtensionGuiView` — Wiring Everything Together

### State

```rust
pub struct ExtensionGuiView {
    pub(crate) extension_id:  Arc<str>,
    focus_handle:             FocusHandle,
    workspace:                WeakEntity<Workspace>,
    wasm_extension:           WasmExtension,
    ui_tree:                  Option<UiNode>,
    focus_handles:            HashMap<u32, FocusHandle>,
    list_item_cache:          HashMap<(String, u32), UiNode>,
}
```

### Pull-model flow

```
1. host receives GuiPanelMessage::SetViewTree(node)  → store in ui_tree, cx.notify()
2. host calls gui-render() after every gui-on-event   → store in ui_tree, cx.notify()
3. GPUI calls render() on next frame
4. render() calls ui_renderer::render_node(&ui_tree, cx)
5. GPUI displays the GPUI elements natively
```

---

## Phase 7: `gui-test` — Migration to New API

### File
- Modify: `extensions/gui-test/src/lib.rs`

```rust
use zed_extension_api::{Extension, register_command};
use zed_extension_api::ui::*;

struct GuiTest { count: u32 }

impl Extension for GuiTest {
    fn new() -> Self {
        register_command("open-panel", "open panel");
        GuiTest { count: 0 }
    }

    fn gui_render(&mut self) -> UiNode {
        let count = self.count;
        v_flex()
            .size_full()
            .p_4()
            .gap_3()
            .child(
                h_flex()
                    .items_center()
                    .gap_2()
                    .child(Icon::named("blocks").medium().color(color_accent()))
                    .child(Label::new("GUI Test Extension").text_xl().weight(FontWeight::Bold))
            )
            .child(divider_h())
            .child(
                h_flex()
                    .gap_2()
                    .child(
                        Button::new("btn-inc", "Increment")
                            .icon(Icon::named("plus").small())
                            .on_click(|_| {
                                zed_extension_api::emit("state:increment", "");
                            })
                    )
                    .child(
                        Button::new("btn-reset", "Reset")
                            .ghost()
                            .on_click(|_| {
                                zed_extension_api::emit("state:reset", "");
                            })
                    )
            )
            .child(
                div()
                    .p_3()
                    .rounded_lg()
                    .bg(Color::SurfaceBackground)
                    .border_1()
                    .border_color(Color::Border)
                    .child(Label::new(format!("Count: {count}")).text_lg())
            )
            .into_ui_node()
    }

    fn gui_on_data(&mut self, key: &str, _value: &str) {
        match key {
            "state:increment" => { self.count += 1; }
            "state:reset"     => { self.count = 0; }
            _ => {}
        }
        // No explicit set_view needed — host calls gui-render() automatically
    }

    fn run_extension_command(&mut self, command_id: &str) -> Result<(), String> {
        match command_id {
            "open-panel" => Ok(()),
            _            => Err(format!("unknown command: {command_id}")),
        }
    }
}

zed_extension_api::register_extension!(GuiTest);
```

---

## Phase 8: Testing & Verification

### Build checks

```bash
# Full clippy pass
./script/clippy 2>&1 | grep "^error"

# Individual crates
cargo check -p extension_api 2>&1 | grep "^error"
cargo check -p extension_host 2>&1 | grep "^error"
cargo check -p extension_panel 2>&1 | grep "^error"
cargo check -p zed 2>&1 | grep "^error"
```

### Smoke test checklist

- [ ] Zed launches, ExtensionGuiPanel loads in footer
- [ ] `ctrl+shift+P` → "gui_test: open panel" appears
- [ ] Selecting command → panel opens with gui-test UI
- [ ] UI displays correctly (Label, Button, Icon, Divider)
- [ ] Theme colors applied correctly (dark/light mode switch)
- [ ] "Increment" button → count updates
- [ ] "Reset" button → count resets to 0
- [ ] Panel re-renders after each click (no stale state)
- [ ] Second `ctrl+shift+P` invoke → focuses existing tab, no duplicate
- [ ] Extension crash (force panic in gui-render) → Zed continues, panel shows error state

---

## Key files reference

| File | Change |
|------|--------|
| `crates/extension_api/wit/since_v0.9.0/ui-elements.wit` | New — full style + element types |
| `crates/extension_api/wit/since_v0.9.0/gui.wit` | Add `set-view-tree`, `create-focus-handle`, `request-focus`, `drop-focus-handle` |
| `crates/extension_api/wit/since_v0.9.0/panel-ui.wit` | Add `gui-render`, `gui-render-list-item` exports |
| `crates/extension_api/src/ui.rs` | New — virtual element builders mirroring GPUI API |
| `crates/extension_api/src/extension_api.rs` | Add `pub mod ui`; default impls for `gui-render`, `gui-render-list-item` |
| `crates/extension_host/src/wasm_host/wit/since_v0_9_0.rs` | Implement `set_view_tree`, `create_focus_handle`, `request_focus`, `drop_focus_handle` |
| `crates/extension_host/src/wasm_host.rs` | Add `GuiPanelMessage` variants; new `WasmExtension` methods |
| `crates/extension_host/src/wasm_host/wit.rs` | Add `call_gui_render`, `call_gui_render_list_item` to `Extension` enum |
| `crates/extension_panel/src/ui_renderer.rs` | New — WIT node → GPUI element conversion + semantic color mapping |
| `crates/extension_panel/src/extension_panel.rs` | `ExtensionGuiView` stores `ui_tree`, `focus_handles`; pull-model wiring |
| `extensions/gui-test/src/lib.rs` | Migrate to new API |

---

## Versioning note

This entire feature lives under `since_v0.9.0`. If a future version (e.g. v0.10.0) adds new
element types or style fields, the new WIT types go into `since_v0.10.0/`. The host's
`render_node` match arm for old versions simply falls back to a `div()` placeholder for
unknown nodes. Extensions do not break.

---

## Known limitations (future work)

| Limitation | Future solution |
|---|---|
| `animation` element not supported | Add `animation-node` variant + CSS-like timing |
| `anchored` / floating elements | Add `anchored-node` with anchor position enum |
| Drag-and-drop between elements | Add `draggable` flag + `on-drop` payload type |
| Custom fonts | Add `font-family` resolution via host font registry |
| `canvas` (custom GPU drawing) | Add `canvas-node` with `gui-paint-canvas` export |
| Grid layout | Add `grid-template-columns/rows` to style record |
| Rich text (mixed styles) | Add `rich-text-node` with inline span styles |
| `uniform-list` item pre-fetching | Background task fetching visible range + overfetch |
