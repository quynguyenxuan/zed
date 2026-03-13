use zed_extension_api::{self as zed, Extension, register_command, ui};
use zed::ui::{Label, div, v_flex, h_flex};

struct GuiTest {
    count: u32,
}

impl Extension for GuiTest {
    fn new() -> Self {
        register_command("open-panel", "Open GUI Test Panel");
        GuiTest { count: 0 }
    }

    fn gui_on_event(&mut self, source_id: String, _event: zed::gui::UiEvent) {
        if source_id == "btn-increment" {
            self.count += 1;
        } else if source_id == "btn-reset" {
            self.count = 0;
        }
    }

    fn gui_render(&mut self) -> zed::ui_elements::UiTree {
        let count = self.count;
        ui::render_tree(
            v_flex()
                .p(ui::px(16.0))
                .gap_y(ui::def_px(12.0))
                .child(
                    Label::new("GUI Extension Demo")
                        .bold()
                        .size(ui::abs_px(18.0)),
                )
                .child(
                    Label::new(format!("Count: {count}"))
                        .color(if count == 0 {
                            ui::color_muted()
                        } else {
                            ui::color_accent()
                        }),
                )
                .child(
                    h_flex()
                        .gap_x(ui::def_px(8.0))
                        .child(
                            div()
                                .id("btn-increment")
                                .px(ui::px(12.0))
                                .py(ui::px(6.0))
                                .rounded_md()
                                .bg(ui::color_accent())
                                .text_color(ui::color_background())
                                .cursor_pointer()
                                .child(Label::new("Increment")),
                        )
                        .child(
                            div()
                                .id("btn-reset")
                                .px(ui::px(12.0))
                                .py(ui::px(6.0))
                                .rounded_md()
                                .border_1()
                                .border_color(ui::color_border())
                                .cursor_pointer()
                                .child(Label::new("Reset")),
                        ),
                ),
        )
    }

    fn run_extension_command(&mut self, command_id: &str) -> Result<(), String> {
        match command_id {
            "open-panel" => Ok(()),
            _ => Err(format!("unknown command: {command_id}")),
        }
    }
}

zed_extension_api::register_extension!(GuiTest);
