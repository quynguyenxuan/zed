# Extension Commands & Panel Footer Button Implementation Plan

> **For Claude:** REQUIRED SUB-SKILL: Use superpowers:executing-plans to implement this plan task-by-task.

**Goal:** Register `ExtensionGuiPanel` at startup (always-visible footer button); add `register-command` / `run-extension-command` to `extension.wit` so any extension can register commands in `ctrl+shift+P` and handle invocations; wire `gui-test` to open its panel tab.

**Architecture:** `register-command` is an import in `extension.wit` (all extension types). When called, `WasmState::on_main_thread` writes to `GlobalDynamicCommandRegistry` (App global in `command_palette_hooks`). `command_palette.rs` reads the registry. On invocation, `zed.rs` dispatches `OpenExtensionPanel` action then calls `WasmExtension::call_run_extension_command`. `extension_host` does NOT depend on `extension_panel` — `zed.rs` is the integration point.

**Tech Stack:** Rust, GPUI, wasmtime WIT component model, `mpsc::UnboundedSender<MainThreadCall>` for main-thread dispatch.

---

## Status check before starting

Task 1 (footer button) is **already done** — verify:
```bash
grep "extension_panel\|ExtensionGuiPanel::load" crates/zed/src/zed.rs | grep -v "^Binary"
```
Expected: `ExtensionGuiPanel::load(...)` present in the `futures::join!` block.

---

## Task 1: Simplify `GuiExtensionLoaded` handler in `zed.rs`

The panel is always registered at startup now — the fallback `else` branch that creates a new panel is dead code.

**Files:**
- Modify: `crates/zed/src/zed.rs:428-453`

**Step 1: Replace handler**

Find the block:
```rust
let panel = if let Some(panel) = workspace.panel::<ExtensionGuiPanel>(cx) {
    panel
} else {
    ...
    workspace.add_panel(panel.clone(), window, cx);
    panel
};
```

Replace the entire `GuiExtensionLoaded` branch with:
```rust
if let extension_host::Event::GuiExtensionLoaded(manifest, wasm_extension) = event {
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
```

**Step 2: Check**
```bash
cargo check -p zed 2>&1 | grep "^error"
```

**Step 3: Commit**
```bash
git add crates/zed/src/zed.rs
git commit -m "extension_panel: simplify GuiExtensionLoaded handler"
```

---

## Task 2: Add `register-command` to `extension.wit`

**Files:**
- Modify: `crates/extension_api/wit/since_v0.9.0/extension.wit`

**Step 1: Add import after `import get-settings`**

```wit
/// Registers a command that appears in Zed's command palette.
/// When the user invokes the command, Zed calls `run-extension-command` with the given id.
import register-command: func(id: string, label: string);
```

**Step 2: Add export near other exports (after `export run-slash-command`)**

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

## Task 3: Implement `register-command` host function in `wasm_host/wit.rs`

**Files:**
- Modify: `crates/extension_host/src/wasm_host/wit.rs`

**Step 1: Find where GUI host functions are implemented**

Search for the existing `set_view` implementation:
```bash
grep -n "fn set_view\|fn register_command\|fn emit\b" crates/extension_host/src/wasm_host/wit.rs | head -10
```

**Step 2: Add `register_command` implementation**

In the same impl block where `set_view`, `emit`, `call` are implemented (V0_9_0 host functions), add:

```rust
fn register_command(
    &mut self,
    _store: wasmtime::StoreContextMut<WasmState>,
    id: String,
    label: String,
) -> wasmtime::Result<()> {
    let extension_id = _store.data().manifest.id.clone();
    let extension_name = _store.data().manifest.name.clone();
    _store
        .data()
        .on_main_thread(move |cx| {
            async move {
                cx.update_global(|registry: &mut GlobalDynamicCommandRegistry, _| {
                    registry.0.register(DynamicCommand {
                        name: format!(
                            "{}: {}",
                            extension_name.to_lowercase().replace(' ', "_"),
                            label
                        ),
                        extension_id: extension_id.clone(),
                        action: Box::new(OpenExtensionPanel {
                            extension_id,
                            command_id: id.into(),
                        }),
                    });
                });
            }
            .boxed_local()
        })
        .await;
    Ok(())
}
```

Note: this requires importing `GlobalDynamicCommandRegistry`, `DynamicCommand` from `command_palette_hooks`, and `OpenExtensionPanel` from `extension_panel`. Check that these are accessible (no circular dependency: `extension_host` does not depend on `extension_panel` today).

**If circular dependency**: move `OpenExtensionPanel` to a shared crate, OR store only `(extension_id, command_id)` as strings and let `zed.rs` create the action later (see Task 7 alternative).

**Preferred approach (avoids circular dep)**: Store only strings in the registry, wrap creation in `zed.rs`.

Use this simpler version that stores a plain `InvokeExtensionCommand` action (defined in `command_palette_hooks` with just string fields):

```rust
// In on_main_thread closure:
cx.update_global(|registry: &mut GlobalDynamicCommandRegistry, _| {
    registry.0.register(DynamicCommand {
        name: format!("{}: {}", display_name, label),
        extension_id: extension_id.clone(),
        command_id: id.clone().into(),
    });
});
```

Where `DynamicCommand` stores `command_id: Arc<str>` directly (see Task 4 for updated struct).

**Step 3: Add `run_extension_command` stub to Extension trait**

In `wasm_host/wit.rs`, find where `call_gui_init` etc. are wrapped. Add:

```rust
pub async fn call_run_extension_command(&self, command_id: String) -> Result<()> {
    self.call(move |ext, store| {
        async move { ext.call_run_extension_command(store, &command_id).await }.boxed()
    })
    .await?
}
```

**Step 4: Check**
```bash
cargo check -p extension_host 2>&1 | grep "^error"
```

**Step 5: Commit**
```bash
git add crates/extension_host/src/wasm_host/wit.rs crates/extension_host/src/wasm_host.rs
git commit -m "extension_host: implement register-command and run-extension-command"
```

---

## Task 4: Update `DynamicCommandRegistry` (no `Box<dyn Action>`, store strings)

This avoids circular dependency between `extension_host` and `extension_panel`.

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

**Step 2: Initialize in `init`**

In `pub fn init(cx: &mut App)`, add:
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

## Task 5: Add `OpenExtensionPanel` action and `open_or_focus` to `extension_panel`

**Files:**
- Modify: `crates/extension_panel/src/extension_panel.rs`

**Step 1: Add import**

Add `use serde::Serialize;` alongside existing `use serde::Deserialize;`.

**Step 2: Add action after `actions!` block (~line 29)**

```rust
/// Opens a specific extension in the extension GUI panel by extension and command ID.
#[derive(Clone, PartialEq, Eq, gpui::Action, Serialize, Deserialize)]
pub struct OpenExtensionPanel {
    pub extension_id: Arc<str>,
    pub command_id: Arc<str>,
}
```

**Step 3: Make `extension_id` accessible inside crate**

In `pub struct ExtensionGuiView`, change `extension_id: Arc<str>` to `pub(crate) extension_id: Arc<str>`.

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
            item.to_any_view()
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

## Task 6: Make `command_palette` read `GlobalDynamicCommandRegistry`

**Files:**
- Modify: `crates/command_palette/src/command_palette.rs:11-14`

**Step 1: Add import**

In the `use command_palette_hooks::{...}` block, add `GlobalDynamicCommandRegistry`:
```rust
use command_palette_hooks::{
    CommandInterceptItem, CommandInterceptResult, CommandPaletteFilter,
    GlobalCommandPaletteInterceptor, GlobalDynamicCommandRegistry,
};
```

**Step 2: Append dynamic commands after `let commands`**

After `let commands: Vec<_> = window.available_actions(cx)...collect();`:
```rust
let commands = {
    let mut commands = commands;
    if let Some(registry) = cx.try_global::<GlobalDynamicCommandRegistry>() {
        // Extension commands: create OpenExtensionPanel action per entry.
        // The action is constructed here to avoid circular deps in extension_host.
        commands.extend(registry.0.commands().map(|cmd| {
            use extension_panel::OpenExtensionPanel;
            Command {
                name: cmd.name.clone(),
                action: Box::new(OpenExtensionPanel {
                    extension_id: cmd.extension_id.clone(),
                    command_id: cmd.command_id.clone(),
                }),
            }
        }));
    }
    commands
};
```

Note: `command_palette` must add `extension_panel` as a dependency in `Cargo.toml`.

**Step 3: Add `extension_panel` to `command_palette/Cargo.toml`**

```toml
[dependencies]
...
extension_panel.workspace = true
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

## Task 7: Add `wasm_extension_for_id` to `ExtensionStore`

**Files:**
- Modify: `crates/extension_host/src/extension_host.rs` (after line 441)

**Step 1: Add method**

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

## Task 8: Wire everything in `zed.rs`

**Files:**
- Modify: `crates/zed/src/zed.rs`

**Step 1: Add imports**

```rust
use extension_panel::OpenExtensionPanel;
use command_palette_hooks::GlobalDynamicCommandRegistry;
```

**Step 2: Register `OpenExtensionPanel` workspace handler**

In the `cx.observe_new` block near the `extension_panel::ToggleFocus` handler (~line 1079), add:

```rust
.register_action(
    |workspace: &mut Workspace,
     action: &OpenExtensionPanel,
     window: &mut Window,
     cx: &mut Context<Workspace>| {
        let extension_id = action.extension_id.clone();
        let command_id = action.command_id.clone();
        let extension_store = ExtensionStore::global(cx);
        let wasm = extension_store
            .read(cx)
            .wasm_extension_for_id(&extension_id);

        if let Some(panel) = workspace.panel::<ExtensionGuiPanel>(cx) {
            if let Some((manifest, wasm_extension)) = wasm.clone() {
                // Open/focus the panel tab
                cx.spawn_in(window, async move |_, cx| {
                    panel
                        .update_in(cx, |panel, window, cx| {
                            panel.open_or_focus(manifest, wasm_extension.clone(), window, cx);
                        })
                        .ok();
                    // Call back into extension
                    wasm_extension
                        .call_run_extension_command(command_id.to_string())
                        .await
                        .log_err();
                })
                .detach();
            } else {
                // Extension not yet loaded — just show the panel
                workspace.toggle_panel_focus::<ExtensionGuiPanel>(window, cx);
            }
        }
    },
)
```

**Step 3: Unregister commands on extension uninstall**

In the existing `cx.subscribe_in(&extension_store, ...)` block, handle `ExtensionUninstalled`:

```rust
if let extension_host::Event::ExtensionUninstalled(extension_id) = event {
    cx.update_global(|registry: &mut GlobalDynamicCommandRegistry, _| {
        registry.0.unregister_extension(extension_id);
    });
}
```

**Step 4: Check**
```bash
cargo check -p zed 2>&1 | grep "^error"
```

**Step 5: Commit**
```bash
git add crates/zed/src/zed.rs
git commit -m "zed: handle OpenExtensionPanel, unregister commands on extension unload"
```

---

## Task 9: Implement `run-extension-command` in `gui-test`

**Files:**
- Modify: `extensions/gui-test/src/lib.rs` (or wherever `gui_init` lives)

**Step 1: Check current extension code**
```bash
find extensions/gui-test/src -name "*.rs" | head -5
cat extensions/gui-test/src/lib.rs 2>/dev/null | head -50
```

**Step 2: Call `register_command` in `gui_init`**

In the extension's `gui_init` implementation:
```rust
fn gui_init(&mut self) {
    zed_extension_api::register_command("open-panel", "open panel");
    // ... rest of init
}
```

**Step 3: Handle `run_extension_command`**

```rust
fn run_extension_command(&mut self, command_id: &str) -> Result<(), String> {
    match command_id {
        "open-panel" => {
            // Host already opened the panel. No-op here, or render initial view.
            Ok(())
        }
        _ => Err(format!("unknown command: {command_id}")),
    }
}
```

**Step 4: Check**
```bash
cargo check -p gui-test 2>&1 | grep "^error"
```

**Step 5: Commit**
```bash
git add extensions/gui-test/src/
git commit -m "gui-test: register open-panel command and handle run-extension-command"
```

---

## Task 10: Full build and clippy

```bash
./script/clippy 2>&1 | grep "^error" | head -20
```

Fix any errors. Then run a full check:
```bash
cargo check -p extension_panel -p extension_host -p command_palette -p command_palette_hooks -p zed 2>&1 | grep "^error"
```

---

## Verification checklist

- [ ] Status bar footer always shows Extension Panel icon (Blocks)
- [ ] `ctrl+shift+P` → type "gui_test" → shows `gui_test: open panel`
- [ ] Selecting command → panel opens, tab added for gui-test
- [ ] Running command again → focuses existing tab, no duplicate
- [ ] `run-extension-command` called on extension after panel opens

---

## Key files reference

| File | Change |
|------|--------|
| `crates/extension_api/wit/since_v0.9.0/extension.wit` | `import register-command`, `export run-extension-command` |
| `crates/extension_host/src/wasm_host/wit.rs` | implement `register_command` host fn via `on_main_thread` |
| `crates/extension_host/src/wasm_host.rs` | add `call_run_extension_command` to `WasmExtension` |
| `crates/extension_host/src/extension_host.rs:441` | add `wasm_extension_for_id` |
| `crates/command_palette_hooks/src/command_palette_hooks.rs` | `DynamicCommand`, `DynamicCommandRegistry`, `GlobalDynamicCommandRegistry` |
| `crates/command_palette/src/command_palette.rs` | read registry, create `OpenExtensionPanel` actions |
| `crates/command_palette/Cargo.toml` | add `extension_panel` dep |
| `crates/extension_panel/src/extension_panel.rs` | `OpenExtensionPanel` action, `open_or_focus` |
| `crates/zed/src/zed.rs` | simplify handler, register action, unregister on unload |
| `extensions/gui-test/src/lib.rs` | call `register_command`, implement `run_extension_command` |
