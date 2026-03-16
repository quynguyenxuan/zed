# GUI Extensions Integration

This document describes the GUI (Graphical User Interface) extension system integrated into Zed in March 2026.

## Overview

GUI extensions allow extensions to render custom user interfaces in side panels, providing rich interactive experiences beyond traditional text-based extensions. This enables extensions to create file explorers, git panels, dashboards, and other visual tools directly within Zed.

**Key Capabilities**:
- Render custom UI using declarative WIT-based UI builders
- Handle user interactions (clicks, input, keyboard events)
- Access workspace context (project root, active file, open files)
- Execute Zed commands and communicate with host
- Query and pub-sub patterns for extension-to-extension communication

## Architecture

### WIT API (v0.9.0)

GUI extensions are built on WebAssembly Component Model using WIT (WebAssembly Interface Types).

**Core Interfaces**:
- `panel-ui.wit` - Main GUI panel interface
- `ui-elements.wit` - Declarative UI component definitions
- `gui.wit` - Events, theming, and host interactions
- `pub-sub.wit` - Event broadcasting between extensions
- `query.wit` - Request-response data exchange

**Extension Lifecycle**:
```rust
export gui-init: func();                           // Initialize panel
export gui-render: func() -> ui-tree;              // Render UI tree
export gui-on-event: func(source-id, event);       // Handle UI events
export gui-on-theme-change: func(theme);           // React to theme changes
export on-pub-sub-event: func(event);              // Receive pub-sub events
export on-query: func(id, topic, source, data);    // Handle query requests
export run-extension-command: func(command-id);    // Run registered commands
```

### UI System

**Declarative Builder API** (Rust):
```rust
use zed_extension_api::ui::{v_flex, h_flex, div, Label, Input};

v_flex()
    .size_full()
    .gap_y(def_px(8.0))
    .child(Label::new("Hello from extension!").bold().text_lg())
    .child(Input::new("input-1", "").placeholder("Type here..."))
    .child(
        div()
            .id("button-1")
            .px_3()
            .py_2()
            .bg(color_accent())
            .cursor_pointer()
            .on_click(|_| {})
            .child(Label::new("Click me!"))
    )
```

**Rendering Flow**:
1. Host calls `gui_render()` export
2. Extension builds UI tree using builder API
3. Extension calls `ui::render_tree(root)` to convert to WIT types
4. Host renders UI tree as GPUI elements
5. User interactions trigger `gui_on_event()` callbacks
6. Extension updates state and returns new tree

### Host Integration

**Extension Panel** (`crates/extension_panel/`):
- `ExtensionGuiView` - GPUI entity wrapping WASM extension
- `ExtensionGuiPanel` - Panel containing extension views in tabs
- `ui_renderer.rs` - Converts WIT UI trees to GPUI elements

**Key Components**:
```rust
pub struct ExtensionGuiView {
    extension_id: Arc<str>,
    focus_handle: FocusHandle,
    wasm: WasmExtension,
    workspace: WeakEntity<Workspace>,
    ui_tree: Option<UiTree>,
    text_input_fields: HashMap<String, Entity<InputField>>,
    focus_handles: Arc<Mutex<HashMap<String, FocusHandle>>>,
}
```

## Integration Timeline

### March 3, 2026: Foundation
**Commit**: `5f72f1e8de` - `gui-test: implement gui_render and gui_on_event with WIT UI builders`

- Initial WIT definitions for GUI API
- Builder API for constructing UI trees
- Event handling system
- Basic rendering pipeline

### March 12, 2026: Core Features
**Commit**: `6f4af48a42` - `feat: extension work`

- Command registration API
- Extension panel integration
- Tab management for multiple extensions

**Commit**: `a4fb9f4718` - `feat: complete Extension GUI API integration to 90%`

Key additions:
1. **Workspace Context APIs**:
   - `get-project-root: func() -> option<string>`
   - `get-active-file: func() -> option<string>`
   - `get-open-files: func() -> list<string>`

2. **Command Execution**:
   - `execute-command: func(command: string, args: option<string>) -> result`
   - `execute-slash-command: func(command: string, args: list<string>) -> result`

3. **Pub-Sub System**:
   - `subscribe: func(topic: string) -> result<u64, string>`
   - `publish: func(topic: string, data: string) -> result`
   - Topic-based event broadcasting

4. **Query-Response Pattern**:
   - `query: func(topic: string, data: string, timeout-ms: u32) -> result<list<query-response>, string>`
   - `register-query-handler: func(topic: string) -> result<u64, string>`

**Commit**: `1341a81bef` - `fix: extension panel focus capture to prevent editor stealing focus`

- Focus management improvements
- Panel activation on tab switch

**Commit**: `f6e5a46215` - `revert: remove problematic focus handler that blocks panel rendering`

- Fixed focus-related rendering issues
- Improved panel stability

### March 14, 2026: Pub-Sub Refinement

**Workspace Context Auto-Delivery**:
- Extensions auto-subscribe to `zed.*` topics on `gui_init()`
- Host immediately delivers initial context after init
- Topics: `zed.project-root-changed`, `zed.active-file-changed`, `zed.open-files-changed`

**Removed Direct Context APIs**:
All `get-*` functions removed in favor of pub-sub-only approach for cleaner architecture and better event-driven design.

### March 16, 2026: Bug Fixes & Polish

1. **Commit Button Responsiveness**:
   - Fixed: Always call `cx.notify()` after InputChanged events
   - Button state updates immediately when typing

2. **Focus & Zoom Controls**:
   - Fixed: Add `.track_focus(&self.focus_handle)` to root div
   - Zoom button and panel controls now visible consistently

3. **File Opening via Pub-Sub**:
   - Extensions publish `ext.open-file` events
   - Host subscribes and emits `ExtensionViewEvent::OpenFile`
   - Workspace opens file via `open_abs_path()`

## Key Features

### 1. Declarative UI Building

Extensions build UI using a fluent builder API inspired by SwiftUI and GPUI:

```rust
h_flex()
    .items_center()
    .gap_x(def_px(6.0))
    .child(Label::new("Status:").muted())
    .child(Label::new("Ready").color(color_success()).bold())
```

**Available Components**:
- Layout: `v_flex`, `h_flex`, `div`
- Content: `Label`, `Icon`
- Input: `Input` (text fields)
- Lists: `UniformList` (virtualized scrolling)

**Styling**:
- Flexbox properties: `items_center`, `justify_between`, `flex_1`
- Spacing: `p_3`, `px_2`, `gap_x`, `gap_y`
- Colors: `color_accent()`, `color_success()`, `color_error()`, `bg()`, `text_color()`
- Appearance: `rounded_md`, `border_1`, `shadow`
- Typography: `text_sm`, `text_lg`, `bold`, `italic`

### 2. Event Handling

Interactive elements register event handlers:

```rust
div()
    .id("my-button")  // Unique ID for event source
    .cursor_pointer()
    .on_click(|_| {})
    .child(Label::new("Click me"))
```

Events are delivered to `gui_on_event(source_id, event)`:

```rust
fn gui_on_event(&mut self, source_id: String, event: zed::gui::UiEvent) {
    match source_id.as_str() {
        "my-button" => {
            // Handle button click
            self.perform_action();
        }
        "text-input" => {
            if let zed::gui::UiEvent::InputChanged(new_value) = event {
                self.input_value = new_value;
            }
        }
        _ => {}
    }
}
```

**Event Types**:
- Mouse: `Clicked`, `DoubleClicked`, `RightClicked`, `MouseDown`, `MouseUp`, `MouseMoved`
- Keyboard: `KeyDown`, `KeyUp`
- Input: `InputChanged`
- Focus: `FocusGained`, `FocusLost`
- Hover: `HoverStart`, `HoverEnd`
- Scroll: `ScrollWheel`

### 3. Workspace Context

Extensions receive workspace updates automatically via pub-sub:

```rust
impl Extension for MyExtension {
    fn on_pub_sub_event(&mut self, event: zed::pub_sub::PubSubEvent) {
        match event.topic.as_str() {
            "zed.project-root-changed" => {
                self.project_root = Some(event.data);
                self.reload_project_data();
            }
            "zed.active-file-changed" => {
                self.active_file = Some(event.data);
                self.update_highlights();
            }
            "zed.open-files-changed" => {
                let files: Vec<String> = serde_json::from_str(&event.data).ok()?;
                self.open_files = files;
            }
            _ => {}
        }
    }
}
```

Topics are auto-subscribed in `extension_api` wrapper during `gui_init()`.

### 4. Command Registration

Extensions can register commands in the command palette:

```rust
impl Extension for MyExtension {
    fn new() -> Self {
        register_command("open-panel", "Open My Panel");
        Self { /* ... */ }
    }

    fn run_extension_command(&mut self, command_id: &str) -> Result<(), String> {
        match command_id {
            "open-panel" => {
                // Command executed
                Ok(())
            }
            _ => Err(format!("Unknown command: {}", command_id)),
        }
    }
}
```

Users can invoke via:
- Command palette: `Cmd+Shift+P` → type command name
- Keybindings: Bind to registered command ID

### 5. Theme Adaptation

Extensions receive theme updates and can adapt their UI:

```rust
fn gui_on_theme_change(&mut self, theme: zed::gui::Theme) {
    self.is_dark_mode = theme.is_dark;
    self.accent_color = theme.colors.text_accent;
    // UI will be re-rendered automatically
}
```

**Theme colors available**:
- Background: `background`, `editor_background`, `surface_background`, `elevated_surface_background`
- Text: `text`, `text_muted`, `text_disabled`, `text_accent`
- Borders: `border`, `border_muted`, `border_focused`
- Elements: `element_background`, `element_hover`, `element_selected`, `element_active`
- Status: `status_error`, `status_warning`, `status_success`, `status_info`

### 6. Extension Communication

**Pub-Sub (Broadcast)**:
```rust
// Publisher
pub_sub::publish("ext-git.status-changed", &status_json)?;

// Subscriber
fn on_pub_sub_event(&mut self, event: zed::pub_sub::PubSubEvent) {
    if event.topic == "ext-git.status-changed" {
        let status: GitStatus = serde_json::from_str(&event.data)?;
        self.update_git_display(status);
    }
}
```

**Query-Response (Request-Reply)**:
```rust
// Handler (with macro)
#[query_dispatch]
impl Extension for GitExtension {
    #[query_handler("git.status")]
    fn handle_git_status(&mut self, _req: ()) -> Result<GitStatusResponse, String> {
        Ok(GitStatusResponse { branch: self.branch.clone(), /* ... */ })
    }
}

// Consumer
let responses = query::query("git.status", "", 1000)?; // 1s timeout
for response in responses {
    let status: GitStatusResponse = serde_json::from_str(&response.data)?;
    // Use status...
}
```

## Example Extensions

### 1. gui-test (Git Panel)

**Features**:
- Display git status (staged/unstaged files)
- Stage/unstage files with click
- Commit with message input
- Collapsible sections
- Real-time status updates

**Key Implementation**:
```rust
#[query_dispatch]
impl Extension for GitPanel {
    fn gui_init(&mut self) {
        self.query_handler_id = query::register_query_handler("git.status").ok();
    }

    fn gui_render(&mut self) -> zed::ui_elements::UiTree {
        let can_commit = !self.staged.is_empty() && !self.commit_message.is_empty();

        v_flex()
            .child(Input::new("commit-input", self.commit_message.clone()))
            .child(
                div()
                    .id("commit")
                    .when(can_commit, |this| {
                        this.bg(color_accent())
                            .cursor_pointer()
                            .on_click(|_| {})
                    })
                    .child(Label::new("Commit"))
            )
    }

    fn gui_on_event(&mut self, source_id: String, event: zed::gui::UiEvent) {
        match source_id.as_str() {
            "commit-input" => {
                if let UiEvent::InputChanged(msg) = event {
                    self.commit_message = msg;
                }
            }
            "commit" => {
                // Execute git commit
                git(&self.project_root, &["commit", "-m", &self.commit_message])?;
                self.commit_message.clear();
            }
            _ => {}
        }
    }
}
```

### 2. outline-test (File Outline)

**Features**:
- Show symbols in active file
- Auto-update on file change
- Grep-based symbol parsing
- Reacts to `zed.active-file-changed` events

### 3. project-test (File Explorer)

**Features**:
- File tree with icons
- Collapse/expand directories
- Click to open files
- Version display
- Find-based file discovery

**Opening Files**:
```rust
fn gui_on_event(&mut self, source_id: String, _event: zed::gui::UiEvent) {
    if let Some(entry) = self.get_file_entry(&source_id) {
        if matches!(entry.kind, EntryKind::File) {
            // Publish event for host to handle
            pub_sub::publish("ext.open-file", &entry.path)?;
        }
    }
}
```

## Implementation Notes

### Focus Management

Extensions must track focus to maintain panel controls:

```rust
impl Render for ExtensionGuiView {
    fn render(&mut self, window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        div()
            .track_focus(&self.focus_handle)  // Critical for zoom/controls
            .size_full()
            .child(/* UI tree */)
    }
}
```

Without `.track_focus()`, panel controls (zoom button) disappear when panel loses focus.

### Re-Render Triggers

Always notify after state changes that affect UI:

```rust
entity.update(cx, |view, cx| {
    view.ui_tree = Some(new_tree);
    cx.notify();  // Required to trigger re-render
})
```

For input events, always notify since content/styling may have changed even if structure is identical.

### Input Fields

Host manages `InputField` entities for text inputs:

```rust
// In extension
Input::new("my-input", current_value)
    .placeholder("Enter text...")

// Host creates Entity<InputField> and tracks in view
self.text_input_fields.insert(input_id, input_field);
```

Text changes are delivered via `InputChanged` events to extension.

### Event Handler Registration

Clear handlers before each render to prevent duplicates:

```rust
fn gui_render(&mut self) -> zed::ui_elements::UiTree {
    ui::clear_handlers();  // Clear previous render's handlers

    let root = v_flex()
        .child(div().id("button").on_click(|_| {}));

    ui::render_tree(root)
}
```

## Performance Considerations

### Rendering
- Extensions return full UI tree on each render
- Host diffs tree and only updates changed elements
- Avoid deep nesting (>10 levels) for better performance
- Use `UniformList` for long lists (virtualizes rendering)

### Event Handling
- Events are async - handlers complete before next render
- Avoid long-running operations in event handlers
- Spawn background tasks for I/O or computation
- Use debouncing for rapid events (keyboard input)

### Memory
- UI trees are rebuilt on each render
- Avoid storing large data structures in UI nodes
- Keep extension state separate from UI representation
- Use `Arc<str>` for strings shared across renders

## Build & Deployment

**Build Extension**:
```bash
cargo run --package extension_cli --profile release-fast -- \
  --source-dir extensions/my-extension \
  --output-dir extensions/my-extension/ \
  --scratch-dir extensions/my-extension/scratch
```

**Output**:
- `archive.tar.gz` - Contains `extension.wasm` and `extension.toml`
- `manifest.json` - Extension metadata

**Install**:
- Copy/symlink extension directory to `~/.local/share/zed/extensions/installed/`
- Restart Zed or run `zed: reload extensions` command

## Future Enhancements

Potential improvements for GUI extension system:

1. **Rich Components**:
   - Dropdown menus
   - Radio buttons & checkboxes
   - Tabs & accordions
   - Tree views with lazy loading
   - Context menus
   - Modal dialogs

2. **Advanced Layout**:
   - Grid layout
   - Absolute positioning
   - Custom scrollbars
   - Resizable panels

3. **Performance**:
   - Incremental rendering
   - Virtual scrolling for all lists
   - Render caching
   - Batch event processing

4. **Developer Experience**:
   - Hot reload during development
   - Visual UI builder/inspector
   - Performance profiler
   - Better error messages for layout issues

5. **Integration**:
   - LSP integration for code navigation
   - Debugger UI for extensions
   - Terminal embedding
   - WebView for HTML content

## Troubleshooting

### Extension not loading

Check logs:
```bash
tail -f ~/.local/share/zed/logs/Zed.log | grep -i extension
```

Common issues:
- Malformed manifest.json
- Missing WIT exports
- WASM compilation errors
- Capability requirements not met

### UI not rendering

- Verify `gui_render()` returns valid tree
- Check that root node has size (`.size_full()` or explicit dimensions)
- Ensure `ui::render_tree()` is called
- Look for GPUI errors in logs

### Events not firing

- Verify element has unique ID
- Check `.on_click(|_| {})` or other handlers are registered
- Ensure handler is cleared and re-registered on each render
- Confirm `source_id` matches element ID in `gui_on_event()`

### Focus issues

- Add `.track_focus(&self.focus_handle)` to root div
- Implement `Focusable` trait on view
- Check focus handle is properly initialized

## See Also

- [Extension Communication Patterns](extension-communication.md)
- [Development: Reloading Extensions](development-reload-extensions.md)
- [Extension Capabilities](capabilities.md)
- WIT definitions: `crates/extension_api/wit/since_v0.9.0/`
- Example extensions: `extensions/gui-test/`, `extensions/outline-test/`, `extensions/project-test/`
