# Extension GUI Plan

## Mục tiêu

Cho phép Zed extensions có GUI panel native, tái sử dụng Zed UI libraries và theme, mà không ảnh hưởng đến các extensions hiện tại (backward compatible hoàn toàn).

## Ràng buộc thiết kế

- Extensions cũ (không có GUI): **không thay đổi gì**
- Extension GUI chạy **in-process** cùng Zed (không subprocess)
- Extension GUI được compile thành **WASM** — cùng `extension.wasm`, không cần file riêng
- UI render bằng **native GPUI** — đúng theme Zed, không phải webview
- Extension GUI crash **không ảnh hưởng main thread** (wasmtime sandbox + Tokio task isolation)

---

## Kiến trúc

```
Zed Main Process
│
├── Tokio Runtime
│   └── extension.wasm task  (logic + GUI exports — 1 file duy nhất)
│            │
│            │  WIT host functions: set-view(element), emit(name, data)
│            │  (buffered, flush 1 lần per tick)
│            ▼
│   main_thread_message_tx  (đã có sẵn trong WasmHost)
│            │
│            ▼
└── GPUI Main Thread
    └── ExtensionGuiPanel  (Entity<T> implement Panel + Render)
        - Nhận element tree từ WASM (batched)
        - Render GPUI từ element tree
        - Forward UI events → WASM callback
        - Hiển thị error state nếu WASM crash
```

---

## Crash Isolation

| Tình huống | Ảnh hưởng? | Cơ chế |
|---|---|---|
| WASM trap | Không | wasmtime catch trap → `Err`, không unwind host |
| WASM infinite loop | Không | `epoch_deadline_async_yield_and_update` tự interrupt |
| Host function panic | Chỉ task đó | Tokio isolates task panics |
| Extension task kết thúc | Không | Panel chuyển sang error state |
| Channel closed (Zed shutdown) | Không | Host functions dùng `try_send`, không panic |

---

## Backward Compatibility — Vấn Đề WIT Exports

**WIT Component Model rule:** Mọi `export` trong world **phải tồn tại** trong WASM binary.
Wasmtime **từ chối instantiate** nếu thiếu export.

→ Không thể thêm `gui-*` exports vào world `extension` hiện tại — extensions cũ sẽ fail.

**Giải pháp:** Theo đúng pattern versioning của Zed — tạo `since_v0.9.0` với **2 world riêng biệt**:

```
crates/extension_api/wit/
├── since_v0.8.0/         ← extensions cũ dùng world này, không đổi gì
└── since_v0.9.0/         ← version mới
    ├── extension.wit              world "extension" — copy từ v0.8.0, KHÔNG đổi
    ├── extension-with-gui.wit     world "extension-with-gui" — mới
    └── gui.wit                    GUI interface — mới
```

Zed dispatch đúng linker dựa trên version + manifest:

```
Extension cũ  (v0.8.0, gui = None)  → since_v0_8_0 linker  ✅ không đổi
Extension mới (v0.9.0, gui = None)  → since_v0_9_0::Extension linker  ✅ không có gui exports
Extension mới (v0.9.0, gui = Some)  → since_v0_9_0::ExtensionWithGui linker  ✅ phải implement gui-*
```

---

## Những Gì Cần Thay Đổi

### 1. `crates/extension/src/extension_manifest.rs`

Thêm optional field — **không đổi gì khác trong file này**:

```rust
#[derive(Debug, PartialEq, Eq, Clone, Serialize, Deserialize)]
pub struct ExtensionManifest {
    // ... tất cả fields hiện tại giữ nguyên ...

    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub gui: Option<ExtensionGuiManifest>,
}

#[derive(Debug, PartialEq, Eq, Clone, Serialize, Deserialize)]
pub struct ExtensionGuiManifest {
    /// Minimum Zed API version để chạy GUI.
    /// GUI exports nằm trong extension.wasm, không cần file riêng.
    pub min_zed_api: String,
}
```

---

### 2. `crates/extension_api/wit/since_v0.9.0/` *(folder mới)*

#### `extension.wit` — copy từ v0.8.0, không đổi gì

```wit
package zed:extension;

world extension {
    import context-server;
    import dap;
    import github;
    import http-client;
    import platform;
    import process;
    import nodejs;

    // ... y hệt v0.8.0 ...
    export init-extension: func();
    export language-server-command: func(...);
    // KHÔNG có gui exports
}
```

#### `extension-with-gui.wit` — world mới cho extensions có GUI

```wit
package zed:extension;

world extension-with-gui {
    /// Kế thừa toàn bộ world extension.
    include extension;

    /// Host functions Zed cung cấp cho WASM.
    import gui;

    /// WASM phải implement các callbacks sau.
    export gui-init: func();
    export gui-on-theme-change: func(theme: gui.theme);
    export gui-on-data: func(key: string, value: string);
    export gui-on-event: func(source-id: string, event: gui.ui-event);
}
```

#### `gui.wit` — GUI interface

```wit
package zed:extension;

interface gui {

    // ── Theme ─────────────────────────────────────────────────────────────────

    record color { r: u8, g: u8, b: u8, a: u8 }

    record theme-colors {
        background:                  color,
        editor-background:           color,
        surface-background:          color,
        elevated-surface-background: color,
        text:                        color,
        text-muted:                  color,
        text-disabled:               color,
        text-accent:                 color,
        border:                      color,
        border-muted:                color,
        border-focused:              color,
        element-background:          color,
        element-hover:               color,
        element-selected:            color,
        element-active:              color,
        element-disabled:            color,
        panel-background:            color,
        status-error:                color,
        status-warning:              color,
        status-success:              color,
        status-info:                 color,
    }

    record theme {
        name:    string,
        is-dark: bool,
        colors:  theme-colors,
    }

    // ── Sizing ────────────────────────────────────────────────────────────────

    enum size { extra-small, small, medium, large }

    // ── Layout ────────────────────────────────────────────────────────────────

    record padding {
        top:    option<u32>,
        right:  option<u32>,
        bottom: option<u32>,
        left:   option<u32>,
    }

    record flex-element {
        id:       option<string>,
        children: list<element>,
        gap:      option<u32>,
        padding:  option<padding>,
        grow:     bool,
    }

    record scroll-element {
        id:         string,
        child:      element,
        vertical:   bool,
        horizontal: bool,
    }

    // ── Display ───────────────────────────────────────────────────────────────

    record label-element {
        text:   string,
        size:   option<size>,
        muted:  bool,
        italic: bool,
    }

    record icon-element {
        icon:  string,          // Zed IconName
        size:  option<size>,
        color: option<string>,  // "default"|"muted"|"accent"|"error"|"warning"|"success"
    }

    record badge-element {
        text:    string,
        variant: string,        // "info"|"success"|"warning"|"error"
    }

    record divider-element { horizontal: bool }

    // ── Interactive ───────────────────────────────────────────────────────────

    enum button-style { default, primary, ghost, danger }

    record button-element {
        id:       string,
        label:    option<string>,
        icon:     option<string>,
        style:    button-style,
        size:     option<size>,
        disabled: bool,
        tooltip:  option<string>,
    }

    record input-element {
        id:          string,
        placeholder: option<string>,
        value:       string,
        disabled:    bool,
        password:    bool,
    }

    record select-option { value: string, label: string }

    record select-element {
        id:       string,
        options:  list<select-option>,
        value:    option<string>,
        disabled: bool,
    }

    record checkbox-element {
        id:       string,
        label:    string,
        checked:  bool,
        disabled: bool,
    }

    // ── List / Table ──────────────────────────────────────────────────────────

    record list-item {
        id:          string,
        label:       string,
        description: option<string>,
        icon:        option<string>,
        meta:        option<string>,
        indent:      u32,
    }

    record list-element {
        id:       string,
        items:    list<list-item>,
        selected: option<string>,
    }

    record table-column { key: string, label: string, width: option<u32> }
    record table-row    { id: string, cells: list<tuple<string, string>> }

    record table-element {
        id:           string,
        columns:      list<table-column>,
        rows:         list<table-row>,
        selected-row: option<string>,
    }

    // ── Root element variant ──────────────────────────────────────────────────

    variant element {
        v-flex(flex-element),
        h-flex(flex-element),
        scroll(scroll-element),
        label(label-element),
        icon(icon-element),
        badge(badge-element),
        divider(divider-element),
        button(button-element),
        input(input-element),
        select(select-element),
        checkbox(checkbox-element),
        list(list-element),
        table(table-element),
    }

    // ── Events ────────────────────────────────────────────────────────────────

    variant ui-event {
        clicked,
        input-changed(string),
        select-changed(string),
        checkbox-changed(bool),
        row-selected(string),
    }

    // ── Host functions (Zed cung cấp, WASM gọi vào) ──────────────────────────

    /// Cập nhật UI. Buffered — nhiều lần gọi trong 1 tick chỉ render 1 lần.
    import set-view: func(root: element);

    /// Gửi event tên tự do lên Zed để forward sang extension.wasm logic.
    import emit: func(name: string, data: string);

    /// Yêu cầu data — kết quả trả về qua gui-on-data callback.
    import request-data: func(key: string);
}
```

---

### 3. `crates/extension_host/src/wasm_host/wit/since_v0_9_0.rs` *(file mới)*

```rust
pub const MIN_VERSION: Version = Version::new(0, 9, 0);
pub const MAX_VERSION: Version = Version::new(0, 9, 0);

// Bindings cho world "extension" (không có GUI) — y hệt v0.8.0
wasmtime::component::bindgen!({
    async: true,
    world: "extension",
    path: "../extension_api/wit/since_v0.9.0",
    // ...
});

// Bindings cho world "extension-with-gui"
wasmtime::component::bindgen!({
    async: true,
    world: "extension-with-gui",
    path: "../extension_api/wit/since_v0.9.0",
    // ...
});

pub fn linker(executor: &BackgroundExecutor) -> &'static Linker<WasmState> {
    static LINKER: OnceLock<Linker<WasmState>> = OnceLock::new();
    LINKER.get_or_init(|| super::new_linker(executor, |linker, get| {
        Extension::add_to_linker(linker, get)?;
        // Thêm GUI host functions
        gui::add_to_linker(linker, get)?;
        Ok(())
    }))
}

pub fn linker_with_gui(executor: &BackgroundExecutor) -> &'static Linker<WasmState> {
    static LINKER: OnceLock<Linker<WasmState>> = OnceLock::new();
    LINKER.get_or_init(|| super::new_linker(executor, |linker, get| {
        ExtensionWithGui::add_to_linker(linker, get)?;
        Ok(())
    }))
}
```

---

### 4. `crates/extension_host/src/wasm_host/wit.rs`

Thêm dispatch cho v0.9.0 trong `instantiate_async` — **không đổi logic dispatch cũ**:

```rust
pub async fn instantiate_async(
    executor: &BackgroundExecutor,
    store: &mut Store<WasmState>,
    release_channel: ReleaseChannel,
    zed_api_version: Version,
    component: &Component,
    has_gui: bool,          // ← thêm param
) -> Result<Extension> {
    match zed_api_version {
        // ... các version cũ giữ nguyên ...

        v if since_v0_9_0::version_range().contains(&v) => {
            if has_gui {
                since_v0_9_0::ExtensionWithGui::instantiate_async(
                    executor, store, component
                ).await
            } else {
                since_v0_9_0::Extension::instantiate_async(
                    executor, store, component
                ).await
            }
        }
    }
}
```

---

### 5. `crates/extension_host/src/wasm_host.rs`

#### 5a. Thêm fields vào `WasmState` để buffer GUI updates

```rust
pub struct WasmState {
    manifest: Arc<ExtensionManifest>,
    pub table: ResourceTable,
    ctx: wasi::WasiCtx,
    pub host: Arc<WasmHost>,
    pub(crate) capability_granter: CapabilityGranter,

    // GUI — chỉ Some khi manifest.gui.is_some()
    gui_pending_view: Option<wit::gui::Element>,
    gui_flush_scheduled: bool,
    gui_panel_tx: Option<mpsc::UnboundedSender<GuiPanelMessage>>,
}

pub enum GuiPanelMessage {
    SetView(wit::gui::Element),
    Emit { name: String, data: String },
    RequestData(String),
}
```

#### 5b. Host function implementations — dùng `try_send`, không panic

```rust
// set-view: buffer thay vì send ngay, tránh nhiều cx.notify() per tick
fn gui_set_view(state: &mut WasmState, root: wit::gui::Element) -> wasmtime::Result<()> {
    state.gui_pending_view = Some(root);  // overwrite — chỉ giữ latest

    if !state.gui_flush_scheduled {
        state.gui_flush_scheduled = true;
        // try_send — không panic nếu channel closed (Zed shutting down)
        let _ = state.host.main_thread_message_tx.unbounded_send(Box::new(
            move |cx: &mut AsyncApp| {
                async move {
                    // Flush pending view lên ExtensionGuiPanel
                }.boxed_local()
            }
        ));
    }
    Ok(())
}

// emit: fire-and-forget, không panic nếu channel closed
fn gui_emit(state: &mut WasmState, name: String, data: String) -> wasmtime::Result<()> {
    if let Some(tx) = &state.gui_panel_tx {
        let _ = tx.unbounded_send(GuiPanelMessage::Emit { name, data });
    }
    Ok(())
}

// request-data: trigger data fetch từ extension logic
fn gui_request_data(state: &mut WasmState, key: String) -> wasmtime::Result<()> {
    if let Some(tx) = &state.gui_panel_tx {
        let _ = tx.unbounded_send(GuiPanelMessage::RequestData(key));
    }
    Ok(())
}
```

#### 5c. Trong `load_extension` — thêm GUI init sau extension init

```rust
// Sau khi extension.call_init_extension() thành công:
if manifest.gui.is_some() {
    // Check extension thực sự có export gui-init trước khi gọi
    if extension.has_gui_exports() {
        extension.call_gui_init(&mut store).await.log_err();
    } else {
        log::warn!(
            "Extension '{}' declares [gui] but missing gui exports in WASM",
            manifest.id
        );
    }
}
```

---

### 6. `crates/extension_host/src/extension_host.rs`

Thêm branch sau khi load WASM — **không đổi existing flow**:

```rust
fn load_extension(manifest: Arc<ExtensionManifest>, ..., cx: &mut App) {
    // --- code hiện tại không đổi ---
    load_wasm_extension(&manifest, cx);

    // --- additive: chỉ chạy khi manifest có [gui] ---
    if let Some(gui_config) = &manifest.gui {
        if is_gui_api_compatible(&gui_config.min_zed_api) {
            ExtensionGuiPanel::register(&manifest.id, cx);
        } else {
            log::warn!(
                "Extension '{}' GUI requires min_zed_api={}, skipping GUI",
                manifest.id, gui_config.min_zed_api
            );
            // Extension vẫn load bình thường, chỉ không có GUI
        }
    }
}
```

---

### 7. `crates/extension_panel/` *(crate mới)*

**`Cargo.toml`:**
```toml
[package]
name = "extension_panel"
version = "0.1.0"
edition.workspace = true
publish.workspace = true
license = "GPL-3.0-or-later"

[lints]
workspace = true

[lib]
path = "src/extension_panel.rs"
doctest = false

[dependencies]
gpui.workspace = true
ui.workspace = true
workspace.workspace = true
extension.workspace = true
util.workspace = true
log.workspace = true
smol.workspace = true
```

**`src/extension_panel.rs`:**

```rust
use gpui::*;
use ui::prelude::*;
use workspace::dock::{DockPosition, Panel, PanelEvent};

pub struct ExtensionGuiPanel {
    extension_id:  Arc<str>,
    focus_handle:  FocusHandle,
    root_element:  Option<GuiElement>,
    error_message: Option<String>,
    width:         Option<Pixels>,
    event_tx:      smol::channel::Sender<(String, GuiEvent)>,
    _event_task:   Task<()>,
}

impl ExtensionGuiPanel {
    pub fn register(extension_id: &Arc<str>, cx: &mut App) { ... }

    fn render_element(el: &GuiElement, tx: &EventSender, cx: &mut App) -> AnyElement {
        match el {
            GuiElement::VFlex(flex) => v_flex()
                .when_some(flex.gap, |this, g| this.gap(px(g as f32)))
                .when_some(flex.padding.as_ref(), |this, p| this.p(px(p.top.unwrap_or(0) as f32)))
                .children(flex.children.iter().map(|c| Self::render_element(c, tx, cx)))
                .into_any_element(),

            GuiElement::HFlex(flex) => h_flex()
                .children(flex.children.iter().map(|c| Self::render_element(c, tx, cx)))
                .into_any_element(),

            GuiElement::Scroll(scroll) => div()
                .id(SharedString::from(scroll.id.clone()))
                .when(scroll.vertical, |this| this.overflow_y_scroll())
                .child(Self::render_element(&scroll.child, tx, cx))
                .into_any_element(),

            GuiElement::Label(el) => Label::new(el.text.clone())
                .when(el.muted, |this| this.color(Color::Muted))
                .into_any_element(),

            GuiElement::Button(el) => {
                let id = el.id.clone();
                let tx = tx.clone();
                Button::new(SharedString::from(id.clone()), el.label.clone().unwrap_or_default())
                    .disabled(el.disabled)
                    .when(el.style == ButtonStyle::Primary, |b| b.style(ButtonStyle::Primary))
                    .on_click(move |_, _, _| {
                        tx.try_send((id.clone(), GuiEvent::Clicked)).ok();
                    })
                    .into_any_element()
            }

            GuiElement::List(el) => v_flex()
                .children(el.items.iter().map(|item| {
                    let id = item.id.clone();
                    let tx = tx.clone();
                    let selected = el.selected.as_deref() == Some(&item.id);
                    ListItem::new(SharedString::from(item.id.clone()))
                        .selected(selected)
                        .child(Label::new(item.label.clone()))
                        .when_some(item.description.as_ref(), |this, desc| {
                            this.child(Label::new(desc.clone()).color(Color::Muted))
                        })
                        .on_click(move |_, _, _| {
                            tx.try_send((id.clone(), GuiEvent::Clicked)).ok();
                        })
                        .into_any_element()
                }))
                .into_any_element(),

            GuiElement::Divider(_) => div()
                .h(px(1.0))
                .bg(cx.theme().colors().border)
                .into_any_element(),

            // ... các elements còn lại
            _ => div().into_any_element(),
        }
    }
}

impl Render for ExtensionGuiPanel {
    fn render(&mut self, _: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        div()
            .id("extension-gui-panel")
            .size_full()
            .bg(cx.theme().colors().panel_background)
            .track_focus(&self.focus_handle)
            .map(|this| {
                if let Some(ref err) = self.error_message {
                    this.child(
                        v_flex()
                            .size_full()
                            .items_center()
                            .justify_center()
                            .gap_2()
                            .child(Icon::new(IconName::Warning).color(Color::Error))
                            .child(Label::new(err.clone()).color(Color::Muted))
                    )
                } else if let Some(ref root) = self.root_element {
                    this.child(Self::render_element(root, &self.event_tx, cx))
                } else {
                    this.child(
                        v_flex()
                            .size_full()
                            .items_center()
                            .justify_center()
                            .child(Label::new("Loading...").color(Color::Muted))
                    )
                }
            })
    }
}

impl Panel for ExtensionGuiPanel {
    fn persistent_name() -> &'static str { "ExtensionGuiPanel" }
    fn position(&self, _: &Window, _: &App) -> DockPosition { DockPosition::Right }
    fn position_is_valid(&self, pos: DockPosition) -> bool {
        matches!(pos, DockPosition::Left | DockPosition::Right | DockPosition::Bottom)
    }
    fn set_position(&mut self, _: DockPosition, _: &mut Window, _: &mut Context<Self>) {}
    fn size(&self, _: &Window, _: &App) -> Pixels { self.width.unwrap_or(px(400.)) }
    fn set_size(&mut self, size: Option<Pixels>, _: &mut Window, cx: &mut Context<Self>) {
        self.width = size;
        cx.notify();
    }
    fn icon(&self, _: &Window, _: &App) -> Option<IconName> { Some(IconName::Window) }
    fn icon_tooltip(&self, _: &Window, _: &App) -> Option<&'static str> { Some("Extension Panel") }
    fn toggle_action(&self) -> Box<dyn Action> { Box::new(ToggleFocus) }
    fn activation_priority(&self) -> u32 { 200 }
}

impl EventEmitter<PanelEvent> for ExtensionGuiPanel {}
impl Focusable for ExtensionGuiPanel {
    fn focus_handle(&self, _: &App) -> FocusHandle { self.focus_handle.clone() }
}
```

---

### 8. `Cargo.toml` (workspace root)

```toml
[workspace]
members = [
    # ... existing ...
    "crates/extension_panel",   # ← thêm
]

[workspace.dependencies]
extension_panel = { path = "crates/extension_panel" }  # ← thêm
```

### 9. `crates/zed/Cargo.toml`

```toml
[dependencies]
# ... existing ...
extension_panel.workspace = true   # ← thêm
```

### 10. `crates/zed/src/zed.rs`

```rust
use extension_panel::ExtensionGuiPanel;

// Trong init_panels — thêm 1 dòng, không đổi gì khác
ExtensionGuiPanel::init(cx);
```

---

## Extension Developer Flow

Extension có GUI chỉ cần **1 WASM file** — logic và GUI trong cùng crate:

```
my-extension/
├── extension.toml
└── extension.wasm      (compile target wasm32-wasip2 — 1 file duy nhất)
```

```toml
# extension.toml
id = "my-extension"
name = "My Extension"
version = "1.0.0"
schema_version = 1

[gui]
min_zed_api = "0.9.0"   # không cần khai báo wasm path

[language_servers.my-lsp]
language = "MyLang"
```

```rust
// src/lib.rs — 1 file, logic và GUI dùng chung state
use zed_extension_api::*;

struct MyExtension {
    items: Vec<String>,   // state dùng chung bởi cả logic và GUI
}

// ── Logic (như hiện tại) ──────────────────────────────────────────────────────

impl Extension for MyExtension {
    fn new() -> Self {
        Self { items: vec![] }
    }

    fn language_server_command(
        &mut self,
        _id: &LanguageServerId,
        _worktree: &Worktree,
    ) -> Result<Command> {
        // ...
    }
}

// ── GUI (thêm mới — chỉ khi có [gui] trong manifest) ─────────────────────────

impl GuiExtension for MyExtension {
    fn gui_init(&mut self) {
        request_data("items");          // yêu cầu data ban đầu
        set_view(self.render());
    }

    fn gui_on_theme_change(&mut self, _theme: Theme) {
        set_view(self.render());        // re-render với theme mới
    }

    fn gui_on_data(&mut self, key: &str, value: &str) {
        if key == "items" {
            self.items = serde_json::from_str(value).unwrap_or_default();
            set_view(self.render());
        }
    }

    fn gui_on_event(&mut self, source_id: &str, event: UiEvent) {
        match (source_id, event) {
            ("refresh", UiEvent::Clicked) => {
                emit("refresh_clicked", "{}");
                request_data("items");
            }
            ("item-list", UiEvent::RowSelected(id)) => {
                emit("item_selected", &id);
            }
            _ => {}
        }
    }

    fn render(&self) -> Element {
        v_flex(vec![
            h_flex(vec![
                label("My Extension"),
                button("refresh", Some("Refresh"), None, ButtonStyle::Default),
            ]),
            scroll("main-scroll", true, false,
                list("item-list", self.items.iter().enumerate().map(|(i, item)| {
                    ListItem {
                        id: i.to_string(),
                        label: item.clone(),
                        ..Default::default()
                    }
                }).collect())
            ),
        ])
    }
}
```

---

## Tóm Tắt Thay Đổi

| File / Crate | Loại | Mô tả |
|---|---|---|
| `crates/extension/src/extension_manifest.rs` | Sửa | Thêm `Option<ExtensionGuiManifest>` (không có `wasm` field) |
| `crates/extension_api/wit/since_v0.9.0/extension.wit` | Mới | Copy từ v0.8.0, không đổi |
| `crates/extension_api/wit/since_v0.9.0/extension-with-gui.wit` | Mới | `include extension` + gui exports |
| `crates/extension_api/wit/since_v0.9.0/gui.wit` | Mới | GUI interface đầy đủ |
| `crates/extension_host/src/wasm_host/wit/since_v0_9_0.rs` | Mới | bindgen cho 2 worlds, versioned dispatch |
| `crates/extension_host/src/wasm_host/wit.rs` | Sửa | Thêm dispatch v0.9.0 trong `instantiate_async` |
| `crates/extension_host/src/wasm_host.rs` | Sửa | Thêm GUI fields vào `WasmState`, host functions dùng `try_send` |
| `crates/extension_host/src/extension_host.rs` | Sửa | Thêm branch spawn `ExtensionGuiPanel` |
| `crates/extension_panel/` | Mới | GPUI Panel render element tree từ WASM |
| `Cargo.toml` (workspace) | Sửa | Thêm `extension_panel` member |
| `crates/zed/Cargo.toml` | Sửa | Thêm `extension_panel` dependency |
| `crates/zed/src/zed.rs` | Sửa | `ExtensionGuiPanel::init(cx)` |

**Không thay đổi:**
- `WasmHost` core load/execute flow
- `since_v0.8.0` WIT và Rust bindings
- Extension trait và WIT interfaces hiện tại
- Tất cả extensions đang hoạt động (backward compatible hoàn toàn)

---

## Ảnh Hưởng Đến Extensions Cũ

| Thay đổi | Ảnh hưởng extensions cũ |
|---|---|
| `ExtensionManifest.gui` field mới | Không — `#[serde(default)]` → `None` |
| `since_v0.9.0` WIT folder | Không — extensions cũ dùng v0.8.0 linker |
| `extension-with-gui` world mới | Không — chỉ load khi `manifest.gui.is_some()` |
| `WasmState` GUI fields | Không — internal, `gui_panel_tx = None` khi không có GUI |
| `extension_panel` crate | Không — extensions không biết crate này tồn tại |
| `instantiate_async` thêm param | Không — extensions cũ version < 0.9.0 không vào nhánh mới |

**Kết luận: ảnh hưởng = 0** với tất cả extensions hiện tại.
