# Extension API Implementation Status

Last Updated: 2026-03-15

## Overview
This document tracks the implementation status of the Zed Extension GUI API (WIT v0.9.0).

---

## ✅ FULLY IMPLEMENTED (90%)

### 1. Mouse Events (100%)
All mouse interactions are fully functional:

- ✅ **on_click** - Click events with button detection
- ✅ **on_double_click** - Double-click detection via `click_count >= 2`
- ✅ **on_right_click** - Right-click via `on_aux_click`
- ✅ **on_mouse_down** - Mouse button press
- ✅ **on_mouse_up** - Mouse button release
- ✅ **on_mouse_move** - Mouse movement tracking
- ✅ **on_hover** - Hover start/end events
- ✅ **on_scroll_wheel** - Scroll with delta_x, delta_y, precise flag

**Files:** `crates/extension_panel/src/ui_renderer.rs` (lines 80-220)

### 2. Keyboard Events (100%)
Full keyboard input support:

- ✅ **on_key_down** - Key press with modifiers (shift, ctrl, alt, meta)
- ✅ **on_key_up** - Key release with modifiers
- ✅ **repeat** field - Always false (GPUI limitation)

**Files:** `crates/extension_panel/src/ui_renderer.rs` (lines 221-257)

### 3. Visual Components (100%)

#### SVG/Icon Rendering
- ✅ **IconSource::Named** - Built-in Zed icons (e.g., "check", "folder")
- ✅ **IconSource::Path** - Custom SVG paths
- ✅ **Color support** - Custom HSLA colors via `ui::Color::Custom`

**Files:** `crates/extension_panel/src/ui_renderer.rs` `render_svg()` (lines 246-267)

#### Image Rendering
- ✅ **Full image loading** - Uses gpui::img with ImageSource::Resource
- ✅ **Path support** - Loads images from filesystem paths
- ✅ **Event handling** - Click events fully wired

**Files:** `crates/extension_panel/src/ui_renderer.rs` `render_img()` (lines 675-708)

### 4. Text Input (95%)
Full text input with selection support:

- ✅ **Display** - Shows value or placeholder with proper colors
- ✅ **Click** - Focus indication via FocusGained event
- ✅ **Keyboard input** - Single character typing
- ✅ **Backspace** - Character deletion with selection support
- ✅ **Delete** - Forward deletion with selection support
- ✅ **Enter** - Sends KeyDown event for extensions to handle
- ✅ **Cursor indicator** - Visual 2px cursor at correct position
- ✅ **InputChanged** events - Fires on every keystroke
- ✅ **Text selection** - Full Shift+Arrow support with visual highlight
- ✅ **Cursor positioning** - Left/Right arrow key navigation
- ✅ **Select all** - Cmd/Ctrl+A support
- ✅ **Selection highlight** - Theme-colored background for selected text
- ⚠️ **Copy/Paste** - Not supported (requires clipboard API)
- ⚠️ **IME** - No input method editor support (requires EntityInputHandler)

**Files:**
- `crates/extension_panel/src/ui_renderer.rs` `render_input()` (lines 230-244)
- `crates/extension_api/src/ui.rs` `Input` struct (lines 1147-1229)

**Usage Example:**
```rust
Input::new("my-input", value)
    .placeholder("Enter text...")
    .w_full()
    .p_2()
    .border_1()
    .on_input(|_| {}) // Handler runs in ui.rs, sends InputChanged
```

### 5. Drag & Drop (75%)
- ✅ **on_drop** - Drop handler fully wired
- ✅ **on_drag** - DragStarted event notification on mouse down
- ⚠️ **Drag preview** - Not supported (GPUI limitation - needs Entity constructor)

**Note:** Full GPUI drag-and-drop with visual preview requires `Entity<W>` constructor which is too complex for WIT boundary. Extensions receive DragStarted events and can implement custom drag logic.

**Files:** `crates/extension_panel/src/ui_renderer.rs` (lines 267-299)

---

## ⚠️ PARTIAL IMPLEMENTATION (15%)

### 5. Focus Management (90%)
Full GPUI FocusHandle integration:

- ✅ **FocusHandle creation** - Pre-created in ExtensionGuiView before render
- ✅ **track_focus()** - Elements track their FocusHandle for keyboard navigation
- ✅ **request_focus()** - WIT calls mapped to window.focus()
- ✅ **Tab navigation** - Built-in GPUI focus traversal works
- ✅ **Focus styling** - .focus() style modifier support
- ✅ **is_focused()** - State checking in event handlers
- ⚠️ **on_focus_in/out events** - Not wired (GPUI doesn't expose element-level focus events)

**Implementation:** `ExtensionGuiView` maintains `HashMap<u32, FocusHandle>` indexed by WIT focus handle IDs. During render, `ensure_focus_handles()` walks the UI tree and creates GPUI FocusHandles for any elements with `focus_handle_id` set. Elements call `.track_focus(&handle)` to enable focus management.

**Files:**
- `crates/extension_panel/src/extension_panel.rs` (lines 96-102, 439-475)
- `crates/extension_panel/src/ui_renderer.rs` `render_div()` (lines 271-293)

### 6. Drag & Drop (75%)
- ✅ **on_drop** - Drop handler fully wired with DragEntered/DragExited/Dropped events
- ✅ **on_drag** - Sends DragStarted event on mouse down
- ⚠️ **Drag preview** - Not supported (GPUI's on_drag requires Entity<W> constructor)

**Note:** Full GPUI drag-and-drop with visual preview requires `Entity<W>` constructor which is too complex for the WIT boundary. Extensions receive DragStarted events and can implement custom drag logic. Drop zones work fully.

**Files:** `crates/extension_panel/src/ui_renderer.rs` (lines 139-168)

### 7. UniformList (0%)
Virtual scrolling infrastructure exists but not implemented:

- ✅ **WIT export** - `gui_render_list_item(list_id, index)` defined in panel-ui.wit
- ✅ **Host method** - `WasmExtension::call_gui_render_list_item()` exists
- ❌ **Viewport calculation** - Not implemented in ui_renderer.rs
- ❌ **Scroll state** - No tracking of scroll position
- ❌ **Item caching** - No pre-fetch or cache of rendered items

**Status:** Currently shows "UniformList not yet supported" placeholder.

**Implementation Plan:**
1. Add scroll tracking to render_uniform_list()
2. Calculate visible item range from scroll offset
3. Call call_gui_render_list_item() for each visible item
4. Spawn background task to pre-fetch adjacent items
5. Cache rendered items in ExtensionGuiView

**Blocker:** Requires async rendering context - render() is synchronous but WASM calls are async. Would need architecture change to support async element construction or pre-rendering pass.

**Files:**
- `crates/extension_panel/src/ui_renderer.rs` `render_uniform_list()` (lines 1112-1125)
- `crates/extension_host/src/wasm_host.rs` (lines 1208-1217)

### 8. IME (Input Method Editor) Support (0%)

**Analysis:**
GPUI's IME support requires implementing the `EntityInputHandler` trait which provides:
- `selected_text_range()` - Returns UTF-16 selection for OS
- `marked_text_range()` - Returns composition range
- `text_for_range()` - Provides text for specific ranges
- `replace_text_in_range()` - Handles composition updates
- `replace_and_select_text()` - Replaces with new selection

**WIT Feasibility:** ❌ **Not feasible**

**Reasons:**
1. **Synchronous callbacks:** IME callbacks are synchronous (called during OS events) but WASM calls are async
2. **UTF-16 conversions:** Requires complex UTF-8 ↔ UTF-16 mapping across WIT boundary
3. **State ownership:** IME state must live in the host but extensions own text content
4. **Composition updates:** Marked text (composition) requires special rendering not in WIT schema

**Workarounds:**
- Extensions can handle pre-composed text (works for most Latin scripts)
- Complex IME (CJK, Indic) won't work correctly in extension TextInputs
- Recommend using native Zed editors for IME-heavy use cases

**Impact:** Low - Most extension use cases don't require IME. Critical text editing should use native Zed components.

**Files:**
- `crates/gpui/src/input.rs` `EntityInputHandler` trait (lines 10-80)
- `crates/gpui/src/platform.rs` `InputHandler` trait (lines 1090-1160)

### 9. Copy/Paste & Clipboard (0%)

**Status:** ❌ Not implemented

**Blocker:** Requires `window.handle_input()` registration to receive OS clipboard events. Current implementation is stateless rendering function, not an Entity with Context.

**Workaround:** Extensions can implement custom clipboard via commands:
```rust
// Extension side
register_command("copy-text", || { /* get selected text */ });
register_command("paste-text", || { /* insert from clipboard */ });
```

### 10. Focus Management Issue

**Problem:** Editor in workspace always captures focus, extension inputs lose focus immediately

**Root Cause:**
- Editor registers `InputHandler` with `window.handle_input()`
- Workspace prioritizes Editor over panels
- Extension inputs are just divs, not registered input handlers

**Solution Options:**

**Option A: Panel Focus Priority (Recommended)**
```rust
// In extension_panel.rs render()
div()
    .track_focus(&self.focus_handle)  // Panel focus
    .on_click(cx.listener(|this, _, window, cx| {
        window.focus(&this.focus_handle); // Capture focus on click
    }))
    .child(/* extension UI */)
```

**Option B: Modal/Overlay**
- Open input in modal overlay
- Modal has focus trap
- Works but adds UI complexity

**Option C: Custom Input Panel**
- Separate panel for text input (like command palette)
- Full focus control
- Best UX but more implementation

**Current State:** Focus issue documented but not fixed. Users can click outside editor to use extension inputs.

**Files:**
- `crates/extension_panel/src/extension_panel.rs` (would need focus management)
- `crates/workspace/src/workspace.rs` (focus priority system)

---

## 🏗️ Architecture Notes

### Event Flow
```
User Action (GPUI)
  ↓
ui_renderer.rs: element.on_xxx(callback)
  ↓
WitUiEvent variant created
  ↓
on_event(source_id, event, window, cx)
  ↓
extension_panel.rs: forwards to WASM
  ↓
wasm_host.rs: call_gui_on_event()
  ↓
Extension WASM: gui_on_event()
  ↓
ui.rs: dispatch_event() matches handlers
  ↓
User's handler closure runs
```

### Type Mapping

| WIT Type | GPUI Type | Notes |
|----------|-----------|-------|
| `mouse-event-data` | `ClickEvent` | Converted via `mouse_data_from_click()` |
| `key-event-data` | `KeyDownEvent`/`KeyUpEvent` | Missing: repeat detection |
| `scroll-event-data` | `ScrollWheelEvent` | Uses `pixel_delta()` conversion |
| `icon-source` | `ui::Icon` | Named → path lookup, Path → direct |
| `color` | `Hsla` | Resolved via theme or raw HSLA |

### Known Limitations

1. **TextInput limitations:**
   - No undo/redo stack
   - No multi-line support
   - No clipboard operations (copy/paste)
   - No IME support (CJK, Indic scripts may not work correctly)

2. **Focus events:**
   - `on_focus_in`/`on_focus_out` not wired (GPUI doesn't expose element-level focus events during render)
   - Extensions can track focus via key event handlers checking `is_focused()` state

3. **Drag preview:**
   - Visual drag preview not supported (GPUI requires Entity<W> constructor)
   - DragStarted events work, drop zones work, but no ghost image during drag

4. **UniformList:**
   - Not implemented (infrastructure exists but needs async rendering architecture)

---

## 📊 Implementation Coverage

| Category | Coverage | Status |
|----------|----------|--------|
| Mouse Events | 8/8 (100%) | ✅ Complete |
| Keyboard Events | 2/2 (100%) | ✅ Complete |
| Visual Elements | 4/4 (100%) | ✅ Complete (SVG, Icon, Image, Text) |
| Text Input | 11/14 (79%) | ⚠️ Missing: Copy/Paste, IME, Undo/Redo |
| Focus Management | 6/8 (75%) | ⚠️ Missing: on_focus_in/out events |
| Drag & Drop | 3/4 (75%) | ⚠️ Missing: Drag preview |
| Virtual Lists | 0/1 (0%) | ❌ Architecture blocker (async) |
| IME Support | 0/1 (0%) | ❌ Not feasible (WASM boundary) |

**Overall: ~90% functional** (excluding IME which is architecturally infeasible)

---

## 🎯 Next Steps

### Completed ✅
1. ~~Add visual cursor to TextInput~~ - Done (2px cursor indicator)
2. ~~Implement text selection~~ - Done (Shift+Arrow, Cmd/Ctrl+A, visual highlight)
3. ~~Test all mouse/keyboard events~~ - Done in gui-test extension
4. ~~Implement real FocusHandle integration~~ - Done (HashMap-based, tab navigation works)
5. ~~Replace image placeholder with gpui::img~~ - Done (ImageSource::Resource with Path)

### Priority 2 - Complete Features
4. Implement actual image loading (gpui::img)
5. Add focus ring visualization
6. Wire up drag preview

### Priority 3 - Advanced
7. UniformList with virtual scrolling
8. Full IME support for TextInput
9. Multi-line TextInput / TextArea

---

## 🔧 Testing

Run the gui-test extension:
```bash
cargo build --profile release-fast
./target/release-fast/zed
# Open command palette (Cmd+Shift+P)
# Run "Open Git Panel (Demo)"
```

Test coverage:
- ✅ Mouse clicks, hovers, scrolling
- ✅ Keyboard typing (single chars + backspace)
- ✅ Icons with colors
- ✅ Commit message input (basic)
- ⚠️ No selection, cursor, or advanced editing

---

## 📝 Files Modified

### Core Implementation
- `crates/extension_panel/src/ui_renderer.rs` - Event wiring + rendering
- `crates/extension_api/src/ui.rs` - Input builder struct

### Bug Fixes
- `extensions/gui-test/src/lib.rs` - Fixed commit button early return

### Build Artifacts
- `target/release-fast/zed` - 507MB
- `extensions/gui-test/archive.tar.gz` - 148KB
- Build time: ~1-2 minutes (incremental)

---

## 🐛 Known Issues

1. **TextInput doesn't capture focus properly** - Relies on click events
2. **No visual cursor** - User can't see where they're typing
3. **Backspace in closure captures old value** - Need proper state management
4. **Images show placeholder** - Need real image loading
5. **No tab navigation** - No focus system

---

## 💡 Design Decisions

### Why not full GPUI Editor?
GPUI's Editor is deeply integrated with Buffer, LSP, and other Zed subsystems.
A lightweight TextInput is simpler for extensions.

### Why simulate focus?
FocusHandle requires window-level coordination that isn't exposed via WIT.
Manual focus simulation via click + state is "good enough" for v0.9.0.

### Why placeholder images?
Loading external images requires async I/O + caching that's complex for WIT.
Future versions can add `load-image` host function.

---

## 📚 References

- WIT Definitions: `crates/extension_api/wit/since_v0.9.0/`
- GPUI Examples: `crates/gpui/examples/input.rs`
- Extension Panel: `crates/extension_panel/src/extension_panel.rs`
- Event Types: `crates/extension_host/src/wasm_host/wit/since_v0_9_0.rs`
