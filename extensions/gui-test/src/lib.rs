use zed_extension_api::{Extension, register_command};

struct GuiTest;

impl Extension for GuiTest {
    fn new() -> Self {
        register_command("open-panel", "open panel");
        GuiTest
    }

    fn run_extension_command(&mut self, command_id: &str) -> Result<(), String> {
        match command_id {
            "open-panel" => Ok(()),
            _ => Err(format!("unknown command: {command_id}")),
        }
    }
}

zed_extension_api::register_extension!(GuiTest);
