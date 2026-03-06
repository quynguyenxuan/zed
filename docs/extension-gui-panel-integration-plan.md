# Extension GUI Panel - PaneGroup Integration Plan

## Current Status Analysis

### ✅ Already Implemented
- **PaneGroup structure** (line 135): `center: PaneGroup`
- **Single pane creation** (line 149): `new_extension_pane()`
- **Basic drop validation** (line 199-213): Validates ExtensionGuiView drops
- **Rendering** (line 308-320): Uses `center.render()` correctly
- **Item trait** (line 125-131): ExtensionGuiView implements Item

### ❌ Missing Features
- Pane event handling (split, remove, focus)
- Split functionality (4 directions)
- Cross-pane drag and drop
- Tab bar customization buttons
- Pane lifecycle management
- State serialization/persistence

---

## Implementation Plan

### Phase 1: Core Event Handling (Priority: HIGH)

#### 1.1 Subscribe to Pane Events
**File**: `extension_panel.rs`
**Location**: In `new_extension_pane()` function, after line 215

```rust
// Subscribe to pane events
let panel = panel_weak_entity.clone();
cx.subscribe_in(&pane, window, move |panel, pane, event: &pane::Event, window, cx| {
    panel.handle_pane_event(pane, event, window, cx);
})
.detach();
```

#### 1.2 Implement `handle_pane_event` Method
**File**: `extension_panel.rs`
**Location**: Add new method to `impl ExtensionGuiPanel`

```rust
fn handle_pane_event(
    &mut self,
    pane: &Entity<Pane>,
    event: &pane::Event,
    window: &mut Window,
    cx: &mut Context<Self>,
) {
    match event {
        pane::Event::ActivateItem { .. } => {
            // Update active_pane if needed
            if pane != &self.active_pane {
                self.active_pane = pane.clone();
            }
            cx.notify();
        }
        pane::Event::Remove { focus_on_pane } => {
            let pane_count = self.center.panes().len();
            let _removal_result = self.center.remove(pane, cx);

            // If last pane removed, close panel
            if pane_count == 1 {
                self.center.first_pane().update(cx, |pane, cx| {
                    pane.set_zoomed(false, cx);
                });
                cx.emit(PanelEvent::Close);
            } else if let Some(focus_pane) = focus_on_pane.as_ref()
                .or_else(|| self.center.panes().last())
            {
                focus_pane.focus_handle(cx).focus(window, cx);
            }
        }
        pane::Event::ZoomIn => {
            for pane in self.center.panes() {
                pane.update(cx, |pane, cx| pane.set_zoomed(true, cx));
            }
            cx.emit(PanelEvent::ZoomIn);
            cx.notify();
        }
        pane::Event::ZoomOut => {
            for pane in self.center.panes() {
                pane.update(cx, |pane, cx| pane.set_zoomed(false, cx));
            }
            cx.emit(PanelEvent::ZoomOut);
            cx.notify();
        }
        pane::Event::Focus => {
            self.active_pane = pane.clone();
            cx.notify();
        }
        pane::Event::AddItem { item } => {
            if let Some(workspace) = self.workspace.upgrade() {
                workspace.update(cx, |workspace, cx| {
                    item.added_to_pane(workspace, pane.clone(), window, cx);
                });
            }
        }
        pane::Event::Split { direction, mode } => {
            self.handle_split(pane, *direction, *mode, window, cx);
        }
        _ => {}
    }
}
```

**Estimated Time**: 1-2 hours
**Complexity**: Medium
**Dependencies**: None

---

### Phase 2: Split Functionality (Priority: HIGH)

#### 2.1 Implement Split Handler
**File**: `extension_panel.rs`
**Location**: Add new method to `impl ExtensionGuiPanel`

```rust
fn handle_split(
    &mut self,
    pane: &Entity<Pane>,
    direction: SplitDirection,
    mode: SplitMode,
    window: &mut Window,
    cx: &mut Context<Self>,
) {
    match mode {
        SplitMode::EmptyPane => {
            // Create new empty pane
            let new_pane = self.create_new_pane(false, window, cx);
            self.center.split(pane, &new_pane, direction, cx);
            window.focus(&new_pane.focus_handle(cx), cx);
        }
        SplitMode::ClonePane => {
            // Clone active item to new pane
            if let Some(active_item) = pane.read(cx).active_item() {
                if let Some(gui_view) = active_item.downcast::<ExtensionGuiView>() {
                    // Note: For true cloning, we'd need to clone the extension state
                    // For now, just create a new empty pane
                    let new_pane = self.create_new_pane(false, window, cx);
                    self.center.split(pane, &new_pane, direction, cx);
                    window.focus(&new_pane.focus_handle(cx), cx);
                }
            }
        }
        SplitMode::MovePane => {
            // Move active item to new pane
            if let Some(item) = pane.update(cx, |pane, cx| {
                pane.take_active_item(window, cx)
            }) {
                let new_pane = self.create_new_pane(false, window, cx);
                new_pane.update(cx, |new_pane, cx| {
                    new_pane.add_item(item, true, true, None, window, cx);
                });
                self.center.split(pane, &new_pane, direction, cx);
                window.focus(&new_pane.focus_handle(cx), cx);
            }
        }
    }
}

fn create_new_pane(
    &mut self,
    zoomed: bool,
    window: &mut Window,
    cx: &mut Context<Self>,
) -> Entity<Pane> {
    let project = self.workspace
        .upgrade()
        .map(|ws| ws.read(cx).project().clone());

    if let Some(project) = project {
        let pane = new_extension_pane(
            self.workspace.clone(),
            project,
            window,
            cx,
        );
        pane.update(cx, |pane, cx| {
            pane.set_zoomed(zoomed, cx);
        });
        pane
    } else {
        // Fallback: shouldn't happen
        self.active_pane.clone()
    }
}
```

#### 2.2 Add Split Actions to Imports
**File**: `extension_panel.rs`
**Location**: Update imports (line 16-20)

```rust
use workspace::{
    DraggedTab, Pane, PaneGroup, PaneRenderContext, Workspace,
    SplitDirection, SplitMode, SplitUp, SplitDown, SplitLeft, SplitRight,
    dock::{DockPosition, Panel, PanelEvent},
    item::{Item, ItemEvent},
    pane,
};
```

**Estimated Time**: 2-3 hours
**Complexity**: Medium-High
**Dependencies**: Phase 1

---

### Phase 3: Enhanced Drag & Drop (Priority: MEDIUM)

#### 3.1 Update `set_can_split` Validation
**File**: `extension_panel.rs`
**Location**: In `new_extension_pane()`, add before line 199

```rust
// Store panel weak entity for closures
let panel_weak = /* need to pass this from caller */;

pane.set_can_split(Some(Arc::new(move |pane, dragged_item, _window, cx| {
    if let Some(tab) = dragged_item.downcast_ref::<DraggedTab>() {
        let is_current_pane = tab.pane == cx.entity();

        // Check if we can drag this item away
        let can_drag = panel_weak
            .read_with(cx, |panel, _| {
                let current_panes = panel.center.panes();
                // Allow if: from external pane, OR multiple panes exist,
                // OR not the only item in current pane
                !current_panes.contains(&&tab.pane)
                    || current_panes.len() > 1
                    || (!is_current_pane || pane.items_len() > 1)
            })
            .ok()
            .unwrap_or(false);

        if can_drag {
            let item = if is_current_pane {
                pane.item_for_index(tab.ix)
            } else {
                tab.pane.read(cx).item_for_index(tab.ix)
            };
            if let Some(item) = item {
                return item.downcast::<ExtensionGuiView>().is_some();
            }
        }
    }
    false
})));
```

#### 3.2 Enhanced Drop Handler with Split Support
**File**: `extension_panel.rs`
**Location**: Replace current `set_custom_drop_handle` (line 199-213)

```rust
let panel_weak = /* need to pass this */;
let workspace_weak = workspace.clone();

pane.set_custom_drop_handle(cx, move |pane, dropped_item, window, cx| {
    if let Some(tab) = dropped_item.downcast_ref::<DraggedTab>() {
        let this_pane = cx.entity();
        let item = if tab.pane == this_pane {
            pane.item_for_index(tab.ix)
        } else {
            tab.pane.read(cx).item_for_index(tab.ix)
        };

        if let Some(item) = item {
            if item.downcast::<ExtensionGuiView>().is_some() {
                let source = tab.pane.clone();
                let item_id = item.item_id();

                // Check if we're splitting
                let split_direction = pane.drag_split_direction();

                if let Some(direction) = split_direction {
                    // Defer split to avoid re-entrancy
                    cx.spawn_in(window, async move |_, cx| {
                        cx.update(|window, cx| {
                            let Ok(new_pane) = panel_weak.update(cx, |panel, cx| {
                                let project = workspace_weak
                                    .read_with(cx, |ws, cx| ws.project().clone())
                                    .ok()?;
                                let new_pane = new_extension_pane(
                                    workspace_weak.clone(),
                                    project,
                                    window,
                                    cx,
                                );
                                panel.center.split(&this_pane, &new_pane, direction, cx);
                                Some(new_pane)
                            }).ok().flatten() else {
                                return;
                            };

                            workspace::move_item(
                                &source,
                                &new_pane,
                                item_id,
                                new_pane.read(cx).active_item_index(),
                                true,
                                window,
                                cx,
                            );
                        }).ok();
                    }).detach();
                } else {
                    // Regular drop (move to same pane)
                    return ControlFlow::Continue(());
                }
                return ControlFlow::Break(());
            }
        }
    }
    ControlFlow::Break(())
});
```

**Estimated Time**: 3-4 hours
**Complexity**: High
**Dependencies**: Phase 2

---

### Phase 4: Tab Bar Customization (Priority: LOW)

#### 4.1 Add Tab Bar Buttons
**File**: `extension_panel.rs**
**Location**: In `new_extension_pane()`, after pane creation

```rust
pane.set_render_tab_bar_buttons(cx, move |pane, window, cx| {
    use ui::{IconButton, IconName, IconSize, PopoverMenu, ContextMenu, Corner};

    if !pane.has_focus(window, cx) && !pane.context_menu_focused(window, cx) {
        return (None, None);
    }

    let focus_handle = pane.focus_handle(cx);
    let right_children = h_flex()
        .gap(DynamicSpacing::Base02.rems(cx))
        .child(
            IconButton::new("extension-pane-split", IconName::Split)
                .icon_size(IconSize::Small)
                .tooltip(|_, cx| Tooltip::text("Split Pane", cx))
        )
        .child({
            let zoomed = pane.is_zoomed();
            IconButton::new("toggle_zoom", IconName::Maximize)
                .icon_size(IconSize::Small)
                .toggle_state(zoomed)
                .selected_icon(IconName::Minimize)
                .on_click(cx.listener(|pane, _, window, cx| {
                    pane.toggle_zoom(&workspace::ToggleZoom, window, cx);
                }))
                .tooltip(move |_, cx| {
                    Tooltip::text(
                        if zoomed { "Zoom Out" } else { "Zoom In" },
                        cx,
                    )
                })
        })
        .into_any_element()
        .into();

    (None, right_children)
});
```

**Estimated Time**: 2 hours
**Complexity**: Low-Medium
**Dependencies**: None (can be done independently)

---

### Phase 5: State Persistence (Priority: LOW - Optional)

#### 5.1 Define Serialization Structures
**File**: `extension_panel.rs`
**Location**: Top of file, after imports

```rust
use serde::{Serialize, Deserialize};

#[derive(Serialize, Deserialize)]
struct SerializedExtensionPanel {
    width: Option<Pixels>,
    pane_group: SerializedPaneGroup,
}

#[derive(Serialize, Deserialize)]
struct SerializedPaneGroup {
    // Store pane layout and active items
    active_extension_ids: Vec<String>,
}
```

#### 5.2 Implement Save/Load
**File**: `extension_panel.rs`

```rust
impl ExtensionGuiPanel {
    fn serialize(&self, cx: &App) -> SerializedExtensionPanel {
        let active_extension_ids = self.center
            .panes()
            .iter()
            .flat_map(|pane| pane.read(cx).items())
            .filter_map(|item| {
                item.downcast::<ExtensionGuiView>()
                    .map(|view| view.read(cx).extension_id.to_string())
            })
            .collect();

        SerializedExtensionPanel {
            width: self.width,
            pane_group: SerializedPaneGroup {
                active_extension_ids,
            },
        }
    }
}
```

**Estimated Time**: 3-4 hours
**Complexity**: Medium
**Dependencies**: All previous phases

---

## Required Changes to Function Signatures

### Problem: Circular Dependency in `new_extension_pane`

**Current**: `new_extension_pane` is called inside `ExtensionGuiPanel::new()`
**Issue**: We need `WeakEntity<ExtensionGuiPanel>` for closures, but panel doesn't exist yet

**Solution**: Refactor initialization order

```rust
impl ExtensionGuiPanel {
    pub fn new(
        manifest: Arc<ExtensionManifest>,
        wasm_extension: WasmExtension,
        workspace: WeakEntity<Workspace>,
        project: Entity<Project>,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) -> Self {
        // Create minimal panel first
        let pane = cx.new(|cx| {
            Pane::new(
                workspace.clone(),
                project.clone(),
                Default::default(),
                None,
                ToggleFocus.boxed_clone(),
                false,
                window,
                cx,
            )
        });

        let mut panel = Self {
            center: PaneGroup::new(pane.clone()),
            active_pane: pane.clone(),
            workspace,
            width: None,
        };

        // Now setup pane with panel reference
        panel.setup_pane(&pane, window, cx);

        // Add initial view
        let view = cx.new(|cx| ExtensionGuiView::new(manifest, wasm_extension, cx));
        pane.update(cx, |pane, cx| {
            pane.add_item(Box::new(view), true, true, None, window, cx);
        });

        panel
    }

    fn setup_pane(
        &self,
        pane: &Entity<Pane>,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        let panel_weak = cx.entity().downgrade();

        // All the set_can_split, set_custom_drop_handle, etc. here
        // They can now access panel_weak
    }
}
```

---

## Testing Strategy

### Unit Tests
1. **Pane creation**: Verify pane is created with correct settings
2. **Split operations**: Test all 4 split directions
3. **Drag validation**: Test can_split logic with various scenarios

### Integration Tests
1. **Multi-pane workflow**: Create → Split → Drag → Close
2. **Focus management**: Verify active_pane updates correctly
3. **Zoom behavior**: Test zoom in/out affects all panes

### Manual Testing Checklist
- [ ] Create extension panel with one view
- [ ] Split right/left/up/down
- [ ] Drag tab between panes
- [ ] Drag to split (create new pane)
- [ ] Close panes one by one
- [ ] Zoom in/out
- [ ] Verify tab bar buttons work
- [ ] Test with multiple extension views

---

## Risks & Mitigations

### Risk 1: Circular References
**Mitigation**: Use `WeakEntity` consistently, refactor init order

### Risk 2: Re-entrancy Panics
**Mitigation**: Use `cx.spawn_in()` to defer split operations (see TerminalPanel pattern)

### Risk 3: Extension State Loss on Clone
**Mitigation**: Document that clone creates empty pane (extensions are stateful)

### Risk 4: Memory Leaks
**Mitigation**: Ensure subscriptions are stored and dropped with panel

---

## Timeline Estimate

| Phase | Time | Dependencies |
|-------|------|--------------|
| Phase 1: Event Handling | 1-2h | None |
| Phase 2: Split | 2-3h | Phase 1 |
| Phase 3: Drag & Drop | 3-4h | Phase 2 |
| Phase 4: Tab Bar | 2h | None |
| Phase 5: Persistence | 3-4h | All |
| Testing | 2-3h | All |
| **Total** | **13-18h** | |

---

## References

- **TerminalPanel**: `/home/sunteco/quynguyen/zed/crates/terminal_view/src/terminal_panel.rs`
  - Lines 1210-1353: Drag & drop implementation
  - Lines 337-437: Pane event handling
  - Lines 387-426: Split handling

- **AgentPanel**: `/home/sunteco/quynguyen/zed/crates/agent_ui/src/agent_panel.rs`
  - Lines 601-602: PaneGroup usage
  - Similar structure to ExtensionGuiPanel

- **Workspace PaneGroup**: `/home/sunteco/quynguyen/zed/crates/workspace/src/pane_group.rs`
  - Core implementation of split/remove/resize logic

---

## Next Steps

1. **Start with Phase 1** (Event Handling) - Foundation for everything else
2. **Test each phase** before moving to next
3. **Consider Phase 4** (Tab Bar) as quick win for UX
4. **Phase 5** can be deferred if time-constrained

## Notes

- ExtensionGuiPanel is simpler than TerminalPanel (no terminal-specific logic)
- Most code can be copied/adapted from TerminalPanel
- Focus on correctness over optimization initially
- Consider adding keyboard shortcuts for split actions later
