# Extension Commands & Panel Footer Button Implementation Plan

> **For Claude:** REQUIRED SUB-SKILL: Use superpowers:executing-plans to implement this plan task-by-task.

**Goal:** Register `ExtensionGuiPanel` at startup (always-visible footer button), add `GlobalDynamicCommandRegistry` so extension commands appear in the command palette as `gui_test: open panel`, and open/focus the extension tab when invoked.

**Architecture:** `ExtensionManifest` declares `extension_commands`; `zed.rs` reads them on `GuiExtensionLoaded` and writes `DynamicCommand` entries into `GlobalDynamicCommandRegistry` (stored in App); `command_palette.rs` reads the registry when building the command list. `extension_host` does NOT depend on `extension_panel` — `zed.rs` is the integration point.

**Tech Stack:** Rust, GPUI, wasmtime, `BTreeMap`/`Arc<str>` for manifest keys, `Box<dyn Action>` for palette entries.

---

## Status check before starting

Task 1 (footer button) is **already done** by prior branch work:
- `ExtensionGuiPanel::load()` / `empty()` exist in `extension_panel.rs`
- `zed.rs` line 667-691: `extension_panel` is loaded and added in `futures::join!`

Verify this is working before proceeding:
```bash
cargo check -p zed 2>&1 | grep -E "error|warning.*unused"
```

---

## Task 1: Simplify `GuiExtensionLoaded` handler in `zed.rs`

The handler currently has a fallback that creates a new panel if none exists. Since the panel is always registered at startup now, the `else` branch is dead code.

**Files:**
- Modify: `crates/zed/src/zed.rs:428-453`

**Step 1: Replace the handler**

Find the block starting at line ~428:
```rust
let panel = if let Some(panel) = workspace.panel::<ExtensionGuiPanel>(cx) {
    panel
} else {
    let project = workspace.project().clone();
    ...
    workspace.add_panel(panel.clone(), window, cx);
    panel
};
let manifest = manifest.clone();
let wasm_extension = wasm_extension.clone();
cx.spawn_in(window, async move |_, cx| {
    panel.update_in(cx, |panel, window, cx| {
        panel.add_view(manifest, wasm_extension, window, cx);
    }).ok();
}).detach();
```

Replace with:
```rust
if let Some(panel) = workspace.panel::<ExtensionGuiPanel>(cx) {
    let manifest = manifest.clone();
    let wasm_extension = wasm_extension.clone();
    cx.spawn_in(window, async move |_, cx| {
        panel.update_in(cx, |panel, window, cx| {
            panel.add_view(manifest, wasm_extension, window, cx);
        }).ok();
    }).detach();
}
```

**Step 2: Check**
```bash
cargo check -p zed 2>&1 | grep "^error"
```
Expected: no errors.

**Step 3: Commit**
```bash
git add crates/zed/src/zed.rs
git commit -m "extension_panel: simplify GuiExtensionLoaded handler"
```

---

## Task 2: Add `DynamicCommandRegistry` to `command_palette_hooks`

**Files:**
- Modify: `crates/command_palette_hooks/src/command_palette_hooks.rs`

**Step 1: Add imports at top of file**

Add `use std::sync::Arc;` to the existing `use std::` block.

**Step 2: Add types after the existing `GlobalCommandPaletteInterceptor` impl block (end of file)**

```rust
/// A command dynamically registered at runtime (e.g. by an extension).
pub struct DynamicCommand {
    /// Display name shown in the command palette, e.g. "gui_test: open panel".
    pub name: String,
    /// The extension that registered this command. Used for bulk-unregistration on unload.
    pub extension_id: Arc<str>,
    /// The action dispatched when the command is selected.
    pub action: Box<dyn Action>,
}

/// Registry of runtime-registered commands that appear in the command palette.
#[derive(Default)]
pub struct DynamicCommandRegistry {
    commands: Vec<DynamicCommand>,
}

impl DynamicCommandRegistry {
    /// Register a command. Duplicate `(extension_id, name)` pairs are allowed
    /// (the last one wins in display order).
    pub fn register(&mut self, command: DynamicCommand) {
        self.commands.push(command);
    }

    /// Remove all commands registered by the given extension.
    pub fn unregister_extension(&mut self, extension_id: &str) {
        self.commands
            .retain(|c| c.extension_id.as_ref() != extension_id);
    }

    /// Iterate all registered commands.
    pub fn commands(&self) -> impl Iterator<Item = &DynamicCommand> {
        self.commands.iter()
    }
}

/// App-global wrapper for [`DynamicCommandRegistry`].
#[derive(Default)]
pub struct GlobalDynamicCommandRegistry(pub DynamicCommandRegistry);

impl Global for GlobalDynamicCommandRegistry {}
```

**Step 3: Initialize the global in `init`**

In `pub fn init(cx: &mut App)` (line ~13), add after `cx.set_global(GlobalCommandPaletteFilter::default());`:
```rust
cx.set_global(GlobalDynamicCommandRegistry::default());
```

**Step 4: Check**
```bash
cargo check -p command_palette_hooks 2>&1 | grep "^error"
```
Expected: no errors.

**Step 5: Commit**
```bash
git add crates/command_palette_hooks/src/command_palette_hooks.rs
git commit -m "command_palette_hooks: add DynamicCommandRegistry"
```

---

## Task 3: Make `command_palette` read `GlobalDynamicCommandRegistry`

**Files:**
- Modify: `crates/command_palette/src/command_palette.rs:11-14` (imports) and `~:113` (commands list)

**Step 1: Add import**

In the existing `use command_palette_hooks::{...}` block, add `GlobalDynamicCommandRegistry`:
```rust
use command_palette_hooks::{
    CommandInterceptItem, CommandInterceptResult, CommandPaletteFilter,
    GlobalCommandPaletteInterceptor, GlobalDynamicCommandRegistry,
};
```

**Step 2: Append dynamic commands after the `let commands` block**

After the existing `let commands: Vec<_> = window.available_actions(cx)...collect();`, add:

```rust
let commands = {
    let mut commands = commands;
    if let Some(registry) = cx.try_global::<GlobalDynamicCommandRegistry>() {
        commands.extend(registry.0.commands().map(|cmd| Command {
            name: cmd.name.clone(),
            action: cmd.action.boxed_clone(),
        }));
    }
    commands
};
```

**Step 3: Check**
```bash
cargo check -p command_palette 2>&1 | grep "^error"
```
Expected: no errors.

**Step 4: Commit**
```bash
git add crates/command_palette/src/command_palette.rs
git commit -m "command_palette: include GlobalDynamicCommandRegistry entries"
```

---

## Task 4: Add `extension_commands` to `ExtensionManifest`

**Files:**
- Modify: `crates/extension/src/extension_manifest.rs:111` (after `slash_commands` field) and `~:352` (after `SlashCommandManifestEntry`)

**Step 1: Add entry struct after `SlashCommandManifestEntry` (line ~358)**

```rust
/// Manifest entry for a command registered in the command palette.
#[derive(Clone, PartialEq, Eq, Debug, Deserialize, Serialize)]
pub struct ExtensionCommandEntry {
    /// Label shown in the command palette, e.g. "open panel".
    pub label: String,
}
```

**Step 2: Add field to `ExtensionManifest` after `slash_commands` (line ~113)**

```rust
#[serde(default, skip_serializing_if = "BTreeMap::is_empty")]
pub extension_commands: BTreeMap<Arc<str>, ExtensionCommandEntry>,
```

**Step 3: Add `extension_commands: BTreeMap::default()` to every place that constructs `ExtensionManifest`**

Search for all construction sites:
```bash
grep -n "slash_commands: BTreeMap::default" crates/extension/src/extension_manifest.rs
grep -rn "slash_commands: BTreeMap::default" crates/
```

Add `extension_commands: BTreeMap::default(),` next to each `slash_commands: BTreeMap::default(),` found.

**Step 4: Export `ExtensionCommandEntry` from the crate**

Check `crates/extension/src/extension.rs` for the public re-export list and add:
```rust
pub use extension_manifest::ExtensionCommandEntry;
```

**Step 5: Check**
```bash
cargo check -p extension 2>&1 | grep "^error"
cargo check -p extension_host 2>&1 | grep "^error"
```
Expected: no errors.

**Step 6: Commit**
```bash
git add crates/extension/src/extension_manifest.rs crates/extension/src/extension.rs
git commit -m "extension: add extension_commands to ExtensionManifest"
```

---

## Task 5: Add `OpenExtensionPanel` action and `open_or_focus` to `extension_panel`

**Files:**
- Modify: `crates/extension_panel/src/extension_panel.rs`

**Step 1: Add `Action` derive macro import**

The `Action` derive is already in scope via `gpui::{..., actions, ...}`. Add `serde::Serialize` if not present.

Check current imports — `Serialize` from serde isn't imported. Add `use serde::Serialize;` (alongside existing `use serde::Deserialize;`).

**Step 2: Add `OpenExtensionPanel` action after the `actions!` block (line ~29)**

```rust
/// Opens a specific extension in the extension GUI panel, focusing an existing
/// tab if one is already open, or creating a new tab otherwise.
#[derive(Clone, PartialEq, Eq, gpui::Action, Serialize, Deserialize)]
pub struct OpenExtensionPanel {
    pub extension_id: Arc<str>,
    pub command_id: Arc<str>,
}
```

**Step 3: Add `open_or_focus` method to `ExtensionGuiPanel` impl block**

Add after `add_view`:

```rust
/// Opens the given extension in a new tab, or focuses the existing tab if
/// it is already open. Emits `PanelEvent::Activate` to ensure the panel is visible.
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

Note: `ExtensionGuiView` needs `pub extension_id` — check if it's currently `pub` or `pub(crate)`.

**Step 4: Make `ExtensionGuiView::extension_id` accessible**

In `pub struct ExtensionGuiView`, change:
```rust
extension_id: Arc<str>,
```
to:
```rust
pub(crate) extension_id: Arc<str>,
```

**Step 5: Check**
```bash
cargo check -p extension_panel 2>&1 | grep "^error"
```
Expected: no errors.

**Step 6: Commit**
```bash
git add crates/extension_panel/src/extension_panel.rs
git commit -m "extension_panel: add OpenExtensionPanel action and open_or_focus"
```

---

## Task 6: Add `wasm_extension_for_id` to `ExtensionStore`

**Files:**
- Modify: `crates/extension_host/src/extension_host.rs` near line 441 (after `extension_manifest_for_id`)

**Step 1: Add method**

After `fn extension_manifest_for_id`:
```rust
/// Returns the loaded WASM extension and its manifest for the given extension ID,
/// if the extension is currently loaded as a GUI extension.
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
Expected: no errors.

**Step 3: Commit**
```bash
git add crates/extension_host/src/extension_host.rs
git commit -m "extension_host: add wasm_extension_for_id lookup"
```

---

## Task 7: Wire everything together in `zed.rs`

**Files:**
- Modify: `crates/zed/src/zed.rs`

**Step 1: Add imports at top of file**

Find the `use extension_panel::ExtensionGuiPanel;` import. Add alongside it:
```rust
use extension_panel::OpenExtensionPanel;
```

Find `use command_palette_hooks` or add near other palette imports:
```rust
use command_palette_hooks::{DynamicCommand, GlobalDynamicCommandRegistry};
```

**Step 2: Register `OpenExtensionPanel` workspace action handler**

In the `cx.observe_new` block where other workspace actions are registered (around line 1060-1090 where `extension_panel::ToggleFocus` is registered), add:

```rust
.register_action(
    |workspace: &mut Workspace,
     action: &OpenExtensionPanel,
     window: &mut Window,
     cx: &mut Context<Workspace>| {
        let extension_id = action.extension_id.clone();
        if let Some(panel) = workspace.panel::<ExtensionGuiPanel>(cx) {
            let extension_store = ExtensionStore::global(cx);
            if let Some((manifest, wasm_extension)) = extension_store
                .read(cx)
                .wasm_extension_for_id(&extension_id)
            {
                cx.spawn_in(window, async move |_, cx| {
                    panel
                        .update_in(cx, |panel, window, cx| {
                            panel.open_or_focus(manifest, wasm_extension, window, cx);
                        })
                        .ok();
                })
                .detach();
            } else {
                workspace.toggle_panel_focus::<ExtensionGuiPanel>(window, cx);
            }
        }
    },
)
```

**Step 3: Register commands from manifest in `GuiExtensionLoaded` handler**

In the simplified handler from Task 1, after `panel.add_view(...)`, add command registration:

```rust
if let extension_host::Event::GuiExtensionLoaded(manifest, wasm_extension) = event {
    if let Some(panel) = workspace.panel::<ExtensionGuiPanel>(cx) {
        let manifest = manifest.clone();
        let wasm_extension = wasm_extension.clone();
        // Register extension commands in the command palette
        let extension_name = manifest.name.to_lowercase().replace(' ', "_");
        for (command_id, entry) in &manifest.extension_commands {
            let action = OpenExtensionPanel {
                extension_id: manifest.id.clone(),
                command_id: command_id.clone(),
            };
            cx.update_global(|registry: &mut GlobalDynamicCommandRegistry, _| {
                registry.0.register(DynamicCommand {
                    name: format!("{}: {}", extension_name, entry.label),
                    extension_id: manifest.id.clone(),
                    action: Box::new(action),
                });
            });
        }
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

**Step 4: Unregister commands on extension uninstall**

In the same `cx.subscribe_in` block on `extension_store`, also handle `ExtensionUninstalled`:

```rust
if let extension_host::Event::ExtensionUninstalled(extension_id) = event {
    cx.update_global(|registry: &mut GlobalDynamicCommandRegistry, _| {
        registry.0.unregister_extension(extension_id);
    });
}
```

**Step 5: Check**
```bash
cargo check -p zed 2>&1 | grep "^error"
```
Expected: no errors.

**Step 6: Check the whole workspace**
```bash
cargo check -p extension_panel -p extension_host -p command_palette -p command_palette_hooks -p zed 2>&1 | grep "^error"
```
Expected: no errors.

**Step 7: Commit**
```bash
git add crates/zed/src/zed.rs
git commit -m "zed: register extension commands in palette and handle OpenExtensionPanel"
```

---

## Task 8: Add `extension_commands` to `gui-test/extension.toml`

**Files:**
- Modify: `extensions/gui-test/extension.toml`

**Step 1: Add command declaration**

```toml
[extension_commands.open-panel]
label = "open panel"
```

**Step 2: Verify the toml parses correctly**
```bash
cargo check -p zed 2>&1 | grep "^error"
```

**Step 3: Commit**
```bash
git add extensions/gui-test/extension.toml
git commit -m "gui-test: declare open-panel extension command"
```

---

## Task 9: Full build and clippy check

```bash
./script/clippy 2>&1 | grep "^error" | head -20
```

Fix any errors found. If clippy passes clean, the feature is complete.

---

## Verification checklist

- [ ] Status bar footer shows Extension Panel icon (Blocks) at all times, even before any extension loads
- [ ] `ctrl+shift+P` → type "gui_test" → shows "gui_test: open panel"
- [ ] Selecting the command opens the extension panel on the right and adds a "gui-test" tab
- [ ] Running the command again (panel already open, tab already there) → focuses the existing tab, does not duplicate
- [ ] Reloading the extension → command still works

---

## Key files reference

| File | Purpose |
|------|---------|
| `crates/command_palette_hooks/src/command_palette_hooks.rs` | `DynamicCommandRegistry`, `GlobalDynamicCommandRegistry` |
| `crates/command_palette/src/command_palette.rs:100-120` | reads registry when building commands |
| `crates/extension/src/extension_manifest.rs:111,352` | `extension_commands` field + `ExtensionCommandEntry` |
| `crates/extension_panel/src/extension_panel.rs` | `OpenExtensionPanel`, `open_or_focus` |
| `crates/extension_host/src/extension_host.rs:441` | `wasm_extension_for_id` |
| `crates/zed/src/zed.rs:420-460,1060-1090` | integration wiring |
| `extensions/gui-test/extension.toml` | declares commands |
