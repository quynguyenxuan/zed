use std::{ops::ControlFlow, sync::Arc};

use collections::HashMap;
use extension_host::{
    ExtensionManifest,
    wasm_host::{GuiPanelMessage, WasmExtension},
};
use futures::{StreamExt as _, channel::mpsc};
use gpui::{
    Action, AnyElement, App, Context, EventEmitter, FocusHandle, Focusable, IntoElement, Pixels,
    Render, SharedString, Task, WeakEntity, Window, actions, px,
};
use project::Project;
use serde::Deserialize;
use ui::{IconName, prelude::*};
use util::ResultExt as _;
use workspace::{
    DraggedTab, Pane, PaneGroup, PaneRenderContext, Workspace,
    dock::{DockPosition, Panel, PanelEvent},
    item::{Item, ItemEvent},
};

actions!(extension_panel, [ToggleFocus]);

/// A JSON-decodable element tree sent by a WASM GUI extension via `gui::set-view`.
#[derive(Clone, Debug, Deserialize)]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum ViewElement {
    VFlex { children: Vec<ViewElement> },
    HFlex { children: Vec<ViewElement> },
    Label { text: String },
    Button { label: Option<String> },
    Divider,
    #[serde(other)]
    Unknown,
}

pub struct ExtensionGuiView {
    extension_id: Arc<str>,
    focus_handle: FocusHandle,
    root_element: Option<ViewElement>,
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

        let message_task = cx.spawn(async move |this, cx| {
            wasm_extension
                .inject_gui_panel_tx(gui_tx)
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
            GuiPanelMessage::Emit { .. }
            | GuiPanelMessage::RequestData(_)
            | GuiPanelMessage::Call { .. } => {}
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
    fn render(&mut self, _window: &mut Window, _cx: &mut Context<Self>) -> impl IntoElement {
        let content: AnyElement = match &self.root_element {
            Some(root) => render_element(root),
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
    center: PaneGroup,
    workspace: WeakEntity<Workspace>,
    width: Option<Pixels>,
}

impl ExtensionGuiPanel {
    pub fn new(
        manifest: Arc<ExtensionManifest>,
        wasm_extension: WasmExtension,
        workspace: WeakEntity<Workspace>,
        project: gpui::Entity<Project>,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) -> Self {
        let pane = new_extension_pane(workspace.clone(), project, window, cx);
        let view = cx.new(|cx| ExtensionGuiView::new(manifest, wasm_extension, cx));
        pane.update(cx, |pane, cx| {
            pane.add_item(Box::new(view), true, true, None, window, cx);
        });

        Self {
            center: PaneGroup::new(pane.clone()),
            active_pane: pane,
            workspace,
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

        pane.set_custom_drop_handle(cx, move |pane, dropped_item, _window, cx| {
            if let Some(tab) = dropped_item.downcast_ref::<DraggedTab>() {
                let item = if tab.pane == cx.entity() {
                    pane.item_for_index(tab.ix)
                } else {
                    tab.pane.read(cx).item_for_index(tab.ix)
                };
                if let Some(item) = item {
                    if item.downcast::<ExtensionGuiView>().is_some() {
                        return ControlFlow::Continue(());
                    }
                }
            }
            ControlFlow::Break(())
        });

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
        for pane in self.center.panes() {
            pane.update(cx, |pane, cx| pane.set_zoomed(zoomed, cx));
        }
        cx.notify();
    }

    fn pane(&self) -> Option<gpui::Entity<Pane>> {
        Some(self.active_pane.clone())
    }

    fn icon_label(&self, _window: &Window, cx: &App) -> Option<String> {
        let count = self
            .center
            .panes()
            .into_iter()
            .map(|pane| pane.read(cx).items_len())
            .sum::<usize>();
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
    fn render(&mut self, window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        self.workspace
            .update(cx, |workspace, cx| {
                div().size_full().child(self.center.render(
                    workspace.zoomed_item(),
                    &PaneRenderContext {
                        follower_states: &HashMap::default(),
                        active_call: workspace.active_call(),
                        active_pane: &self.active_pane,
                        app_state: workspace.app_state(),
                        project: workspace.project(),
                        workspace: &workspace.weak_handle(),
                    },
                    window,
                    cx,
                ))
            })
            .unwrap_or_else(|_| div())
    }
}

fn render_element(element: &ViewElement) -> AnyElement {
    match element {
        ViewElement::VFlex { children } => gpui::div()
            .flex()
            .flex_col()
            .children(children.iter().map(render_element))
            .into_any_element(),
        ViewElement::HFlex { children } => gpui::div()
            .flex()
            .flex_row()
            .children(children.iter().map(render_element))
            .into_any_element(),
        ViewElement::Label { text } => gpui::div()
            .child(SharedString::from(text.clone()))
            .into_any_element(),
        ViewElement::Button { label } => gpui::div()
            .child(SharedString::from(label.clone().unwrap_or_default()))
            .into_any_element(),
        ViewElement::Divider => gpui::div().w_full().h(gpui::px(1.0)).into_any_element(),
        ViewElement::Unknown => gpui::div().into_any_element(),
    }
}
