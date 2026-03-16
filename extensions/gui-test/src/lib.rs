use serde::Serialize;
use zed_extension_api::{self as zed, Extension, query_dispatch, register_command, ui};
use zed::process::Command;
use zed::query;
use zed::ui::{Color, Div, Input, Label, div, v_flex, h_flex};

#[derive(Clone)]
enum FileStatus {
    Added,
    Modified,
    Deleted,
    Renamed,
}

impl FileStatus {
    fn label(&self) -> &str {
        match self {
            FileStatus::Added => "A",
            FileStatus::Modified => "M",
            FileStatus::Deleted => "D",
            FileStatus::Renamed => "R",
        }
    }

    fn color(&self) -> Color {
        match self {
            FileStatus::Added => ui::color_success(),
            FileStatus::Modified => ui::color_warning(),
            FileStatus::Deleted => ui::color_error(),
            FileStatus::Renamed => ui::color_accent(),
        }
    }
}

#[derive(Clone)]
struct FileEntry {
    path: String,
    status: FileStatus,
}

#[derive(Serialize)]
struct GitStatusResponse {
    has_repo: bool,
    branch: String,
    staged: usize,
    unstaged: usize,
}

enum PanelState {
    Loading,
    NoRepo,
    Error(String),
    Ready,
}

struct GitPanel {
    project_root: Option<String>,
    open_files: Vec<String>,
    branch: String,
    staged: Vec<FileEntry>,
    unstaged: Vec<FileEntry>,
    commit_message: String,
    staged_collapsed: bool,
    unstaged_collapsed: bool,
    state: PanelState,
    query_handler_id: Option<u64>,
}

fn git(root: &str, args: &[&str]) -> Result<String, String> {
    let output = Command::new("git")
        .arg("-C")
        .arg(root)
        .args(args.iter().copied())
        .output()?;
    if output.status.unwrap_or(1) != 0 {
        return Err(String::from_utf8_lossy(&output.stderr).trim().to_string());
    }
    Ok(String::from_utf8_lossy(&output.stdout).trim().to_string())
}

fn has_commits(root: &str) -> bool {
    git(root, &["rev-parse", "HEAD"]).is_ok()
}

fn char_to_status(c: char) -> FileStatus {
    match c {
        'A' => FileStatus::Added,
        'D' => FileStatus::Deleted,
        'R' | 'C' => FileStatus::Renamed,
        _ => FileStatus::Modified,
    }
}

fn refresh_status(root: &str) -> Result<(String, Vec<FileEntry>, Vec<FileEntry>), String> {
    let branch = git(root, &["symbolic-ref", "--short", "HEAD"])
        .unwrap_or_else(|_| "HEAD".to_string());

    let status_out = git(root, &["status", "--porcelain=v1"])?;
    let mut staged = Vec::new();
    let mut unstaged = Vec::new();

    for line in status_out.lines() {
        if line.len() < 3 {
            continue;
        }
        let x = line.as_bytes()[0] as char;
        let y = line.as_bytes()[1] as char;
        // For renames, the path may be "old -> new"; use the new path.
        let path = if let Some((_, new)) = line[3..].split_once(" -> ") {
            new.to_string()
        } else {
            line[3..].to_string()
        };

        if x == '?' && y == '?' {
            unstaged.push(FileEntry { path, status: FileStatus::Added });
            continue;
        }
        if x != ' ' {
            staged.push(FileEntry { path: path.clone(), status: char_to_status(x) });
        }
        if y != ' ' {
            unstaged.push(FileEntry { path, status: char_to_status(y) });
        }
    }

    Ok((branch, staged, unstaged))
}

fn file_row(
    id: impl Into<String>,
    path: &str,
    status_label: &str,
    status_color: Color,
    action: &str,
) -> Div {
    h_flex()
        .id(id)
        .px_3()
        .py(ui::px(3.0))
        .items_center()
        .gap_x(ui::def_px(6.0))
        .cursor_pointer()
        .on_click(|_| {})
        .child(Label::new(status_label).color(status_color).bold().text_xs())
        .child(
            div()
                .flex_1()
                .min_w(ui::def_px(0.0))
                .overflow_x_hidden()
                .child(Label::new(path).text_sm().single_line()),
        )
        .child(Label::new(action).text_xs().muted())
}

fn rule() -> Div {
    div().w_full().h(ui::px(1.0)).bg(ui::color_border())
}

#[query_dispatch]
impl Extension for GitPanel {
    fn new() -> Self {
        register_command("open-panel", "Open Git Panel (Demo)");
        Self {
            project_root: None,
            open_files: Vec::new(),
            branch: String::new(),
            staged: Vec::new(),
            unstaged: Vec::new(),
            commit_message: String::new(),
            staged_collapsed: false,
            unstaged_collapsed: false,
            state: PanelState::Loading,
            query_handler_id: None,
        }
    }

    fn gui_init(&mut self) {
        self.query_handler_id = query::register_query_handler("git.status").ok();
        // Workspace context is delivered via pub-sub immediately after init by the host.
    }

    fn on_pub_sub_event(&mut self, event: zed::pub_sub::PubSubEvent) {
        match event.topic.as_str() {
            "zed.project-root-changed" => {
                let root = event.data;
                if root.is_empty() {
                    self.project_root = None;
                    self.state = PanelState::NoRepo;
                    return;
                }
                self.project_root = Some(root.clone());
                match refresh_status(&root) {
                    Ok((branch, staged, unstaged)) => {
                        self.branch = branch;
                        self.staged = staged;
                        self.unstaged = unstaged;
                        self.state = PanelState::Ready;
                    }
                    Err(err) => {
                        self.state = PanelState::Error(err);
                    }
                }
            }
            "zed.open-files-changed" => {
                if let Ok(files) = zed::serde_json::from_str::<Vec<String>>(&event.data) {
                    self.open_files = files;
                }
            }
            _ => {}
        }
    }

    #[query_handler("git.status")]
    fn handle_git_status(&mut self, _req: ()) -> Result<GitStatusResponse, String> {
        let Some(root) = &self.project_root else {
            return Ok(GitStatusResponse {
                has_repo: false,
                branch: String::new(),
                staged: 0,
                unstaged: 0,
            });
        };
        match refresh_status(root) {
            Ok((branch, staged, unstaged)) => Ok(GitStatusResponse {
                has_repo: true,
                branch,
                staged: staged.len(),
                unstaged: unstaged.len(),
            }),
            Err(e) => Err(e),
        }
    }

    fn gui_on_event(&mut self, source_id: String, event: zed::gui::UiEvent) {
        // Handle non-git UI operations first (no refresh needed).
        match source_id.as_str() {
            "toggle-staged" => {
                self.staged_collapsed = !self.staged_collapsed;
                return;
            }
            "toggle-unstaged" => {
                self.unstaged_collapsed = !self.unstaged_collapsed;
                return;
            }
            "commit-input" => {
                if let zed::gui::UiEvent::InputChanged(new_value) = event {
                    eprintln!("[gui-test v0.1.2] InputChanged event received: '{}'", new_value);
                    self.commit_message = new_value;
                    eprintln!("[gui-test v0.1.2] Updated commit_message to: '{}'", self.commit_message);
                }
                return;
            }
            "msg-clear" => {
                self.commit_message.clear();
                return;
            }
            "msg-feat" => {
                self.commit_message = "feat: add new feature".into();
                return;
            }
            "msg-fix" => {
                self.commit_message = "fix: resolve issue".into();
                return;
            }
            _ => {}
        }

        let Some(root) = self.project_root.clone() else {
            return;
        };

        let git_result = match source_id.as_str() {
            "refresh" => Ok(()),
            "stage-all" => git(&root, &["add", "-A"]).map(|_| ()),
            "unstage-all" => if has_commits(&root) {
                git(&root, &["restore", "--staged", "."]).map(|_| ())
            } else {
                git(&root, &["rm", "--cached", "-r", "--", "."]).map(|_| ())
            },
            "commit" => {
                if self.staged.is_empty() || self.commit_message.is_empty() {
                    Ok(())
                } else {
                    match git(&root, &["commit", "-m", &self.commit_message]) {
                        Ok(_) => {
                            self.commit_message.clear();
                            Ok(())
                        }
                        Err(err) => Err(err),
                    }
                }
            }
            id => {
                if let Some(rest) = id.strip_prefix("stage-file-") {
                    if let Ok(idx) = rest.parse::<usize>() {
                        if let Some(entry) = self.unstaged.get(idx) {
                            git(&root, &["add", "--", &entry.path.clone()]).map(|_| ())
                        } else {
                            return;
                        }
                    } else {
                        return;
                    }
                } else if let Some(rest) = id.strip_prefix("unstage-file-") {
                    if let Ok(idx) = rest.parse::<usize>() {
                        if let Some(entry) = self.staged.get(idx) {
                            if has_commits(&root) {
                                git(&root, &["restore", "--staged", "--", &entry.path.clone()]).map(|_| ())
                            } else {
                                git(&root, &["rm", "--cached", "--", &entry.path.clone()]).map(|_| ())
                            }
                        } else {
                            return;
                        }
                    } else {
                        return;
                    }
                } else {
                    return;
                }
            }
        };

        if let Err(err) = git_result {
            self.state = PanelState::Error(err);
            return;
        }

        match refresh_status(&root) {
            Ok((branch, staged, unstaged)) => {
                self.branch = branch;
                self.staged = staged;
                self.unstaged = unstaged;
                self.state = PanelState::Ready;
            }
            Err(err) => {
                self.state = PanelState::Error(err);
            }
        }
    }

    fn gui_render(&mut self) -> zed::ui_elements::UiTree {
        ui::clear_handlers();

        // Error / loading states
        match &self.state {
            PanelState::Loading => {
                return ui::render_tree(
                    v_flex()
                        .size_full()
                        .items_center()
                        .justify_center()
                        .child(Label::new("Loading…").muted()),
                );
            }
            PanelState::NoRepo => {
                return ui::render_tree(
                    v_flex()
                        .size_full()
                        .items_center()
                        .justify_center()
                        .child(Label::new("No git repository found").muted()),
                );
            }
            PanelState::Error(err) => {
                let msg = format!("Error: {err}");
                return ui::render_tree(
                    v_flex()
                        .size_full()
                        .p_3()
                        .gap_y(ui::def_px(8.0))
                        .child(Label::new(msg).color(ui::color_error()).text_sm())
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
                                .child(Label::new("Retry").text_sm()),
                        ),
                );
            }
            PanelState::Ready => {}
        }

        let open_files = self.open_files.clone();
        let staged_count = self.staged.len();
        let unstaged_count = self.unstaged.len();
        let total = staged_count + unstaged_count;
        let branch = self.branch.clone();
        let commit_msg = self.commit_message.clone();
        let commit_msg_is_empty = commit_msg.is_empty();
        let staged_collapsed = self.staged_collapsed;
        let unstaged_collapsed = self.unstaged_collapsed;
        let can_commit = staged_count > 0 && !commit_msg_is_empty;

        let staged_rows: Vec<Div> = self
            .staged
            .iter()
            .enumerate()
            .map(|(i, f)| {
                file_row(
                    format!("unstage-file-{i}"),
                    &f.path,
                    f.status.label(),
                    f.status.color(),
                    "−",
                )
            })
            .collect();

        let unstaged_rows: Vec<Div> = self
            .unstaged
            .iter()
            .enumerate()
            .map(|(i, f)| {
                file_row(
                    format!("stage-file-{i}"),
                    &f.path,
                    f.status.label(),
                    f.status.color(),
                    "+",
                )
            })
            .collect();

        let commit_label = if staged_count > 0 {
            format!(
                "Commit {} file{}",
                staged_count,
                if staged_count == 1 { "" } else { "s" }
            )
        } else {
            "Commit".into()
        };

        let mut root = v_flex()
            .size_full()
            .overflow_y_scroll()
            // ── Extension version ────────────────────────────────────────
            .child(
                h_flex()
                    .px_3()
                    .py_1()
                    .justify_end()
                    .child(Label::new("v0.1.2").muted().text_xs()),
            )
            // ── Repository header ────────────────────────────────────────
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
                            .child(Label::new("⎇").color(ui::color_accent()))
                            .child(Label::new(branch).bold()),
                    )
                    .child(
                        h_flex()
                            .gap_x(ui::def_px(6.0))
                            .items_center()
                            .child(
                                Label::new(format!("{total} changes"))
                                    .muted()
                                    .text_xs(),
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
                                    .child(Label::new("↺").text_xs().muted()),
                            ),
                    ),
            )
            .child(rule())
            // ── Open files section ───────────────────────────────────────
            .child(
                h_flex()
                    .px_3()
                    .py_1()
                    .items_center()
                    .justify_between()
                    .child(
                        h_flex()
                            .gap_x(ui::def_px(4.0))
                            .items_center()
                            .child(Label::new("Open Files").bold().text_sm())
                            .child(
                                div()
                                    .px(ui::px(6.0))
                                    .py(ui::px(1.0))
                                    .rounded_full()
                                    .bg(ui::color_surface())
                                    .child(Label::new(open_files.len().to_string()).text_xs().muted()),
                            ),
                    ),
            )
            .children(open_files.iter().map(|path| {
                let name = path.rsplit('/').next().unwrap_or(path);
                h_flex()
                    .px_3()
                    .py(ui::px(2.0))
                    .items_center()
                    .gap_x(ui::def_px(6.0))
                    .child(Label::new("○").muted().text_xs())
                    .child(
                        div()
                            .flex_1()
                            .min_w(ui::def_px(0.0))
                            .overflow_x_hidden()
                            .child(Label::new(name.to_string()).text_sm().single_line()),
                    )
            }))
            .child(rule())
            // ── Staged section header ────────────────────────────────────
            .child(
                h_flex()
                    .id("toggle-staged")
                    .px_3()
                    .py_1()
                    .items_center()
                    .justify_between()
                    .cursor_pointer()
                    .on_click(|_| {})
                    .child(
                        h_flex()
                            .gap_x(ui::def_px(4.0))
                            .items_center()
                            .child(
                                Label::new(if staged_collapsed { "▶" } else { "▼" })
                                    .muted()
                                    .text_xs(),
                            )
                            .child(Label::new("Staged Changes").bold().text_sm())
                            .child(
                                div()
                                    .px(ui::px(6.0))
                                    .py(ui::px(1.0))
                                    .rounded_full()
                                    .bg(ui::color_surface())
                                    .child(Label::new(staged_count.to_string()).text_xs().muted()),
                            ),
                    )
                    .when(staged_count > 0, |this| {
                        this.child(
                            div()
                                .id("unstage-all")
                                .px_2()
                                .py(ui::px(2.0))
                                .rounded_sm()
                                .border_1()
                                .border_color(ui::color_border())
                                .cursor_pointer()
                                .on_click(|_| {})
                                .child(Label::new("Unstage All").text_xs().muted()),
                        )
                    }),
            );

        if !staged_collapsed {
            if staged_rows.is_empty() {
                root = root.child(
                    div()
                        .px_3()
                        .py_2()
                        .child(Label::new("No staged changes").muted().text_xs().italic()),
                );
            } else {
                root = root.children(staged_rows);
            }
        }

        root = root
            .child(rule())
            // ── Unstaged section header ──────────────────────────────────
            .child(
                h_flex()
                    .id("toggle-unstaged")
                    .px_3()
                    .py_1()
                    .items_center()
                    .justify_between()
                    .cursor_pointer()
                    .on_click(|_| {})
                    .child(
                        h_flex()
                            .gap_x(ui::def_px(4.0))
                            .items_center()
                            .child(
                                Label::new(if unstaged_collapsed { "▶" } else { "▼" })
                                    .muted()
                                    .text_xs(),
                            )
                            .child(Label::new("Unstaged Changes").bold().text_sm())
                            .child(
                                div()
                                    .px(ui::px(6.0))
                                    .py(ui::px(1.0))
                                    .rounded_full()
                                    .bg(ui::color_surface())
                                    .child(
                                        Label::new(unstaged_count.to_string()).text_xs().muted(),
                                    ),
                            ),
                    )
                    .when(unstaged_count > 0, |this| {
                        this.child(
                            div()
                                .id("stage-all")
                                .px_2()
                                .py(ui::px(2.0))
                                .rounded_sm()
                                .border_1()
                                .border_color(ui::color_border())
                                .cursor_pointer()
                                .on_click(|_| {})
                                .child(Label::new("Stage All").text_xs()),
                        )
                    }),
            );

        if !unstaged_collapsed {
            if unstaged_rows.is_empty() {
                root = root.child(
                    div()
                        .px_3()
                        .py_2()
                        .child(Label::new("No unstaged changes").muted().text_xs().italic()),
                );
            } else {
                root = root.children(unstaged_rows);
            }
        }

        root = root
            .child(rule())
            // ── Commit area ──────────────────────────────────────────────
            .child(
                v_flex()
                    .p_3()
                    .gap_y(ui::def_px(8.0))
                    // Commit message input
                    .child(
                        Input::new("commit-input", commit_msg)
                            .placeholder("Enter commit message...")
                            .w_full()
                            .p_2()
                            .rounded_md()
                            .border_1()
                            .border_color(ui::color_border())
                            .bg(ui::color_surface())
                            .text_sm(),
                    )
                    // Quick-fill preset row
                    .child(
                        h_flex()
                            .gap_x(ui::def_px(6.0))
                            .child(
                                div()
                                    .id("msg-feat")
                                    .px_2()
                                    .py(ui::px(3.0))
                                    .rounded_sm()
                                    .border_1()
                                    .border_color(ui::color_border())
                                    .cursor_pointer()
                                    .on_click(|_| {})
                                    .child(Label::new("feat: …").text_xs().muted()),
                            )
                            .child(
                                div()
                                    .id("msg-fix")
                                    .px_2()
                                    .py(ui::px(3.0))
                                    .rounded_sm()
                                    .border_1()
                                    .border_color(ui::color_border())
                                    .cursor_pointer()
                                    .on_click(|_| {})
                                    .child(Label::new("fix: …").text_xs().muted()),
                            )
                            .when(!commit_msg_is_empty, |this| {
                                this.child(
                                    div()
                                        .id("msg-clear")
                                        .px_2()
                                        .py(ui::px(3.0))
                                        .rounded_sm()
                                        .border_1()
                                        .border_color(ui::color_border())
                                        .cursor_pointer()
                                        .on_click(|_| {})
                                        .child(Label::new("Clear").text_xs().color(ui::color_error())),
                                )
                            }),
                    )
                    // Commit button
                    .child(
                        h_flex()
                            .id("commit")
                            .w_full()
                            .py_2()
                            .rounded_md()
                            .justify_center()
                            .items_center()
                            .when(can_commit, |this| {
                                this.bg(ui::color_accent())
                                    .text_color(ui::color_background())
                                    .cursor_pointer()
                                    .on_click(|_| {})
                            })
                            .when(!can_commit, |this| {
                                this.bg(ui::color_surface()).text_color(ui::color_muted())
                            })
                            .child(Label::new(commit_label).bold().text_sm()),
                    ),
            );

        let tree = ui::render_tree(root);
        eprintln!("[gui-test] gui_render: Ready -> tree nodes={} root={}", tree.nodes.len(), tree.root);
        tree
    }

    fn run_extension_command(&mut self, command_id: &str) -> Result<(), String> {
        match command_id {
            "open-panel" => Ok(()),
            _ => Err(format!("unknown command: {command_id}")),
        }
    }
}

zed_extension_api::register_extension!(GitPanel);
