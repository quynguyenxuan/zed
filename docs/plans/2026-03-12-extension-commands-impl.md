# Extension Commands & Panel Footer Button Implementation Plan

> **For Claude:** REQUIRED SUB-SKILL: Use superpowers:executing-plans to implement this plan task-by-task.

**Goal:** Register `ExtensionGuiPanel` at startup (always-visible footer button); add `register-command` / `run-extension-command` to `extension.wit` so any extension can register commands in `ctrl+shift+P` and handle invocations; wire `gui-test` to open its panel tab.

**Architecture:** `register-command` is a WIT import in `extension.wit` (all extension types). When called, `WasmState::register_command` sends to a channel owned by `ExtensionStore`. `ExtensionStore` reads the channel and emits `Event::ExtensionCommandRegistered`. `extension_panel::init(cx)` subscribes to `ExtensionStore` events, updates `GlobalDynamicCommandRegistry` (in `command_palette_hooks`), and handles the `OpenExtensionPanel` action. `command_palette.rs` reads the registry to show commands. `extension_host` does NOT depend on `extension_panel` — `extension_panel::init(cx)` is the integration point, following the same pattern as `notification_panel::init(cx)`.

**Tech Stack:** Rust, GPUI, wasmtime WIT component model, `mpsc::UnboundedSender` for channel-based dispatch from WASM threads to GPUI main thread.

---

## Status check before starting

Verify existing work is in place:
```bash
grep -n "ExtensionGuiPanel::load\|extension_panel" crates/zed/src/zed.rs | grep -v "^Binary"
```
Expected: `ExtensionGuiPanel::load(...)` present at line ~666, listed in the `futures::join!` block at line ~691.

---

## Task 1: Add `register-command` import and `run-extension-command` export to `extension.wit`

**Files:**
- Modify: `crates/extension_api/wit/since_v0.9.0/extension.wit`

**Step 1: Add import after `import get-settings` (line 51)**

```wit
/// Registers a command that appears in Zed's command palette.
/// When the user invokes the command, Zed calls `run-extension-command` with the given id.
import register-command: func(id: string, label: string);
```

**Step 2: Add export after `export run-slash-command` (line 154)**

```wit
/// Called when the user invokes a command previously registered via `register-command`.
export run-extension-command: func(command-id: string) -> result<_, string>;
```

**Step 3: Check WIT compiles**
```bash
cargo check -p extension_api 2>&1 | grep "^error"
```

**Step 4: Commit**
```bash
git add crates/extension_api/wit/since_v0.9.0/extension.wit
git commit -m "extension_api: add register-command import and run-extension-command export"
```

---

## Task 2: Add channel + `Event::ExtensionCommandRegistered` to `extension_host`

**Files:**
- Modify: `crates/extension_host/src/wasm_host.rs`
- Modify: `crates/extension_host/src/extension_host.rs`

**Step 1: Add field to `WasmHost` struct in `wasm_host.rs`**

In `pub struct WasmHost { ... }`, add after `main_thread_message_tx`:
```rust
pub(crate) command_registrations_tx: mpsc::UnboundedSender<(Arc<str>, String, Arc<str>)>,
```

**Step 2: Add `command_registrations_tx` parameter to `WasmHost::new`**

Change the function signature:
```rust
pub fn new(
    fs: Arc<dyn Fs>,
    http_client: Arc<dyn HttpClient>,
    node_runtime: NodeRuntime,
    proxy: Arc<ExtensionHostProxy>,
    work_dir: PathBuf,
    command_registrations_tx: mpsc::UnboundedSender<(Arc<str>, String, Arc<str>)>,
    cx: &mut App,
) -> Arc<Self> {
```

Add the field in the `Arc::new(Self { ... })` initializer:
```rust
command_registrations_tx,
```

**Step 3: Add `Event::ExtensionCommandRegistered` variant to `Event` enum in `extension_host.rs`**

```rust
ExtensionCommandRegistered {
    extension_id: Arc<str>,
    display_name: String,
    command_id: Arc<str>,
},
```

**Step 4: Update `ExtensionStore::new` in `extension_host.rs`**

Before the `let mut this = Self { ... }` block (~line 253), add:
```rust
let (command_tx, mut command_rx) = mpsc::unbounded::<(Arc<str>, String, Arc<str>)>();
```

Pass `command_tx` to `WasmHost::new`:
```rust
wasm_host: WasmHost::new(
    fs.clone(),
    http_client.clone(),
    node_runtime,
    extension_host_proxy,
    work_dir,
    command_tx,
    cx,
),
```

After constructing `this`, add a task that reads from the channel and emits events. Push it to `this.tasks`:
```rust
let command_task = cx.spawn(async move |this, mut cx| {
    while let Some((extension_id, display_name, command_id)) = command_rx.next().await {
        this.update(&mut cx, |_, cx| {
            cx.emit(Event::ExtensionCommandRegistered {
                extension_id,
                display_name,
                command_id,
            });
        })
        .ok();
    }
});
this.tasks.push(command_task);
```

**Step 5: Check**
```bash
cargo check -p extension_host 2>&1 | grep "^error"
```

**Step 6: Commit**
```bash
git add crates/extension_host/src/wasm_host.rs crates/extension_host/src/extension_host.rs
git commit -m "extension_host: add command registration channel and ExtensionCommandRegistered event"
```

---

## Task 3: Add `DynamicCommandRegistry` to `command_palette_hooks`

**Files:**
- Modify: `crates/command_palette_hooks/src/command_palette_hooks.rs`

**Step 1: Add types at end of file**

```rust
use std::sync::Arc;

/// A command dynamically registered at runtime by an extension.
pub struct DynamicCommand {
    /// Display name in command palette: "gui_test: open panel"
    pub name: String,
    /// Extension that registered this command (for bulk-unregister on unload).
    pub extension_id: Arc<str>,
    /// Opaque command identifier passed back to the extension via `run-extension-command`.
    pub command_id: Arc<str>,
}

#[derive(Default)]
pub struct DynamicCommandRegistry {
    commands: Vec<DynamicCommand>,
}

impl DynamicCommandRegistry {
    pub fn register(&mut self, command: DynamicCommand) {
        self.commands.push(command);
    }

    pub fn unregister_extension(&mut self, extension_id: &str) {
        self.commands
            .retain(|c| c.extension_id.as_ref() != extension_id);
    }

    pub fn commands(&self) -> impl Iterator<Item = &DynamicCommand> {
        self.commands.iter()
    }
}

#[derive(Default)]
pub struct GlobalDynamicCommandRegistry(pub DynamicCommandRegistry);

impl Global for GlobalDynamicCommandRegistry {}
```

**Step 2: Initialize in `pub fn init(cx: &mut App)`**

Add to the `init` function:
```rust
cx.set_global(GlobalDynamicCommandRegistry::default());
```

**Step 3: Check**
```bash
cargo check -p command_palette_hooks 2>&1 | grep "^error"
```

**Step 4: Commit**
```bash
git add crates/command_palette_hooks/src/command_palette_hooks.rs
git commit -m "command_palette_hooks: add DynamicCommandRegistry"
```

---

## Task 4: Add `OpenExtensionPanel` action and `open_or_focus` to `extension_panel`

**Files:**
- Modify: `crates/extension_panel/src/extension_panel.rs`

**Step 1: Add `use serde::Serialize;` to existing `use serde::Deserialize;` import**

**Step 2: Add `OpenExtensionPanel` action after the `actions!` block (~line 29)**

```rust
/// Opens a specific extension in the extension GUI panel by extension and command ID.
#[derive(Clone, PartialEq, Eq, gpui::Action, Serialize, Deserialize)]
pub struct OpenExtensionPanel {
    pub extension_id: Arc<str>,
    pub command_id: Arc<str>,
}
```

**Step 3: Make `extension_id` in `ExtensionGuiView` pub(crate)**

Change `extension_id: Arc<str>` to `pub(crate) extension_id: Arc<str>` in `pub struct ExtensionGuiView`.

**Step 4: Add `open_or_focus` to `ExtensionGuiPanel` impl block**

```rust
/// Opens the extension in a new tab, or focuses the existing tab if already open.
/// Emits `PanelEvent::Activate` to show the panel.
pub fn open_or_focus(
    &mut self,
    manifest: Arc<ExtensionManifest>,
    wasm_extension: WasmExtension,
    window: &mut Window,
    cx: &mut Context<Self>,
) {
    let existing_index = self
        .active_pane
        .read(cx)
        .items()
        .enumerate()
        .find_map(|(ix, item)| {
            item.to_any()
                .downcast::<ExtensionGuiView>()
                .ok()
                .filter(|v| v.read(cx).extension_id == manifest.id)
                .map(|_| ix)
        });

    if let Some(ix) = existing_index {
        self.active_pane.update(cx, |pane, cx| {
            pane.activate_item(ix, true, true, window, cx);
        });
    } else {
        self.add_view(manifest, wasm_extension, window, cx);
    }

    cx.emit(PanelEvent::Activate);
}
```

Note: if `item.to_any()` is not the correct method name for downcasting items from a `Pane`, check the `ItemHandle` trait in `workspace/src/item.rs` for the actual method (may be `to_any_handle()`, `as_any()`, or similar). The pattern is: get an `Entity<ExtensionGuiView>` from the item handle.

**Step 5: Check**
```bash
cargo check -p extension_panel 2>&1 | grep "^error"
```

**Step 6: Commit**
```bash
git add crates/extension_panel/src/extension_panel.rs
git commit -m "extension_panel: add OpenExtensionPanel action and open_or_focus"
```

---

## Task 5: Add `extension_panel::init(cx)` with all workspace wiring

**Files:**
- Modify: `crates/extension_panel/src/extension_panel.rs`
- Modify: `crates/extension_panel/Cargo.toml`

**Step 1: Add `command_palette_hooks` to `Cargo.toml`**

```toml
command_palette_hooks.workspace = true
```

**Step 2: Verify `ExtensionStore` is publicly accessible**
```bash
grep "pub struct ExtensionStore\|pub use.*ExtensionStore" crates/extension_host/src/extension_host.rs | head -5
```

**Step 3: Add imports to `extension_panel.rs`**

At the top, add:
```rust
use command_palette_hooks::{DynamicCommand, GlobalDynamicCommandRegistry};
use extension_host::ExtensionStore;
```

**Step 4: Add `pub fn init(cx: &mut App)` function**

```rust
pub fn init(cx: &mut App) {
    cx.observe_new(
        |workspace: &mut Workspace,
         window: Option<&mut Window>,
         cx: &mut Context<Workspace>| {
            let Some(window) = window else { return };

            workspace.register_action(|workspace, _: &ToggleFocus, window, cx| {
                workspace.toggle_panel_focus::<ExtensionGuiPanel>(window, cx);
            });

            workspace.register_action(|workspace, action: &OpenExtensionPanel, window, cx| {
                let extension_id = action.extension_id.clone();
                let command_id = action.command_id.clone();
                let extension_store = ExtensionStore::global(cx);
                let wasm = extension_store.read(cx).wasm_extension_for_id(&extension_id);
                if let Some(panel) = workspace.panel::<ExtensionGuiPanel>(cx) {
                    if let Some((manifest, wasm_extension)) = wasm {
                        cx.spawn_in(window, async move |_, cx| {
                            panel
                                .update_in(cx, |panel, window, cx| {
                                    panel.open_or_focus(manifest, wasm_extension.clone(), window, cx);
                                })
                                .ok();
                            wasm_extension
                                .call_run_extension_command(command_id.to_string())
                                .await
                                .log_err();
                        })
                        .detach();
                    } else {
                        workspace.toggle_panel_focus::<ExtensionGuiPanel>(window, cx);
                    }
                }
            });

            let extension_store = ExtensionStore::global(cx);
            cx.subscribe_in(
                &extension_store,
                window,
                |workspace, _, event, window, cx| match event {
                    extension_host::Event::GuiExtensionLoaded(manifest, wasm_extension) => {
                        if let Some(panel) = workspace.panel::<ExtensionGuiPanel>(cx) {
                            let manifest = manifest.clone();
                            let wasm_extension = wasm_extension.clone();
                            cx.spawn_in(window, async move |_, cx| {
                                panel
                                    .update_in(cx, |panel, window, cx| {
                                        panel.add_view(manifest, wasm_extension, window, cx);
                                    })
                                    .ok();
                            })
                            .detach();
                        }
                    }
                    extension_host::Event::ExtensionCommandRegistered {
                        extension_id,
                        display_name,
                        command_id,
                    } => {
                        cx.update_global(|registry: &mut GlobalDynamicCommandRegistry, _| {
                            registry.0.register(DynamicCommand {
                                name: display_name.clone(),
                                extension_id: extension_id.clone(),
                                command_id: command_id.clone(),
                            });
                        });
                    }
                    extension_host::Event::ExtensionUninstalled(extension_id) => {
                        cx.update_global(|registry: &mut GlobalDynamicCommandRegistry, _| {
                            registry.0.unregister_extension(extension_id);
                        });
                    }
                    _ => {}
                },
            )
            .detach();
        },
    )
    .detach();
}
```

**Step 5: Check**
```bash
cargo check -p extension_panel 2>&1 | grep "^error"
```

**Step 6: Commit**
```bash
git add crates/extension_panel/src/extension_panel.rs crates/extension_panel/Cargo.toml
git commit -m "extension_panel: add init(cx) with workspace wiring"
```

---

## Task 6: Implement `register_command` host function in `since_v0_9_0.rs`

**Files:**
- Modify: `crates/extension_host/src/wasm_host/wit/since_v0_9_0.rs`

**Step 1: Add `register_command` to `impl PanelUiImports for WasmState`**

After `make_file_executable` (~line 228), add:
```rust
async fn register_command(
    &mut self,
    id: String,
    label: String,
) -> wasmtime::Result<()> {
    let extension_id = self.manifest.id.clone();
    let display_name = format!(
        "{}: {}",
        self.manifest.name.to_lowercase().replace(' ', "_"),
        label
    );
    self.host
        .command_registrations_tx
        .unbounded_send((extension_id, display_name, id.into()))
        .ok();
    Ok(())
}
```

**Step 2: Check**
```bash
cargo check -p extension_host 2>&1 | grep "^error"
```

**Step 3: Commit**
```bash
git add crates/extension_host/src/wasm_host/wit/since_v0_9_0.rs
git commit -m "extension_host: implement register_command host function"
```

---

## Task 7: Add `call_run_extension_command` to `Extension` and `WasmExtension`

**Files:**
- Modify: `crates/extension_host/src/wasm_host/wit.rs`
- Modify: `crates/extension_host/src/wasm_host.rs`

**Step 1: Add `call_run_extension_command` to `impl Extension` in `wit.rs`**

After `call_gui_on_event` (~line 1473):
```rust
pub async fn call_run_extension_command(
    &self,
    store: &mut Store<WasmState>,
    command_id: &str,
) -> Result<Result<(), String>> {
    match self {
        Extension::V0_9_0(ext) => ext.call_run_extension_command(store, command_id).await,
        _ => Ok(Ok(())),
    }
}
```

**Step 2: Add `call_run_extension_command` to `WasmExtension` in `wasm_host.rs`**

After `call_gui_on_event`:
```rust
pub async fn call_run_extension_command(&self, command_id: String) -> Result<()> {
    self.call(move |ext, store| {
        async move {
            ext.call_run_extension_command(store, &command_id)
                .await?
                .map_err(|e| anyhow::anyhow!(e))
        }
        .boxed()
    })
    .await
}
```

**Step 3: Check**
```bash
cargo check -p extension_host 2>&1 | grep "^error"
```

**Step 4: Commit**
```bash
git add crates/extension_host/src/wasm_host/wit.rs crates/extension_host/src/wasm_host.rs
git commit -m "extension_host: add call_run_extension_command to Extension and WasmExtension"
```

---

## Task 8: Add `wasm_extension_for_id` to `ExtensionStore`

**Files:**
- Modify: `crates/extension_host/src/extension_host.rs`

**Step 1: Add method to `impl ExtensionStore`**

Find the `impl ExtensionStore` block (search for `fn reload` or `fn install_extension`) and add:
```rust
/// Returns the loaded WASM extension and manifest for the given extension ID.
pub fn wasm_extension_for_id(
    &self,
    extension_id: &str,
) -> Option<(Arc<ExtensionManifest>, WasmExtension)> {
    self.wasm_extensions
        .iter()
        .find(|(manifest, _)| manifest.id.as_ref() == extension_id)
        .map(|(manifest, wasm)| (manifest.clone(), wasm.clone()))
}
```

**Step 2: Check**
```bash
cargo check -p extension_host 2>&1 | grep "^error"
```

**Step 3: Commit**
```bash
git add crates/extension_host/src/extension_host.rs
git commit -m "extension_host: add wasm_extension_for_id lookup"
```

---

## Task 9: Make `command_palette` read `GlobalDynamicCommandRegistry`

**Files:**
- Modify: `crates/command_palette/src/command_palette.rs`
- Modify: `crates/command_palette/Cargo.toml`

**Step 1: Add `extension_panel` to `command_palette/Cargo.toml`**

```toml
extension_panel.workspace = true
```

**Step 2: Add `GlobalDynamicCommandRegistry` to imports in `command_palette.rs`**

Change:
```rust
use command_palette_hooks::{
    CommandInterceptItem, CommandInterceptResult, CommandPaletteFilter,
    GlobalCommandPaletteInterceptor,
};
```
To:
```rust
use command_palette_hooks::{
    CommandInterceptItem, CommandInterceptResult, CommandPaletteFilter,
    GlobalCommandPaletteInterceptor, GlobalDynamicCommandRegistry,
};
```

**Step 3: Extend the commands list in `CommandPalette::new`**

Replace `let commands = window.available_actions(cx)...collect();` with:
```rust
let mut commands: Vec<Command> = window
    .available_actions(cx)
    .into_iter()
    .filter_map(|action| {
        if filter.is_some_and(|filter| filter.is_hidden(&*action)) {
            return None;
        }
        Some(Command {
            name: humanize_action_name(action.name()),
            action,
        })
    })
    .collect();

if let Some(registry) = cx.try_global::<GlobalDynamicCommandRegistry>() {
    commands.extend(registry.0.commands().map(|cmd| Command {
        name: cmd.name.clone(),
        action: Box::new(extension_panel::OpenExtensionPanel {
            extension_id: cmd.extension_id.clone(),
            command_id: cmd.command_id.clone(),
        }),
    }));
}
let commands = commands;
```

**Step 4: Check**
```bash
cargo check -p command_palette 2>&1 | grep "^error"
```

**Step 5: Commit**
```bash
git add crates/command_palette/src/command_palette.rs crates/command_palette/Cargo.toml
git commit -m "command_palette: include extension commands from GlobalDynamicCommandRegistry"
```

---

## Task 10: Wire `extension_panel::init(cx)` in `zed.rs`; remove old handler

**Files:**
- Modify: `crates/zed/src/zed.rs`

**Step 1: Find existing wiring to remove**
```bash
grep -n "GuiExtensionLoaded\|extension_panel::ToggleFocus\|ExtensionGuiPanel\|extension_panel" crates/zed/src/zed.rs | head -20
```

**Step 2: Add `extension_panel::init(cx)` call**

Find where other panel init functions are called (e.g., near `collab_ui::init(cx)` or `command_palette::init(cx)`). Add:
```rust
extension_panel::init(cx);
```

**Step 3: Remove the old `GuiExtensionLoaded` subscription block**

Remove the entire `cx.subscribe_in(&extension_store, ...)` block that handles `GuiExtensionLoaded` (currently lines ~420-452). This logic is now inside `extension_panel::init`.

Also remove any standalone `ToggleFocus` registration for `ExtensionGuiPanel` if it exists elsewhere in `zed.rs` (it is now registered inside `extension_panel::init`).

**Step 4: Check**
```bash
cargo check -p zed 2>&1 | grep "^error"
```

**Step 5: Commit**
```bash
git add crates/zed/src/zed.rs
git commit -m "zed: delegate extension panel wiring to extension_panel::init"
```

---

## Task 11: Implement `run_extension_command` in `gui-test`

**Files:**
- Modify: `extensions/gui-test/src/lib.rs`

**Step 1: Call `register_command` in `fn new()`**

```rust
fn new() -> Self {
    zed_extension_api::register_command("open-panel", "open panel");
    GuiTest {
        result_text: "Click a button to call a host action.".to_string(),
    }
}
```

**Step 2: Implement `run_extension_command`**

Add to `impl Extension for GuiTest`:
```rust
fn run_extension_command(&mut self, command_id: &str) -> Result<(), String> {
    match command_id {
        "open-panel" => Ok(()),
        _ => Err(format!("unknown command: {command_id}")),
    }
}
```

**Step 3: Check**
```bash
cargo check -p gui-test 2>&1 | grep "^error"
```

**Step 4: Commit**
```bash
git add extensions/gui-test/src/lib.rs
git commit -m "gui-test: register open-panel command and handle run-extension-command"
```

---

## Task 12: Full build and clippy

```bash
./script/clippy 2>&1 | grep "^error" | head -20
```

Fix any errors. Then run a targeted check:
```bash
cargo check -p extension_panel -p extension_host -p command_palette -p command_palette_hooks -p zed 2>&1 | grep "^error"
```

---

## Verification checklist

- [ ] Status bar footer always shows Extension Panel icon (even before any extension loads)
- [ ] `ctrl+shift+P` → type "gui_test" → shows `gui_test: open panel`
- [ ] Selecting command → panel opens, tab added for gui-test
- [ ] Running command again → focuses existing tab, no duplicate tab created
- [ ] `run-extension-command` called on the extension after panel opens

---

## Key files reference

| File | Change |
|------|--------|
| `crates/extension_api/wit/since_v0.9.0/extension.wit` | `import register-command`, `export run-extension-command` |
| `crates/extension_host/src/wasm_host/wit/since_v0_9_0.rs` | implement `register_command` in `impl PanelUiImports for WasmState` |
| `crates/extension_host/src/wasm_host/wit.rs` | add `call_run_extension_command` to `impl Extension` |
| `crates/extension_host/src/wasm_host.rs` | add `command_registrations_tx` field; add `call_run_extension_command` to `WasmExtension` |
| `crates/extension_host/src/extension_host.rs` | add `Event::ExtensionCommandRegistered`; create channel in `new`; add `wasm_extension_for_id` |
| `crates/command_palette_hooks/src/command_palette_hooks.rs` | `DynamicCommand`, `DynamicCommandRegistry`, `GlobalDynamicCommandRegistry` |
| `crates/command_palette/src/command_palette.rs` | read registry, create `OpenExtensionPanel` actions |
| `crates/command_palette/Cargo.toml` | add `extension_panel` dep |
| `crates/extension_panel/src/extension_panel.rs` | `OpenExtensionPanel` action, `open_or_focus`, `pub fn init(cx)` |
| `crates/extension_panel/Cargo.toml` | add `command_palette_hooks` dep |
| `crates/zed/src/zed.rs` | call `extension_panel::init(cx)`; remove old `GuiExtensionLoaded` subscription |
| `extensions/gui-test/src/lib.rs` | call `register_command`, implement `run_extension_command` |

---

## Architecture: data flow

### Registration (extension loads)

```
WASM extension calls register_command("open-panel", "open panel")
  → since_v0_9_0.rs: WasmState::register_command
      sends (extension_id, "gui_test: open panel", "open-panel") to channel
  → ExtensionStore channel task
      cx.emit(Event::ExtensionCommandRegistered { ... })
  → extension_panel::init subscribe_in callback
      GlobalDynamicCommandRegistry.register(DynamicCommand { ... })
```

### Invocation (user selects command)

```
ctrl+shift+P → "gui_test: open panel"
  ← command_palette reads GlobalDynamicCommandRegistry
  → dispatch OpenExtensionPanel { extension_id: "gui-test", command_id: "open-panel" }
  → extension_panel::init register_action handler:
      get wasm_extension from ExtensionStore::wasm_extension_for_id
      panel.open_or_focus(manifest, wasm_extension)
        → if tab exists: focus it
        → if not: add_view + emit PanelEvent::Activate
      wasm_extension.call_run_extension_command("open-panel")
        → extension handles it (no-op for "open-panel")
```

### Unregistration (extension unloads)

```
Extension unloads → Event::ExtensionUninstalled("gui-test")
  → extension_panel::init subscribe_in callback
      GlobalDynamicCommandRegistry.unregister_extension("gui-test")
  → commands disappear from palette immediately
```
