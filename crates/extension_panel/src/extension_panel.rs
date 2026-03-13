mod ui_renderer;

use std::sync::Arc;

use anyhow::Result;
use command_palette_hooks::{DynamicCommand, GlobalDynamicCommandRegistry};
use extension_host::{
    ExtensionManifest, ExtensionStore,
    wasm_host::{WasmExtension, wit},
};
use gpui::{
    Action, App, AsyncWindowContext, Context, EventEmitter, FocusHandle, Focusable,
    IntoElement, Pixels, Render, SharedString, WeakEntity, Window, actions, px,
};
use project::Project;
use serde::{Deserialize, Serialize};
use settings::SettingsStore;
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

pub struct ExtensionGuiView {
    pub(crate) extension_id: Arc<str>,
    focus_handle: FocusHandle,
    wasm: WasmExtension,
    ui_tree: Option<wit::since_v0_9_0::ui_elements::UiTree>,
}

impl ExtensionGuiView {
    pub fn new(
        manifest: Arc<ExtensionManifest>,
        wasm: WasmExtension,
        cx: &mut Context<Self>,
    ) -> Self {
        let wasm_for_init = wasm.clone();
        cx.spawn(async move |this, cx| {
            wasm_for_init.call_gui_init().await.log_err();
            match wasm_for_init.call_gui_render().await {
                Ok(tree) => {
                    this.update(cx, |view, cx| {
                        view.ui_tree = Some(tree);
                        cx.notify();
                    })
                    .log_err();
                }
                Err(err) => log::error!("gui_render failed after init: {err}"),
            }
        })
        .detach();
        Self {
            extension_id: manifest.id.clone(),
            focus_handle: cx.focus_handle(),
            wasm,
            ui_tree: None,
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
        let wasm = self.wasm.clone();
        let entity = cx.entity().downgrade();
        let on_event = move |source_id: String,
                             event: wit::since_v0_9_0::gui::UiEvent,
                             _window: &mut Window,
                             cx: &mut App| {
            let wasm = wasm.clone();
            let entity = entity.clone();
            cx.spawn(async move |cx| {
                wasm.call_gui_on_event(source_id, event).await.log_err();
                match wasm.call_gui_render().await {
                    Ok(tree) => {
                        entity
                            .update(cx, |view, cx| {
                                view.ui_tree = Some(tree);
                                cx.notify();
                            })
                            .log_err();
                    }
                    Err(err) => log::error!("gui_render failed: {err}"),
                }
            })
            .detach();
        };
        match &self.ui_tree {
            Some(tree) => {
                let tree = tree.clone();
                ui_renderer::render_ui_tree(&tree, on_event, cx).into_any_element()
            }
            None => div()
                .size_full()
                .child(Label::new("Loading…"))
                .into_any_element(),
        }
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
    position: DockPosition,
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
            position: DockPosition::Left,
        }
    }

    pub fn add_view(
        &mut self,
        manifest: Arc<ExtensionManifest>,
        wasm: WasmExtension,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        let view = cx.new(|cx| ExtensionGuiView::new(manifest, wasm, cx));
        self.active_pane.update(cx, |pane, cx| {
            pane.add_item(Box::new(view), true, true, None, window, cx);
        });
    }

    /// Opens the extension in a new tab, or focuses the existing tab if already open.
    /// Emits `PanelEvent::Activate` to make the panel visible.
    pub fn open_or_focus(
        &mut self,
        manifest: Arc<ExtensionManifest>,
        wasm: WasmExtension,
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
                    .filter(|view| view.read(cx).extension_id == manifest.id)
                    .map(|_| ix)
            });
        if let Some(ix) = existing_ix {
            self.active_pane.update(cx, |pane, cx| {
                pane.activate_item(ix, true, true, window, cx);
            });
        } else {
            self.add_view(manifest, wasm, window, cx);
        }
        cx.emit(PanelEvent::Activate);
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
        self.position
    }

    fn position_is_valid(&self, position: DockPosition) -> bool {
        matches!(position, DockPosition::Left | DockPosition::Right)
    }

    fn set_position(
        &mut self,
        position: DockPosition,
        _window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        self.position = position;
        cx.notify();
        // Trigger SettingsStore observers so the dock repositions this panel.
        cx.update_global::<SettingsStore, _>(|_, _| {});
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
        7
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


/// Registers `ExtensionGuiPanel` actions and subscribes to `ExtensionStore` events so that:
/// - Commands registered by WASM extensions appear in the command palette.
/// - GUI extensions get a tab added when they load.
/// - `OpenExtensionPanel` dispatches open the correct extension tab and invoke the WASM handler.
pub fn init(cx: &mut App) {
    cx.observe_new(
        |workspace: &mut Workspace, window: Option<&mut Window>, cx: &mut Context<Workspace>| {
            workspace
                .register_action(|workspace, _: &ToggleFocus, window, cx| {
                    workspace.toggle_panel_focus::<ExtensionGuiPanel>(window, cx);
                })
                .register_action(|workspace, action: &OpenExtensionPanel, window, cx| {
                    let extension_id = action.extension_id.clone();
                    let command_id = action.command_id.clone();
                    let wasm = ExtensionStore::global(cx)
                        .read(cx)
                        .wasm_extension_for_id(&extension_id);
                    if let Some(panel) = workspace.panel::<ExtensionGuiPanel>(cx) {
                        if let Some((manifest, wasm_extension)) = wasm {
                            let wasm_for_cmd = wasm_extension.clone();
                            cx.spawn_in(window, async move |_, cx| {
                                panel
                                    .update_in(cx, |panel, window, cx| {
                                        panel.open_or_focus(
                                            manifest,
                                            wasm_extension,
                                            window,
                                            cx,
                                        );
                                    })
                                    .ok();
                                wasm_for_cmd
                                    .call_run_extension_command(command_id)
                                    .await
                                    .log_err();
                            })
                            .detach();
                        } else {
                            workspace.toggle_panel_focus::<ExtensionGuiPanel>(window, cx);
                        }
                    }
                });

            let Some(window) = window else { return };
            let extension_store = ExtensionStore::global(cx);
            cx.subscribe_in(
                &extension_store,
                window,
                |workspace, _, event, window, cx| match event {
                    extension_host::Event::GuiExtensionLoaded(manifest, wasm_extension) => {
                        if let Some(panel) = workspace.panel::<ExtensionGuiPanel>(cx) {
                            let manifest = manifest.clone();
                            let wasm = wasm_extension.clone();
                            cx.spawn_in(window, async move |_, cx| {
                                panel
                                    .update_in(cx, |panel, window, cx| {
                                        panel.add_view(manifest, wasm, window, cx);
                                    })
                                    .ok();
                            })
                            .detach();
                        }
                    }
                    extension_host::Event::ExtensionCommandRegistered {
                        extension_id,
                        display_name,
                        command_id,
                    } => {
                        cx.update_global(|registry: &mut GlobalDynamicCommandRegistry, _| {
                            registry.0.register(DynamicCommand {
                                name: display_name.clone(),
                                extension_id: extension_id.clone(),
                                command_id: command_id.clone(),
                            });
                        });
                    }
                    _ => {}
                },
            )
            .detach();
        },
    )
    .detach();
}