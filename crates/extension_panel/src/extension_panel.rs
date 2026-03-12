use std::sync::Arc;

use anyhow::Result;
use extension_host::{
    ExtensionManifest,
    wasm_host::{GuiPanelMessage, WasmExtension},
};
use futures::{StreamExt as _, channel::mpsc};
use gpui::{
    Action, AnyElement, App, AsyncWindowContext, Context, EventEmitter, FocusHandle, Focusable,
    IntoElement, Pixels, Render, SharedString, Task, WeakEntity, Window, actions, px,
};
use project::Project;
use serde::{Deserialize, Serialize};
use ui::{IconName, prelude::*};
use util::ResultExt as _;
use workspace::{
    Pane, Workspace,
    dock::{DockPosition, Panel, PanelEvent},
    item::{Item, ItemEvent},
};

actions!(
    extension_panel,
    [
        /// Focuses the extension GUI panel.
        ToggleFocus
    ]
);

/// Opens a specific extension in the extension GUI panel by extension and command ID.
#[derive(Clone, Default, PartialEq, Eq, schemars::JsonSchema, gpui::Action, Serialize, Deserialize)]
pub struct OpenExtensionPanel {
    /// The extension ID to open.
    pub extension_id: String,
    /// The command ID that triggered the open.
    pub command_id: String,
}

/// A JSON-decodable element tree sent by a WASM GUI extension via `gui::set-view`.
#[derive(Clone, Debug, Deserialize)]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum ViewElement {
    VFlex { children: Vec<ViewElement> },
    HFlex { children: Vec<ViewElement> },
    Label { text: String },
    Button {
        #[serde(default)]
        source_id: Option<String>,
        label: Option<String>,
    },
    Divider,
    #[serde(other)]
    Unknown,
}

pub struct ExtensionGuiView {
    pub(crate) extension_id: Arc<str>,
    focus_handle: FocusHandle,
    root_element: Option<ViewElement>,
    wasm_extension: WasmExtension,
    _message_task: Task<()>,
}

impl ExtensionGuiView {
    pub fn new(
        manifest: Arc<ExtensionManifest>,
        wasm_extension: WasmExtension,
        cx: &mut Context<Self>,
    ) -> Self {
        let extension_id = manifest.id.clone();
        let (gui_tx, gui_rx) = mpsc::unbounded::<GuiPanelMessage>();
        let wasm_extension_for_task = wasm_extension.clone();

        let message_task = cx.spawn(async move |this, cx| {
            wasm_extension_for_task
                .inject_gui_panel_tx(gui_tx)
                .await
                .log_err();

            // Call gui_init so the extension renders its initial UI
            wasm_extension_for_task
                .call_gui_init()
                .await
                .log_err();

            let mut rx = gui_rx;
            while let Some(message) = rx.next().await {
                if this
                    .update(cx, |view, cx| view.handle_message(message, cx))
                    .is_err()
                {
                    break;
                }
            }
        });

        Self {
            extension_id,
            focus_handle: cx.focus_handle(),
            root_element: None,
            wasm_extension,
            _message_task: message_task,
        }
    }

    fn handle_message(&mut self, message: GuiPanelMessage, cx: &mut Context<Self>) {
        match message {
            GuiPanelMessage::SetView(json) => {
                match serde_json::from_str::<ViewElement>(&json) {
                    Ok(element) => {
                        self.root_element = Some(element);
                        cx.notify();
                    }
                    Err(error) => {
                        log::error!("extension_panel: failed to parse view JSON: {error}");
                    }
                }
            }
            GuiPanelMessage::Call { key, method, params: _ } => {
                let result = match method.as_str() {
                    "workspace.open_files" => "[]".to_string(),
                    "editor.get_selection" => "\"\"".to_string(),
                    _ => "{\"error\":\"unknown method\"}".to_string(),
                };
                let wasm_extension = self.wasm_extension.clone();
                cx.spawn(async move |_, _| {
                    wasm_extension.call_gui_on_data(key, result).await.log_err();
                }).detach();
            }
            GuiPanelMessage::RequestData(key) => {
                let wasm_extension = self.wasm_extension.clone();
                cx.spawn(async move |_, _| {
                    wasm_extension.call_gui_on_data(key, "null".to_string()).await.log_err();
                }).detach();
            }
            GuiPanelMessage::Emit { .. } => {}
        }
    }
}

impl Focusable for ExtensionGuiView {
    fn focus_handle(&self, _cx: &App) -> FocusHandle {
        self.focus_handle.clone()
    }
}

impl EventEmitter<ItemEvent> for ExtensionGuiView {}

impl Render for ExtensionGuiView {
    fn render(&mut self, _window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let content: AnyElement = match &self.root_element {
            Some(root) => render_element(root, cx.weak_entity()),
            None => gpui::div()
                .flex()
                .items_center()
                .justify_center()
                .size_full()
                .child(SharedString::from(format!(
                    "Loading extension: {}",
                    self.extension_id
                )))
                .into_any_element(),
        };
        gpui::div().size_full().child(content)
    }
}

impl Item for ExtensionGuiView {
    type Event = ItemEvent;

    fn tab_content_text(&self, _detail: usize, _cx: &App) -> SharedString {
        SharedString::from(self.extension_id.clone())
    }
}

pub struct ExtensionGuiPanel {
    active_pane: gpui::Entity<Pane>,
    width: Option<Pixels>,
}

impl ExtensionGuiPanel {
    pub async fn load(
        workspace: WeakEntity<Workspace>,
        mut cx: AsyncWindowContext,
    ) -> Result<gpui::Entity<Self>> {
        workspace.update_in(&mut cx, |workspace, window, cx| {
            let project = workspace.project().clone();
            let workspace_handle = workspace.weak_handle();
            cx.new(|cx| Self::empty(workspace_handle, project, window, cx))
        })
    }

    pub fn empty(
        workspace: WeakEntity<Workspace>,
        project: gpui::Entity<Project>,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) -> Self {
        let pane = new_extension_pane(workspace, project, window, cx);
        Self {
            active_pane: pane,
            width: None,
        }
    }

    pub fn add_view(
        &mut self,
        manifest: Arc<ExtensionManifest>,
        wasm_extension: WasmExtension,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        let view = cx.new(|cx| ExtensionGuiView::new(manifest, wasm_extension, cx));
        self.active_pane.update(cx, |pane, cx| {
            pane.add_item(Box::new(view), true, true, None, window, cx);
        });
    }

    pub fn open_or_focus(
        &mut self,
        extension_id: &str,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        let existing_ix = self
            .active_pane
            .read(cx)
            .items()
            .enumerate()
            .find_map(|(ix, item)| {
                item.downcast::<ExtensionGuiView>()
                    .filter(|view| view.read(cx).extension_id.as_ref() == extension_id)
                    .map(|_| ix)
            });
        if let Some(ix) = existing_ix {
            self.active_pane.update(cx, |pane, cx| {
                pane.activate_item(ix, true, true, window, cx);
            });
        }
    }
}

fn new_extension_pane(
    workspace: WeakEntity<Workspace>,
    project: gpui::Entity<Project>,
    window: &mut Window,
    cx: &mut Context<ExtensionGuiPanel>,
) -> gpui::Entity<Pane> {
    cx.new(|cx| {
        let mut pane = Pane::new(
            workspace.clone(),
            project.clone(),
            Default::default(),
            None,
            ToggleFocus.boxed_clone(),
            false,
            window,
            cx,
        );
        pane.set_can_navigate(false, cx);
        pane.display_nav_history_buttons(None);
        pane.set_should_display_tab_bar(|_, _| true);
        pane.set_zoom_out_on_close(false);

        // TODO: Re-enable custom drop handling when the API is available
        // pane.set_custom_drop_handle(cx, move |pane, dropped_item, _window, cx| {
        //     if let Some(tab) = dropped_item.downcast_ref::<DraggedTab>() {
        //         let item = if tab.pane == cx.entity() {
        //             pane.item_for_index(tab.ix)
        //         } else {
        //             tab.pane.read(cx).item_for_index(tab.ix)
        //         };
        //         if let Some(item) = item {
        //             if item.downcast::<ExtensionGuiView>().is_some() {
        //                 return ControlFlow::Continue(());
        //             }
        //         }
        //     }
        //     ControlFlow::Break(())
        // });

        pane
    })
}

impl Focusable for ExtensionGuiPanel {
    fn focus_handle(&self, cx: &App) -> FocusHandle {
        self.active_pane.focus_handle(cx)
    }
}

impl EventEmitter<PanelEvent> for ExtensionGuiPanel {}

impl Panel for ExtensionGuiPanel {
    fn persistent_name() -> &'static str {
        "ExtensionGuiPanel"
    }

    fn panel_key() -> &'static str {
        "ExtensionGuiPanel"
    }

    fn position(&self, _window: &Window, _cx: &App) -> DockPosition {
        DockPosition::Right
    }

    fn position_is_valid(&self, position: DockPosition) -> bool {
        matches!(position, DockPosition::Left | DockPosition::Right)
    }

    fn set_position(
        &mut self,
        _position: DockPosition,
        _window: &mut Window,
        _cx: &mut Context<Self>,
    ) {
    }

    fn size(&self, _window: &Window, _cx: &App) -> Pixels {
        self.width.unwrap_or(px(360.))
    }

    fn set_size(&mut self, size: Option<Pixels>, _window: &mut Window, cx: &mut Context<Self>) {
        self.width = size;
        cx.notify();
    }

    fn icon(&self, _window: &Window, _cx: &App) -> Option<IconName> {
        Some(IconName::Blocks)
    }

    fn icon_tooltip(&self, _window: &Window, _cx: &App) -> Option<&'static str> {
        Some("Extension Panel")
    }

    fn is_zoomed(&self, _window: &Window, cx: &App) -> bool {
        self.active_pane.read(cx).is_zoomed()
    }

    fn set_zoomed(&mut self, zoomed: bool, _window: &mut Window, cx: &mut Context<Self>) {
        self.active_pane.update(cx, |pane, cx| pane.set_zoomed(zoomed, cx));
        cx.notify();
    }

    fn pane(&self) -> Option<gpui::Entity<Pane>> {
        Some(self.active_pane.clone())
    }

    fn icon_label(&self, _window: &Window, cx: &App) -> Option<String> {
        let count = self.active_pane.read(cx).items_len();
        if count == 0 { None } else { Some(count.to_string()) }
    }

    fn activation_priority(&self) -> u32 {
        0
    }

    fn toggle_action(&self) -> Box<dyn gpui::Action> {
        Box::new(ToggleFocus)
    }
}

impl Render for ExtensionGuiPanel {
    fn render(&mut self, _window: &mut Window, _cx: &mut Context<Self>) -> impl IntoElement {
        div().size_full().child(self.active_pane.clone())
    }
}

fn render_element(element: &ViewElement, entity: WeakEntity<ExtensionGuiView>) -> AnyElement {
    match element {
        ViewElement::VFlex { children } => gpui::div()
            .flex()
            .flex_col()
            .children(children.iter().map(|c| render_element(c, entity.clone())))
            .into_any_element(),
        ViewElement::HFlex { children } => gpui::div()
            .flex()
            .flex_row()
            .children(children.iter().map(|c| render_element(c, entity.clone())))
            .into_any_element(),
        ViewElement::Label { text } => gpui::div()
            .child(SharedString::from(text.clone()))
            .into_any_element(),
        ViewElement::Button { source_id, label } => {
            let button_source_id = source_id.clone().or(label.clone()).unwrap_or_default();
            let button_label = label.clone().unwrap_or_default();
            let entity_for_click = entity.clone();

            ui::Button::new(
                SharedString::from(format!("ext-btn-{}", button_source_id)),
                button_label
            )
            .on_click(move |_, _window, cx| {
                let button_source_id = button_source_id.clone();
                entity_for_click.update(cx, |view, cx| {
                    let wasm_extension = view.wasm_extension.clone();
                    cx.spawn(async move |_, _| {
                        use extension_host::wasm_host::wit::since_v0_9_0::gui::UiEvent;
                        wasm_extension
                            .call_gui_on_event(button_source_id, UiEvent::Clicked)
                            .await
                            .log_err();
                    }).detach();
                }).log_err();
            })
            .into_any_element()
        }
        ViewElement::Divider => gpui::div().w_full().h(gpui::px(1.0)).into_any_element(),
        ViewElement::Unknown => gpui::div().into_any_element(),
    }
}
