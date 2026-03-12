use zed_extension_api::{Extension, gui, register_command, serde_json};

struct GuiTest {
    result_text: String,
}

impl GuiTest {
    fn render(&self) {
        let result = serde_json::to_string(&self.result_text).unwrap_or_default();
        let view = format!(
            r#"{{
                "type": "vflex",
                "children": [
                    {{ "type": "label", "text": "GUI Test Extension" }},
                    {{ "type": "divider" }},
                    {{
                        "type": "hflex",
                        "children": [
                            {{ "type": "button", "source-id": "btn-open-files", "label": "Get Open Files" }},
                            {{ "type": "button", "source-id": "btn-selection", "label": "Get Selection" }}
                        ]
                    }},
                    {{ "type": "divider" }},
                    {{ "type": "label", "text": {result} }}
                ]
            }}"#
        );
        gui::set_view(&view);
    }
}

impl Extension for GuiTest {
    fn new() -> Self {
        register_command("open-panel", "open panel");
        GuiTest {
            result_text: "Click a button to call a host action.".to_string(),
        }
    }

    fn gui_init(&mut self) {
        self.render();
    }

    fn gui_on_theme_change(&mut self, _theme: gui::Theme) {}

    fn gui_on_data(&mut self, key: String, value: String) {
        self.result_text = format!("[{key}] {value}");
        self.render();
    }

    fn run_extension_command(&mut self, command_id: &str) -> Result<(), String> {
        match command_id {
            "open-panel" => Ok(()),
            _ => Err(format!("unknown command: {command_id}")),
        }
    }

    fn gui_on_event(&mut self, source_id: String, event: gui::UiEvent) {
        if matches!(event, gui::UiEvent::Clicked) {
            let method = match source_id.as_str() {
                "btn-open-files" => "workspace.open_files",
                "btn-selection" => "editor.get_selection",
                _ => return,
            };
            if let Err(err) = gui::call(&source_id, method, "{}") {
                self.result_text = format!("error: {err}");
                self.render();
            }
        }
    }
}

zed_extension_api::register_extension!(GuiTest);
