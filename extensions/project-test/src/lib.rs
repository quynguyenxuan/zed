use zed_extension_api::{self as zed, Extension, register_command, ui};
use zed::ui::{Div, Label, div, v_flex, h_flex};
use zed::process::Command;
use std::path::Path;

#[derive(Clone)]
enum EntryKind {
    Directory,
    File,
}

#[derive(Clone)]
struct FileEntry {
    name: String,
    path: String,
    kind: EntryKind,
    depth: usize,
}

struct ProjectPanel {
    project_root: Option<String>,
    active_file: Option<String>,
    entries: Vec<FileEntry>,
    collapsed_dirs: Vec<String>,
    loading: bool,
    error: Option<String>,
}

fn should_skip(name: &str) -> bool {
    // Skip hidden files and common build/dependency directories
    name.starts_with('.') || name == "target" || name == "node_modules"
}

fn list_directory(root: &str) -> Result<Vec<FileEntry>, String> {
    let mut entries = Vec::new();

    for (kind, type_flag) in [(EntryKind::Directory, "d"), (EntryKind::File, "f")] {
        let output = Command::new("find")
            .arg(root)
            .arg("-mindepth").arg("1")
            .arg("-maxdepth").arg("6")
            .arg("-type").arg(type_flag)
            .arg("-not").arg("-name").arg(".*")
            .arg("-not").arg("-path").arg("*/target/*")
            .arg("-not").arg("-path").arg("*/node_modules/*")
            .output()
            .map_err(|e| format!("find failed: {e}"))?;

        if output.status.unwrap_or(1) != 0 {
            return Err(String::from_utf8_lossy(&output.stderr).trim().to_string());
        }

        for line in String::from_utf8_lossy(&output.stdout).lines() {
            let path = line.trim().to_string();
            if path.is_empty() {
                continue;
            }
            let name = path.rsplit('/').next().unwrap_or("").to_string();
            if should_skip(&name) {
                continue;
            }
            let relative = path.strip_prefix(root).unwrap_or(&path).trim_start_matches('/');
            let depth = if relative.is_empty() { 0 } else { relative.split('/').count() - 1 };
            entries.push(FileEntry { name, path, kind: kind.clone(), depth });
        }
    }

    // Sort: directories first, then alphabetically by path
    entries.sort_by(|a, b| match (&a.kind, &b.kind) {
        (EntryKind::Directory, EntryKind::File) => std::cmp::Ordering::Less,
        (EntryKind::File, EntryKind::Directory) => std::cmp::Ordering::Greater,
        _ => a.path.cmp(&b.path),
    });

    Ok(entries)
}

fn file_icon(kind: &EntryKind, name: &str) -> &'static str {
    match kind {
        EntryKind::Directory => "📁",
        EntryKind::File => {
            if name.ends_with(".rs") {
                "🦀"
            } else if name.ends_with(".toml") {
                "⚙️"
            } else if name.ends_with(".md") {
                "📝"
            } else if name.ends_with(".json") {
                "📋"
            } else {
                "📄"
            }
        }
    }
}

fn render_entry(entry: &FileEntry, index: usize, collapsed: bool, active_file: Option<&str>) -> Div {
    let icon = file_icon(&entry.kind, &entry.name);
    let indent = entry.depth * 16;
    let is_active = matches!(entry.kind, EntryKind::File)
        && active_file.map_or(false, |af| af == entry.path);

    let id = format!("entry-{}", index);
    let mut row = h_flex()
        .id(id)
        .px_2()
        .py(ui::px(2.0))
        .items_center()
        .gap_x(ui::def_px(6.0))
        .cursor_pointer()
        .on_click(|_| {})
        .when(is_active, |this| this.bg(ui::color_surface()));

    // Indent
    if indent > 0 {
        row = row.child(div().w(ui::px(indent as f32)));
    }

    // Collapse/expand indicator for directories
    if matches!(entry.kind, EntryKind::Directory) {
        row = row.child(
            Label::new(if collapsed { "▶" } else { "▼" })
                .text_xs()
                .muted()
        );
    } else {
        row = row.child(div().w(ui::px(12.0)));
    }

    // Icon and name
    let name_label = if is_active {
        Label::new(entry.name.clone()).text_sm().color(ui::color_accent())
    } else {
        Label::new(entry.name.clone()).text_sm()
    };
    row = row.child(Label::new(icon).text_sm()).child(name_label);
    if is_active {
        row = row.child(Label::new("●").text_xs().color(ui::color_accent()));
    }

    row
}

impl Extension for ProjectPanel {
    fn new() -> Self {
        register_command("open-panel", "Open Project Panel (Test)");
        Self {
            project_root: None,
            active_file: None,
            entries: Vec::new(),
            collapsed_dirs: Vec::new(),
            loading: true,
            error: None,
        }
    }

    fn gui_init(&mut self) {
        // project_root and active_file are delivered via pub-sub immediately after init.
    }

    fn on_pub_sub_event(&mut self, event: zed::pub_sub::PubSubEvent) {
        match event.topic.as_str() {
            "zed.project-root-changed" => {
                if event.data.is_empty() {
                    self.project_root = None;
                    self.loading = false;
                    self.error = Some("No project root found".to_string());
                    return;
                }
                self.project_root = Some(event.data.clone());
                self.error = None;
                match list_directory(&event.data) {
                    Ok(entries) => {
                        self.entries = entries;
                        self.loading = false;
                    }
                    Err(err) => {
                        self.loading = false;
                        self.error = Some(err);
                    }
                }
            }
            "zed.active-file-changed" => {
                self.active_file = if event.data.is_empty() {
                    None
                } else {
                    Some(event.data)
                };
            }
            _ => {}
        }
    }

    fn gui_on_event(&mut self, source_id: String, _event: zed::gui::UiEvent) {
        if source_id == "refresh" {
            self.loading = true;
            self.error = None;
            let Some(root) = self.project_root.clone() else {
                self.loading = false;
                self.error = Some("No project root".to_string());
                return;
            };

            match list_directory(&root) {
                Ok(entries) => {
                    self.entries = entries;
                    self.loading = false;
                }
                Err(err) => {
                    self.loading = false;
                    self.error = Some(err);
                }
            }
            return;
        }

        // Handle directory collapse/expand
        if let Some(rest) = source_id.strip_prefix("entry-") {
            if let Ok(idx) = rest.parse::<usize>() {
                if let Some(entry) = self.entries.get(idx) {
                    match entry.kind {
                        EntryKind::Directory => {
                            // Toggle collapse state
                            if let Some(pos) = self.collapsed_dirs.iter().position(|p| p == &entry.path) {
                                self.collapsed_dirs.remove(pos);
                            } else {
                                self.collapsed_dirs.push(entry.path.clone());
                            }
                        }
                        EntryKind::File => {
                            let path = entry.path.clone();
                            let params = format!(
                                r#"{{"path":"{}"}}"#,
                                path.replace('\\', "\\\\").replace('"', "\\\"")
                            );
                            let _ = zed::gui::call("", "open_file", &params);
                        }
                    }
                }
            }
        }
    }

    fn gui_render(&mut self) -> zed::ui_elements::UiTree {
        ui::clear_handlers();

        if self.loading {
            return ui::render_tree(
                v_flex()
                    .size_full()
                    .items_center()
                    .justify_center()
                    .child(Label::new("Loading...").muted())
            );
        }

        if let Some(err) = &self.error {
            return ui::render_tree(
                v_flex()
                    .size_full()
                    .p_3()
                    .gap_y(ui::def_px(8.0))
                    .child(Label::new(format!("Error: {}", err)).color(ui::color_error()).text_sm())
                    .child(
                        div()
                            .id("refresh")
                            .px_3()
                            .py_1()
                            .rounded_md()
                            .border_1()
                            .border_color(ui::color_border())
                            .cursor_pointer()
                            .on_click(|_| {})
                            .child(Label::new("Retry").text_sm())
                    )
            );
        }

        let root_name = self.project_root.as_ref()
            .and_then(|p| Path::new(p).file_name())
            .and_then(|n| n.to_str())
            .unwrap_or("Project");

        let mut root = v_flex()
            .size_full()
            .overflow_y_scroll()
            .child(
                h_flex()
                    .px_3()
                    .py_2()
                    .items_center()
                    .justify_between()
                    .child(
                        h_flex()
                            .gap_x(ui::def_px(6.0))
                            .items_center()
                            .child(Label::new("📦").text_sm())
                            .child(Label::new(root_name).bold().text_sm())
                    )
                    .child(
                        div()
                            .id("refresh")
                            .px(ui::px(6.0))
                            .py(ui::px(2.0))
                            .rounded_sm()
                            .border_1()
                            .border_color(ui::color_border())
                            .cursor_pointer()
                            .on_click(|_| {})
                            .child(Label::new("↺").text_xs().muted())
                    )
            )
            .child(
                div()
                    .w_full()
                    .h(ui::px(1.0))
                    .bg(ui::color_border())
            );

        // Render file tree
        if self.entries.is_empty() {
            root = root.child(
                div()
                    .px_3()
                    .py_2()
                    .child(Label::new("No files found").muted().text_xs().italic())
            );
        } else {
            // Filter out collapsed directories' children
            let mut visible_entries = Vec::new();
            for (i, entry) in self.entries.iter().enumerate() {
                // Check if any parent is collapsed
                let mut should_hide = false;
                for collapsed in &self.collapsed_dirs {
                    if entry.path.starts_with(collapsed) && &entry.path != collapsed {
                        should_hide = true;
                        break;
                    }
                }

                if !should_hide {
                    let is_collapsed = self.collapsed_dirs.contains(&entry.path);
                    visible_entries.push((i, entry, is_collapsed));
                }
            }

            let active_file = self.active_file.as_deref();
            let rows: Vec<Div> = visible_entries
                .into_iter()
                .map(|(i, entry, collapsed)| render_entry(entry, i, collapsed, active_file))
                .collect();

            root = root.children(rows);
        }

        ui::render_tree(root)
    }

    fn run_extension_command(&mut self, command_id: &str) -> Result<(), String> {
        match command_id {
            "open-panel" => Ok(()),
            _ => Err(format!("unknown command: {}", command_id)),
        }
    }
}

zed_extension_api::register_extension!(ProjectPanel);
