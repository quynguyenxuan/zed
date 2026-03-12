# Extension Commands & Panel Footer Button Design

**Date**: 2026-03-12
**Branch**: feature/gui-extension

## Goals

1. **Footer button** — Extension panel toggle button always visible in status bar (next to notification panel)
2. **Command palette** — Extension commands appear in `ctrl+shift+P` as `gui_test: open panel`
3. **Open gui-test** — Selecting the command opens (or focuses) the gui-test extension in a new tab in `ExtensionGuiPanel`

## Architecture Overview

### Crate dependency graph (additions only)

```
command_palette_hooks  ← add GlobalDynamicCommandRegistry
command_palette        ← read GlobalDynamicCommandRegistry
extension             ← add ExtensionCommandEntry to ExtensionManifest
extension_host        ← emit GuiExtensionLoaded (already done)
extension_panel       ← add OpenExtensionPanel action, open_or_focus logic
zed                   ← integration: register commands on GuiExtensionLoaded,
                         add ExtensionGuiPanel::load() to startup join!
```

`extension_host` does NOT depend on `extension_panel`. `zed.rs` is the integration point.

---

## Part 1 — Footer button always visible

**Problem**: `ExtensionGuiPanel` is added dynamically only when a GUI extension loads.
The status bar button (via `PanelButtons`) only appears after that.

**Solution**: Register `ExtensionGuiPanel` at workspace startup like `NotificationPanel`.
`ExtensionGuiPanel::load()` and `ExtensionGuiPanel::empty()` already exist (added on branch).

### Change: `crates/zed/src/zed.rs`

Add to the `futures::join!` block alongside other panels:

```rust
let extension_gui_panel = ExtensionGuiPanel::load(workspace_handle.clone(), cx.clone());

futures::join!(
    add_panel_when_ready(project_panel, ...),
    // ... existing panels ...
    add_panel_when_ready(extension_gui_panel, workspace_handle.clone(), cx.clone()),
);
```

Update `GuiExtensionLoaded` handler: since panel already exists, always use `add_view`:

```rust
if let extension_host::Event::GuiExtensionLoaded(manifest, wasm_extension) = event {
    if let Some(panel) = workspace.panel::<ExtensionGuiPanel>(cx) {
        panel.update_in(cx, |panel, window, cx| {
            panel.add_view(manifest.clone(), wasm_extension.clone(), window, cx);
        }).ok();
    }
}
```

---

## Part 2 — Dynamic command registry

### `crates/command_palette_hooks/src/command_palette_hooks.rs`

Add a generic registry for dynamically-registered commands.
Does not know about extensions — just stores name + action.

```rust
pub struct DynamicCommand {
    pub name: String,            // display: "gui_test: open panel"
    pub extension_id: Arc<str>,  // for lifecycle (unregister on unload)
    pub action: Box<dyn Action>,
}

#[derive(Default)]
pub struct DynamicCommandRegistry {
    commands: Vec<DynamicCommand>,
}

impl DynamicCommandRegistry {
    pub fn register(&mut self, command: DynamicCommand) { ... }

    pub fn unregister_extension(&mut self, extension_id: &str) {
        self.commands.retain(|c| c.extension_id.as_ref() != extension_id);
    }

    pub fn commands(&self) -> impl Iterator<Item = &DynamicCommand> {
        self.commands.iter()
    }
}

pub struct GlobalDynamicCommandRegistry(DynamicCommandRegistry);
impl Global for GlobalDynamicCommandRegistry {}
```

Init in `command_palette_hooks::init`:
```rust
cx.set_global(GlobalDynamicCommandRegistry(DynamicCommandRegistry::default()));
```

### `crates/command_palette/src/command_palette.rs`

In `CommandPalette::new`, after building commands from `window.available_actions()`,
append from the dynamic registry:

```rust
if let Some(registry) = cx.try_global::<GlobalDynamicCommandRegistry>() {
    let dynamic = registry.0.commands().map(|cmd| Command {
        name: cmd.name.clone(),
        action: cmd.action.boxed_clone(),
    });
    commands.extend(dynamic);
}
```

---

## Part 3 — Extension manifest declares commands

### `crates/extension/src/extension_manifest.rs`

```rust
#[derive(Clone, PartialEq, Eq, Debug, Deserialize, Serialize)]
pub struct ExtensionCommandEntry {
    pub label: String,
}

// In ExtensionManifest:
#[serde(default, skip_serializing_if = "BTreeMap::is_empty")]
pub extension_commands: BTreeMap<Arc<str>, ExtensionCommandEntry>,
```

### `extensions/gui-test/extension.toml`

```toml
[extension_commands.open-panel]
label = "open panel"
```

---

## Part 4 — `OpenExtensionPanel` action

### `crates/extension_panel/src/extension_panel.rs`

```rust
#[derive(Clone, PartialEq, Eq, Action)]
pub struct OpenExtensionPanel {
    pub extension_id: Arc<str>,
    pub command_id: Arc<str>,
}
```

Add `open_or_focus` to `ExtensionGuiPanel`:

```rust
pub fn open_or_focus(
    &mut self,
    manifest: Arc<ExtensionManifest>,
    wasm_extension: WasmExtension,
    window: &mut Window,
    cx: &mut Context<Self>,
) {
    // Check if tab for this extension already exists
    let existing = self.active_pane.read(cx).items().find_map(|item| {
        item.downcast::<ExtensionGuiView>()
            .filter(|v| v.read(cx).extension_id == manifest.id)
    });

    if let Some(view) = existing {
        // Focus the existing tab
        self.active_pane.update(cx, |pane, cx| {
            if let Some(ix) = pane.index_for_item(&view) {
                pane.activate_item(ix, true, true, window, cx);
            }
        });
    } else {
        // Open new tab
        self.add_view(manifest, wasm_extension, window, cx);
    }

    // Ensure panel is visible
    cx.emit(PanelEvent::Activate);
}
```

---

## Part 5 — Integration in `zed.rs`

### Register commands on `GuiExtensionLoaded`

```rust
// After adding the view, also register commands in palette
for (command_id, entry) in &manifest.extension_commands {
    let extension_name = manifest.name.to_lowercase().replace(' ', "_");
    cx.update_global(|registry: &mut GlobalDynamicCommandRegistry, _| {
        registry.0.register(DynamicCommand {
            name: format!("{}: {}", extension_name, entry.label),
            extension_id: manifest.id.clone(),
            action: Box::new(OpenExtensionPanel {
                extension_id: manifest.id.clone(),
                command_id: command_id.clone(),
            }),
        });
    });
}
```

### Register workspace handler for `OpenExtensionPanel`

```rust
workspace.register_action(
    |workspace: &mut Workspace,
     action: &OpenExtensionPanel,
     window: &mut Window,
     cx: &mut Context<Workspace>| {
        if let Some(panel) = workspace.panel::<ExtensionGuiPanel>(cx) {
            // Need wasm_extension to open_or_focus — get from ExtensionStore
            let extension_store = ExtensionStore::global(cx);
            let wasm = extension_store.read(cx)
                .wasm_extension_for_id(&action.extension_id);
            if let Some((manifest, wasm_extension)) = wasm {
                panel.update_in(cx, |panel, window, cx| {
                    panel.open_or_focus(manifest, wasm_extension, window, cx);
                }).ok();
            } else {
                // Extension not yet loaded — open panel, it will load
                workspace.toggle_panel_focus::<ExtensionGuiPanel>(window, cx);
            }
        }
    }
);
```

### Unregister on extension unload

When an extension with GUI unloads:
```rust
cx.update_global(|registry: &mut GlobalDynamicCommandRegistry, _| {
    registry.0.unregister_extension(&extension_id);
});
```

---

## Part 6 — Future: WIT callback for non-trivial commands

For commands like `reload`, `focus to tree` that need the extension to act,
add to `panel-ui.wit`:

```wit
/// Called when an extension command registered in the manifest is invoked.
export run-extension-command: func(command-id: string) -> result<_, string>;
```

This is NOT in scope for this design — only `open-panel` (host-handled) is implemented now.

---

## Files changed summary

| File | Change |
|------|--------|
| `crates/command_palette_hooks/src/command_palette_hooks.rs` | Add `DynamicCommand`, `DynamicCommandRegistry`, `GlobalDynamicCommandRegistry` |
| `crates/command_palette/src/command_palette.rs` | Read `GlobalDynamicCommandRegistry` when building command list |
| `crates/extension/src/extension_manifest.rs` | Add `ExtensionCommandEntry`, `extension_commands` field |
| `crates/extension_panel/src/extension_panel.rs` | Add `OpenExtensionPanel` action, `open_or_focus` method |
| `crates/extension_host/src/extension_host.rs` | Add `wasm_extension_for_id` lookup method |
| `crates/zed/src/zed.rs` | Register `ExtensionGuiPanel::load()` at startup; register commands on load; register `OpenExtensionPanel` handler; unregister on unload |
| `extensions/gui-test/extension.toml` | Add `[extension_commands.open-panel]` |

---

## Data flow

### Registration (on extension load)

```
ExtensionStore fires GuiExtensionLoaded(manifest, wasm_extension)
  → zed.rs handler:
      1. panel.add_view(manifest, wasm_extension)          ← show in panel
      2. for cmd in manifest.extension_commands:
           registry.register(DynamicCommand { ... })       ← show in palette
```

### Invocation (user selects command)

```
ctrl+shift+P → "gui_test: open panel"
  ← command_palette reads GlobalDynamicCommandRegistry
  → dispatch OpenExtensionPanel { extension_id: "gui-test", command_id: "open-panel" }
  → workspace handler:
      get wasm_extension from ExtensionStore
      panel.open_or_focus(manifest, wasm_extension)
        → if tab exists: focus it
        → if not: add_view + open panel
```

### Unregistration (on extension unload)

```
Extension unloads
  → zed.rs: registry.unregister_extension("gui-test")
  → commands disappear from palette immediately
```
