# Agent Panel: Drag Tab to Center Workspace

## Tính năng: Kéo tab từ Agent Panel sang Center Panel

### ✅ **CÓ THỂ** - Đây là workspace-level feature, không phụ thuộc split pane!

---

## 🎯 Use Case

User muốn:
1. Mở agent conversation trong Agent Panel (side dock)
2. Kéo tab conversation ra center workspace để có nhiều không gian
3. Làm việc với conversation ở center (full screen)
4. Có thể kéo ngược lại panel hoặc đóng

**Example:**
```
Before:                          After:
┌─────────┬─────────┐           ┌─────────────────────┐
│ Center  │ Agent   │           │ Center              │
│         │ Panel   │   Drag    │ ┌─────────────────┐ │
│         │ ├─Tab1  │  ────→    │ │ Agent Thread 1  │ │
│         │ ├─Tab2  │           │ │ (full width)    │ │
│         │ └─Tab3  │           │ └─────────────────┘ │
└─────────┴─────────┘           └─────────────────────┘
                                       ┌─────────┐
                                       │ Agent   │
                                       │ Panel   │
                                       │ ├─Tab2  │
                                       │ └─Tab3  │
                                       └─────────┘
```

---

## 🔧 How It Works

### 1. Workspace Panes Architecture

```rust
struct Workspace {
    center: PaneGroup,              // Center workspace panes
    left_dock: Dock,                // Contains Agent Panel
    // ...
}

struct AgentPanel {
    pane: Entity<Pane>,             // Single pane (simple)
}
```

### 2. Center Panes Accept All Drops (Default)

**File**: `crates/workspace/src/workspace.rs`

```rust
let center_pane = cx.new(|cx| {
    let mut center_pane = Pane::new(
        workspace.weak_handle(),
        project.clone(),
        next_timestamp.clone(),
        None,  // ← can_drop_predicate = None = Accept ALL
        // ...
    );
    center_pane.set_can_split(Some(Arc::new(|_, _, _, _| true)));
    center_pane
});
```

**Key:** `can_drop_predicate: None` means center panes accept any `DraggedTab`

---

## 📋 Implementation for Agent Panel

### Step 1: Panel Pane Allows Dragging Out

**Pattern from Terminal Panel:**

```rust
// In new_agent_pane() function
pane.set_can_split(Some(Arc::new(move |pane, dragged_item, _window, cx| {
    if let Some(tab) = dragged_item.downcast_ref::<DraggedTab>() {
        let is_current_pane = tab.pane == cx.entity();

        // Allow drag if:
        // 1. Dragging FROM this pane
        // 2. Item is ConnectionView or TextThreadEditor
        if is_current_pane {
            let item = pane.item_for_index(tab.ix);
            if let Some(item) = item {
                return item.downcast::<ConnectionView>().is_some()
                    || item.downcast::<TextThreadEditor>().is_some();
            }
        }
    }
    false
})));
```

### Step 2: Center Accepts ConnectionView

Center panes already accept all items by default!

**No special handling needed** - workspace handles this automatically:

```rust
// Workspace automatically:
1. Detects DraggedTab over center pane
2. Calls pane.add_item() on center pane
3. Removes item from source (agent panel pane)
4. Updates focus
```

### Step 3: Support "Open in Center" Action

**Optional - Add context menu:**

```rust
impl Item for ConnectionView {
    fn tab_context_menu(&self, cx: &App) -> Option<ContextMenu> {
        Some(ContextMenu::build(cx, |menu, _, _| {
            menu.action("Open in Center", OpenInCenter::default().boxed_clone())
                .action("Close", CloseActiveItem::default().boxed_clone())
        }))
    }
}

// Action handler
impl ConnectionView {
    fn open_in_center(&mut self, _: &OpenInCenter, window: &mut Window, cx: &mut Context<Self>) {
        if let Some(workspace) = self.workspace.upgrade() {
            workspace.update(cx, |workspace, cx| {
                // Move this view to center
                let item_id = cx.entity().item_id();
                let source_pane = /* find pane containing this item */;
                let center_pane = workspace.active_pane().clone();

                workspace::move_item(
                    &source_pane,
                    &center_pane,
                    item_id,
                    center_pane.read(cx).items_len(),
                    true, // focus
                    window,
                    cx,
                );
            });
        }
    }
}
```

---

## 🎨 UX Flow

### Drag from Panel to Center

```
1. User starts drag on tab in Agent Panel
   → DraggedTab { pane: agent_panel_pane, item: ConnectionView, ... }

2. User hovers over center workspace
   → Center pane highlights as drop target
   → Shows blue insertion indicator

3. User releases
   → Workspace calls: move_item(agent_pane, center_pane, item_id, ...)
   → Item removed from agent panel
   → Item added to center pane
   → Focus moves to center

4. Agent Panel now has one less tab
   → If was last tab, panel becomes empty (shows placeholder)
```

### Drag from Center back to Panel

```
1. User drags tab in center workspace
2. Hovers over Agent Panel
3. Panel pane accepts drop (if item type matches)
4. Item moves back to panel
```

---

## ⚙️ Configuration

### Agent Panel Pane Setup

```rust
fn new_agent_pane(
    workspace: WeakEntity<Workspace>,
    project: Entity<Project>,
    window: &mut Window,
    cx: &mut Context<AgentPanel>,
) -> Entity<Pane> {
    cx.new(|cx| {
        let mut pane = Pane::new(
            workspace.clone(),
            project.clone(),
            Default::default(),
            Some(Arc::new(|dropped_item, _, cx| {
                // Optional: Restrict what can be dropped INTO panel
                // For now, accept ConnectionView and TextThreadEditor
                if let Some(tab) = dropped_item.downcast_ref::<DraggedTab>() {
                    return tab.item.downcast::<ConnectionView>().is_some()
                        || tab.item.downcast::<TextThreadEditor>().is_some();
                }
                false
            })),
            ToggleFocus.boxed_clone(),
            false,
            window,
            cx,
        );

        // Allow dragging out to center
        pane.set_can_split(Some(Arc::new(|pane, dragged_item, _, cx| {
            if let Some(tab) = dragged_item.downcast_ref::<DraggedTab>() {
                let is_current_pane = tab.pane == cx.entity();
                if is_current_pane {
                    if let Some(item) = pane.item_for_index(tab.ix) {
                        return item.downcast::<ConnectionView>().is_some()
                            || item.downcast::<TextThreadEditor>().is_some();
                    }
                }
            }
            false
        })));

        pane.set_can_navigate(false, cx);
        pane.set_should_display_tab_bar(|_, _| true);
        pane
    })
}
```

---

## 🚀 Benefits

### ✅ Works with Single Pane Design

- **No PaneGroup needed** in Agent Panel
- **Workspace handles cross-panel drag** automatically
- **Simple panel implementation** (just one pane)

### ✅ Better UX

1. **Small screen:** Keep agent in panel (compact)
2. **Large screen:** Drag to center (more space)
3. **Multi-monitor:** Could even detach to separate window (future)

### ✅ No Complexity Added

- Drag-to-center is **workspace feature**, not panel feature
- Agent Panel just needs to:
  - Set `can_split` = true (allow drag out)
  - Implement proper Item trait on views
- **0 extra lines** for split pane logic

---

## 🔍 Comparison: Panel vs Center

### Agent in Panel (Side Dock)
```
Pros:
✅ Always visible
✅ Quick access
✅ Doesn't interfere with editor
✅ Can see code + agent side-by-side

Cons:
❌ Limited width
❌ Scrolling for long responses
```

### Agent in Center (Main Workspace)
```
Pros:
✅ Full width
✅ Better for long conversations
✅ Can split with editor in center
✅ More immersive chat experience

Cons:
❌ Covers editor
❌ Need to switch back to see code
```

**Solution:** Let user choose! Drag tab wherever they want.

---

## 📦 Implementation Checklist

### Phase 1: Basic Drag Support (Week 1)
- [ ] `set_can_split` allows dragging ConnectionView out
- [ ] Test: Drag tab from panel to center ✓
- [ ] Test: Tab moves correctly ✓
- [ ] Test: Focus follows item ✓

### Phase 2: Polish (Week 2)
- [ ] Tab context menu: "Open in Center"
- [ ] Tab context menu: "Open in New Window" (future)
- [ ] Handle edge cases (last tab, empty panel)
- [ ] Serialization (remember which items were in center)

### Phase 3: Advanced (Optional)
- [ ] Keyboard shortcut: Move to Center
- [ ] Double-click tab bar → Open in Center
- [ ] Remember user preference (always open in center vs panel)

---

## 🎯 Recommendation

**YES - Definitely support drag-to-center!**

Reasons:
1. ✅ **Zero complexity cost** (workspace feature, not panel feature)
2. ✅ **Huge UX benefit** (user choice of layout)
3. ✅ **Standard pattern** (Terminal Panel does this)
4. ✅ **Works perfectly with single pane** (no split needed)

Implementation:
```rust
// In Agent Panel:
fn new_agent_pane(...) {
    pane.set_can_split(Some(Arc::new(can_drag_out_predicate)));
    // That's it! Workspace handles the rest.
}
```

**Just ~10 lines of code for this major UX feature!**

---

## 💡 Future: Multi-Window Support

If you want to go further:

```rust
// Tab context menu
menu.action("Detach to New Window", DetachTab::default().boxed_clone())

// Implementation
fn detach_tab(&self, ...) {
    // Create new OS window
    // Move ConnectionView to new window's workspace
    // User gets dedicated agent window!
}
```

This gives users ultimate flexibility without complex split pane logic in panel.
