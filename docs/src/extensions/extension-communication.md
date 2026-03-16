# Extension Communication

This guide covers how extensions communicate with Zed and with each other.

## Communication Patterns

| Method | Direction | Use Case | Example |
|--------|-----------|----------|---------|
| Pub-Sub | Ext ↔ Host ↔ Ext | Events, broadcasts | File open, status updates |
| Query | Ext ↔ Host ↔ Ext | Request-response data | Get git status, fetch config |
| Commands | Ext → Host | Execute Zed actions | Open file, select text |
| Exports | Host → Ext | Direct calls | Init, render, theme change |

## Extension → Host

### Publish Events (Recommended)

Use pub-sub to send events to the host:

```rust
use zed_extension_api::pub_sub;

// Publish an event
pub_sub::publish("ext.open-file", "/path/to/file.rs")?;

// With JSON data
let data = serde_json::json!({"key": "value"}).to_string();
pub_sub::publish("ext.custom-event", &data)?;
```

**Common Topics**:
- `ext.open-file` - Request file to be opened
- `ext.show-notification` - Display notification
- Custom topics with `ext.*` prefix

### Execute Commands

Execute Zed workspace actions:

```rust
use zed_extension_api as zed;

// With arguments
zed::execute_command("workspace::OpenPath", Some(r#"{"path":"/path/to/file"}"#))?;

// Without arguments
zed::execute_command("editor::SelectAll", None)?;
```

## Host → Extension

### Workspace Context (Auto-Subscribed)

Extensions automatically receive workspace updates via pub-sub:

```rust
impl Extension for MyExtension {
    fn on_pub_sub_event(&mut self, event: zed::pub_sub::PubSubEvent) {
        match event.topic.as_str() {
            "zed.project-root-changed" => {
                self.project_root = Some(event.data);
            }
            "zed.active-file-changed" => {
                self.active_file = Some(event.data);
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

**Standard Topics**:
- `zed.project-root-changed` - Project root path
- `zed.active-file-changed` - Active file path
- `zed.open-files-changed` - Array of open file paths (JSON)

### Direct Calls

The host calls these extension exports directly:

```rust
impl Extension for MyExtension {
    // Called once during initialization
    fn gui_init(&mut self) { }

    // Called to render UI
    fn gui_render(&mut self) -> zed::ui_elements::UiTree { }

    // Called on UI events (clicks, input)
    fn gui_on_event(&mut self, source_id: String, event: zed::gui::UiEvent) { }

    // Called when theme changes
    fn gui_on_theme_change(&mut self, theme: zed::gui::Theme) { }

    // Called on pub-sub events
    fn on_pub_sub_event(&mut self, event: zed::pub_sub::PubSubEvent) { }
}
```

## Extension ↔ Extension

### Pub-Sub Broadcast

One extension publishes, others subscribe:

**Publisher Extension**:
```rust
pub_sub::publish("ext-git.status-changed", &status_json)?;
```

**Subscriber Extension**:
```rust
impl Extension for Dashboard {
    fn gui_init(&mut self) {
        pub_sub::subscribe("ext-git.status-changed").ok();
    }

    fn on_pub_sub_event(&mut self, event: zed::pub_sub::PubSubEvent) {
        if event.topic == "ext-git.status-changed" {
            let status: GitStatus = serde_json::from_str(&event.data).ok()?;
            self.update_display(status);
        }
    }
}
```

### Query-Response

Request data from other extensions with timeout:

**Provider Extension** (using macro):
```rust
use zed_extension_api::{Extension, query_dispatch, Serialize};

#[derive(Serialize)]
struct GitStatusResponse {
    branch: String,
    staged: usize,
    unstaged: usize,
}

#[query_dispatch]
impl Extension for GitExtension {
    #[query_handler("git.status")]
    fn handle_git_status(&mut self, _req: ()) -> Result<GitStatusResponse, String> {
        Ok(GitStatusResponse {
            branch: self.branch.clone(),
            staged: self.staged.len(),
            unstaged: self.unstaged.len(),
        })
    }
}
```

**Consumer Extension**:
```rust
use zed_extension_api::query;

// Query with 1 second timeout
let responses = query::query("git.status", "", 1000)?;

for response in responses {
    let data: GitStatusResponse = serde_json::from_str(&response.data)?;
    // Use data...
}
```

## Macro Helpers

### `#[query_dispatch]`

Auto-generates query routing logic. Apply to impl block:

```rust
#[query_dispatch]
impl Extension for MyExtension {
    #[query_handler("topic1")]
    fn handle_topic1(&mut self, req: Request1) -> Result<Response1, String> {
        // Handle query
    }

    #[query_handler("topic2")]
    fn handle_topic2(&mut self, req: Request2) -> Result<Response2, String> {
        // Handle query
    }
}
```

The macro generates the `on_query` method with automatic routing and type-safe serialization.

## Best Practices

### Topic Naming Conventions

- `zed.*` - Host-initiated events
- `ext.*` - Extension requests to host
- `ext-{name}.*` - Extension-specific events
- `{domain}.{action}` - Query topics (e.g., `git.status`)

### Error Handling

Always handle errors gracefully:

```rust
// Don't panic on failures
pub_sub::publish("ext.event", &data).ok();
this.update(cx, |view, cx| { ... }).ok();
```

### Query Timeouts

Specify appropriate timeouts based on operation:

```rust
query::query("config.get", "theme", 100)?;    // Fast: 100ms
query::query("git.status", "", 1000)?;        // Medium: 1s
query::query("lsp.completions", &p, 5000)?;   // Slow: 5s
```

### Cleanup

Unsubscribe when extension unloads:

```rust
impl Drop for MyExtension {
    fn drop(&mut self) {
        if let Some(id) = self.subscription_id {
            pub_sub::unsubscribe(id).ok();
        }
        if let Some(id) = self.handler_id {
            query::unregister_query_handler(id).ok();
        }
    }
}
```

## Example: File Explorer

Complete flow for opening a file from an extension:

**1. Extension publishes event**:
```rust
fn gui_on_event(&mut self, source_id: String, _event: zed::gui::UiEvent) {
    if let Some(entry) = self.get_file_entry(&source_id) {
        let _ = pub_sub::publish("ext.open-file", &entry.path);
    }
}
```

**2. Host receives and opens file**:
The host subscribes to `ext.open-file` and calls `workspace.open_abs_path()`.

**3. Extension receives active file update**:
```rust
fn on_pub_sub_event(&mut self, event: zed::pub_sub::PubSubEvent) {
    if event.topic == "zed.active-file-changed" {
        self.active_file = Some(event.data);
        // Update UI to highlight active file
    }
}
```

## Summary

**Recommended Approach**:
- Use **pub-sub** for events and broadcasts
- Use **query** (with `#[query_dispatch]` macro) for data requests
- Use **execute_command** sparingly, only for Zed actions
- Always handle errors and specify timeouts

For detailed implementation examples and infrastructure details, see the full extension development guide.
