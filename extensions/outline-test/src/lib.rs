use zed_extension_api::{self as zed, Extension, register_command, ui};
use zed::process::Command;
use zed::ui::{Color, Div, Label, div, v_flex, h_flex};

#[derive(Clone, PartialEq)]
enum SymbolKind {
    Function,
    Struct,
    Enum,
    Impl,
    Trait,
    Module,
    Type,
    Constant,
    Class,
    Other,
}

impl SymbolKind {
    fn label(&self) -> &str {
        match self {
            SymbolKind::Function => "fn",
            SymbolKind::Struct => "st",
            SymbolKind::Enum => "en",
            SymbolKind::Impl => "im",
            SymbolKind::Trait => "tr",
            SymbolKind::Module => "mo",
            SymbolKind::Type => "ty",
            SymbolKind::Constant => "co",
            SymbolKind::Class => "cl",
            SymbolKind::Other => "??",
        }
    }

    fn color(&self) -> Color {
        match self {
            SymbolKind::Function => ui::color_accent(),
            SymbolKind::Struct => ui::color_success(),
            SymbolKind::Enum => ui::color_success(),
            SymbolKind::Impl => ui::color_warning(),
            SymbolKind::Trait => ui::color_warning(),
            SymbolKind::Module => ui::color_accent(),
            SymbolKind::Type => ui::color_accent(),
            SymbolKind::Constant => ui::color_muted(),
            SymbolKind::Class => ui::color_success(),
            SymbolKind::Other => ui::color_muted(),
        }
    }
}

#[derive(Clone)]
struct Symbol {
    name: String,
    kind: SymbolKind,
    line: u32,
    indent: u32,
}

enum PanelState {
    NoFile,
    Error(String),
    Ready,
}

struct OutlinePanel {
    active_file: Option<String>,
    symbols: Vec<Symbol>,
    state: PanelState,
}

// Detect symbol kind from a matched line of source code.
fn classify_line(line: &str) -> Option<(SymbolKind, String)> {
    let trimmed = line.trim();

    // Rust patterns
    for prefix in &["pub(crate) fn ", "pub fn ", "async fn ", "pub async fn ", "fn "] {
        if let Some(rest) = trimmed.strip_prefix(prefix) {
            let name = rest.split(['(', '<', ' ', '{']).next().unwrap_or("").trim();
            if !name.is_empty() {
                return Some((SymbolKind::Function, name.to_string()));
            }
        }
    }
    for prefix in &["pub(crate) struct ", "pub struct ", "struct "] {
        if let Some(rest) = trimmed.strip_prefix(prefix) {
            let name = rest.split(['(', '<', ' ', '{']).next().unwrap_or("").trim();
            if !name.is_empty() {
                return Some((SymbolKind::Struct, name.to_string()));
            }
        }
    }
    for prefix in &["pub(crate) enum ", "pub enum ", "enum "] {
        if let Some(rest) = trimmed.strip_prefix(prefix) {
            let name = rest.split(['(', '<', ' ', '{']).next().unwrap_or("").trim();
            if !name.is_empty() {
                return Some((SymbolKind::Enum, name.to_string()));
            }
        }
    }
    for prefix in &["pub(crate) trait ", "pub trait ", "trait "] {
        if let Some(rest) = trimmed.strip_prefix(prefix) {
            let name = rest.split(['(', '<', ' ', '{']).next().unwrap_or("").trim();
            if !name.is_empty() {
                return Some((SymbolKind::Trait, name.to_string()));
            }
        }
    }
    for prefix in &["pub(crate) type ", "pub type ", "type "] {
        if let Some(rest) = trimmed.strip_prefix(prefix) {
            let name = rest.split(['(', '<', ' ', '=', '{']).next().unwrap_or("").trim();
            if !name.is_empty() {
                return Some((SymbolKind::Type, name.to_string()));
            }
        }
    }
    if let Some(rest) = trimmed.strip_prefix("impl ") {
        let name = rest
            .split(['{', '<'])
            .next()
            .unwrap_or("")
            .trim()
            .trim_end_matches(|c: char| c.is_whitespace());
        if !name.is_empty() {
            return Some((SymbolKind::Impl, name.to_string()));
        }
    }
    for prefix in &["pub(crate) mod ", "pub mod ", "mod "] {
        if let Some(rest) = trimmed.strip_prefix(prefix) {
            let name = rest.split([' ', '{', ';']).next().unwrap_or("").trim();
            if !name.is_empty() {
                return Some((SymbolKind::Module, name.to_string()));
            }
        }
    }
    for prefix in &["pub const ", "pub(crate) const ", "const "] {
        if let Some(rest) = trimmed.strip_prefix(prefix) {
            let name = rest.split([':', ' ']).next().unwrap_or("").trim();
            if !name.is_empty() {
                return Some((SymbolKind::Constant, name.to_string()));
            }
        }
    }
    // Python / JS / TypeScript patterns
    if let Some(rest) = trimmed.strip_prefix("def ") {
        let name = rest.split(['(', ' ']).next().unwrap_or("").trim();
        if !name.is_empty() {
            return Some((SymbolKind::Function, name.to_string()));
        }
    }
    if let Some(rest) = trimmed.strip_prefix("class ") {
        let name = rest.split(['(', ':', ' ', '{']).next().unwrap_or("").trim();
        if !name.is_empty() {
            return Some((SymbolKind::Class, name.to_string()));
        }
    }
    for prefix in &["export function ", "export async function ", "function ", "async function "] {
        if let Some(rest) = trimmed.strip_prefix(prefix) {
            let name = rest.split(['(', ' ']).next().unwrap_or("").trim();
            if !name.is_empty() {
                return Some((SymbolKind::Function, name.to_string()));
            }
        }
    }
    // Go patterns
    if let Some(rest) = trimmed.strip_prefix("func ") {
        let name = rest.split(['(', ' ']).next().unwrap_or("").trim();
        if !name.is_empty() {
            return Some((SymbolKind::Function, name.to_string()));
        }
    }
    if let Some(rest) = trimmed.strip_prefix("type ") {
        let name = rest.split([' ']).next().unwrap_or("").trim();
        if !name.is_empty() {
            return Some((SymbolKind::Type, name.to_string()));
        }
    }

    None
}

// Count leading spaces/tabs to estimate indent level.
fn indent_of(line: &str) -> u32 {
    let mut count = 0u32;
    for ch in line.chars() {
        match ch {
            ' ' => count += 1,
            '\t' => count += 4,
            _ => break,
        }
    }
    count / 4
}

fn load_symbols(path: &str) -> Result<Vec<Symbol>, String> {
    let output = Command::new("cat").arg(path).output()?;
    if output.status.unwrap_or(1) != 0 {
        return Err(String::from_utf8_lossy(&output.stderr).trim().to_string());
    }
    let content = String::from_utf8_lossy(&output.stdout);
    let mut symbols = Vec::new();
    for (idx, line) in content.lines().enumerate() {
        if let Some((kind, name)) = classify_line(line) {
            symbols.push(Symbol {
                name,
                kind,
                line: (idx as u32) + 1,
                indent: indent_of(line),
            });
        }
    }
    Ok(symbols)
}

fn rule() -> Div {
    div().w_full().h(ui::px(1.0)).bg(ui::color_border())
}

fn lang_label(path: &str) -> &str {
    if path.ends_with(".rs") {
        "Rust"
    } else if path.ends_with(".py") {
        "Python"
    } else if path.ends_with(".ts") || path.ends_with(".tsx") {
        "TypeScript"
    } else if path.ends_with(".js") || path.ends_with(".jsx") {
        "JavaScript"
    } else if path.ends_with(".go") {
        "Go"
    } else {
        "File"
    }
}

impl Extension for OutlinePanel {
    fn new() -> Self {
        register_command("open-outline", "Open Outline Panel (Demo)");
        Self {
            active_file: None,
            symbols: Vec::new(),
            state: PanelState::NoFile,
        }
    }

    fn gui_init(&mut self) {
        // active_file is delivered via pub-sub "zed.active-file-changed" after init.
    }

    fn on_pub_sub_event(&mut self, event: zed::pub_sub::PubSubEvent) {
        if event.topic == "zed.active-file-changed" {
            let path = if event.data.is_empty() { None } else { Some(event.data) };
            self.reload(path);
        }
    }

    fn gui_on_event(&mut self, source_id: String, _event: zed::gui::UiEvent) {
        if source_id == "refresh" {
            self.reload(self.active_file.clone());
        }
    }

    fn gui_render(&mut self) -> zed::ui_elements::UiTree {
        ui::clear_handlers();

        match &self.state {
            PanelState::NoFile => {
                return ui::render_tree(
                    v_flex()
                        .size_full()
                        .items_center()
                        .justify_center()
                        .gap_y(ui::def_px(8.0))
                        .child(Label::new("No file open").muted())
                        .child(Label::new("Open a file in the editor to see its outline").muted().text_xs()),
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

        let file_path = self.active_file.clone().unwrap_or_default();
        let file_name = file_path
            .rsplit('/')
            .next()
            .unwrap_or(&file_path)
            .to_string();
        let symbol_count = self.symbols.len();
        let lang = lang_label(&file_path).to_string();

        let symbol_rows: Vec<Div> = self
            .symbols
            .iter()
            .enumerate()
            .map(|(i, sym)| {
                let indent_px = (sym.indent * 12) as f32;
                h_flex()
                    .id(format!("sym-{i}"))
                    .px_2()
                    .py(ui::px(3.0))
                    .items_center()
                    .gap_x(ui::def_px(6.0))
                    .cursor_pointer()
                    .on_click(|_| {})
                    .child(div().w(ui::px(indent_px)))
                    .child(
                        div()
                            .px(ui::px(4.0))
                            .py(ui::px(1.0))
                            .rounded_sm()
                            .bg(ui::color_surface())
                            .child(
                                Label::new(sym.kind.label())
                                    .color(sym.kind.color())
                                    .text_xs()
                                    .bold(),
                            ),
                    )
                    .child(
                        div()
                            .flex_1()
                            .min_w(ui::def_px(0.0))
                            .overflow_x_hidden()
                            .child(Label::new(sym.name.clone()).text_sm().single_line()),
                    )
                    .child(
                        Label::new(format!("{}", sym.line))
                            .text_xs()
                            .muted(),
                    )
            })
            .collect();

        let mut root = v_flex()
            .size_full()
            .overflow_y_scroll()
            // ── Header ─────────────────────────────────────────────────────
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
                            .child(Label::new("◈").color(ui::color_accent()))
                            .child(Label::new(file_name).bold().text_sm()),
                    )
                    .child(
                        h_flex()
                            .gap_x(ui::def_px(6.0))
                            .items_center()
                            .child(Label::new(lang).muted().text_xs())
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
            // ── Symbol count ───────────────────────────────────────────────
            .child(
                div()
                    .px_3()
                    .py(ui::px(4.0))
                    .child(
                        Label::new(format!(
                            "{} symbol{}",
                            symbol_count,
                            if symbol_count == 1 { "" } else { "s" }
                        ))
                        .muted()
                        .text_xs(),
                    ),
            )
            .child(rule());

        if symbol_rows.is_empty() {
            root = root.child(
                div()
                    .px_3()
                    .py_3()
                    .child(Label::new("No symbols found").muted().text_xs().italic()),
            );
        } else {
            root = root.children(symbol_rows);
        }

        ui::render_tree(root)
    }

    fn run_extension_command(&mut self, command_id: &str) -> Result<(), String> {
        match command_id {
            "open-outline" => Ok(()),
            _ => Err(format!("unknown command: {command_id}")),
        }
    }
}

impl OutlinePanel {
    fn reload(&mut self, path: Option<String>) {
        self.active_file = path.clone();
        match path {
            None => {
                self.symbols.clear();
                self.state = PanelState::NoFile;
            }
            Some(p) => match load_symbols(&p) {
                Ok(symbols) => {
                    self.symbols = symbols;
                    self.state = PanelState::Ready;
                }
                Err(err) => {
                    self.symbols.clear();
                    self.state = PanelState::Error(err);
                }
            },
        }
    }
}

zed_extension_api::register_extension!(OutlinePanel);
