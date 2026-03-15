mod ui_renderer;

use std::path::PathBuf;
use std::sync::{Arc, Mutex};

use anyhow::Result;
use command_palette_hooks::{DynamicCommand, GlobalDynamicCommandRegistry};
use extension_host::{
    ExtensionManifest, ExtensionStore,
    wasm_host::{ExtensionEventBus, PubSubEvent, QueryDelivery, QueryResponse, WasmExtension, wit, GuiPanelMessage},
};
use futures::StreamExt;
use gpui::{
    Action, App, AsyncWindowContext, Context, EventEmitter, FocusHandle, Focusable,
    IntoElement, Pixels, Render, Rgba, SharedString, WeakEntity, Window, actions, px,
};
use theme::ActiveTheme as _;
use project::{self, Project};
use serde::{Deserialize, Serialize};
use settings::SettingsStore;
use ui::{IconName, prelude::*};
use util::ResultExt as _;
use workspace::{
    Event as WorkspaceEvent, OpenOptions, Pane, ToggleZoom, Workspace,
    dock::{DockPosition, Panel, PanelEvent},
    item::{Item, ItemEvent},
};

fn hsla_to_wit(c: gpui::Hsla) -> wit::since_v0_9_0::gui::Color {
    let r: Rgba = c.into();
    wit::since_v0_9_0::gui::Color {
        r: (r.r.clamp(0.0, 1.0) * 255.0) as u8,
        g: (r.g.clamp(0.0, 1.0) * 255.0) as u8,
        b: (r.b.clamp(0.0, 1.0) * 255.0) as u8,
        a: (r.a.clamp(0.0, 1.0) * 255.0) as u8,
    }
}

fn current_wit_theme(cx: &App) -> wit::since_v0_9_0::gui::Theme {
    let theme = cx.theme();
    let c = theme.colors();
    let s = theme.status();
    wit::since_v0_9_0::gui::Theme {
        name: theme.name.to_string(),
        is_dark: theme.appearance == theme::Appearance::Dark,
        colors: wit::since_v0_9_0::gui::ThemeColors {
            background:                  hsla_to_wit(c.background),
            editor_background:           hsla_to_wit(c.editor_background),
            surface_background:          hsla_to_wit(c.surface_background),
            elevated_surface_background: hsla_to_wit(c.elevated_surface_background),
            text:                        hsla_to_wit(c.text),
            text_muted:                  hsla_to_wit(c.text_muted),
            text_disabled:               hsla_to_wit(c.text_disabled),
            text_accent:                 hsla_to_wit(c.text_accent),
            border:                      hsla_to_wit(c.border),
            border_muted:                hsla_to_wit(c.border_variant),
            border_focused:              hsla_to_wit(c.border_focused),
            element_background:          hsla_to_wit(c.element_background),
            element_hover:               hsla_to_wit(c.element_hover),
            element_selected:            hsla_to_wit(c.element_selected),
            element_active:              hsla_to_wit(c.element_active),
            element_disabled:            hsla_to_wit(c.element_disabled),
            panel_background:            hsla_to_wit(c.panel_background),
            status_error:                hsla_to_wit(s.error),
            status_warning:              hsla_to_wit(s.warning),
            status_success:              hsla_to_wit(s.success),
            status_info:                 hsla_to_wit(s.info),
        },
    }
}

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

/// Events emitted by extension GUI views to request workspace actions.
#[derive(Clone, Debug)]
pub enum ExtensionViewEvent {
    /// Request to open a file at the given path.
    OpenFile(PathBuf),
}

pub struct ExtensionGuiView {
    pub(crate) extension_id: Arc<str>,
    focus_handle: FocusHandle,
    wasm: WasmExtension,
    workspace: WeakEntity<Workspace>,
    ui_tree: Option<wit::since_v0_9_0::ui_elements::UiTree>,
    message_rx: Arc<Mutex<std::sync::mpsc::Receiver<GuiPanelMessage>>>,
    /// Maps WIT focus handle IDs to GPUI FocusHandles
    focus_handles: Arc<Mutex<std::collections::HashMap<u32, FocusHandle>>>,
}

impl ExtensionGuiView {
    pub fn new(
        manifest: Arc<ExtensionManifest>,
        wasm: WasmExtension,
        workspace: WeakEntity<Workspace>,
        cx: &mut Context<Self>,
    ) -> Self {
        // Create synchronous channel for GUI panel messages
        let (tx, rx) = std::sync::mpsc::channel();
        let message_rx = Arc::new(Mutex::new(rx));

        // Create async channel for command execution requests
        let (cmd_tx, mut cmd_rx) = futures::channel::mpsc::unbounded();

        // Create async channel for pub-sub event delivery (host → WASM)
        let (pub_sub_tx, pub_sub_rx) = futures::channel::mpsc::unbounded::<PubSubEvent>();

        // Create async channel for query delivery (host → WASM handler)
        let (query_tx, query_rx) = futures::channel::mpsc::unbounded::<QueryDelivery>();

        // Spawn task to handle command execution requests
        cx.spawn(async move |_this, cx| {
            while let Some(request) = cmd_rx.next().await {
                use extension_host::wasm_host::CommandExecutionRequest;
                match request {
                    CommandExecutionRequest::ExecuteCommand { command, args, response_tx } => {
                        let result: Result<String, String> = cx.update(
                            |cx| -> Result<String, String> {
                                let data = args.as_deref()
                                    .and_then(|s| serde_json::from_str(s).ok());
                                match cx.build_action(&command, data) {
                                    Ok(action) => {
                                        cx.dispatch_action(action.as_ref());
                                        Ok("null".to_string())
                                    }
                                    Err(e) => Err(format!("Unknown action '{}': {}", command, e)),
                                }
                            }
                        );
                        let _ = response_tx.send(result);
                    }
                    CommandExecutionRequest::ExecuteSlashCommand { command, args, response_tx } => {
                        let result: Result<String, String> = cx.update(
                            |_cx| -> Result<String, String> {
                                // Slash commands require agent/buffer context.
                                // Return command info as JSON for the extension to handle.
                                Ok(serde_json::json!({
                                    "command": command,
                                    "args": args,
                                }).to_string())
                            }
                        );
                        let _ = response_tx.send(result);
                    }
                    CommandExecutionRequest::PubSubSubscribe { topic, source_extension_id: _, event_tx, response_tx } => {
                        let id = cx.update(|cx| {
                            cx.default_global::<ExtensionEventBus>().subscribe(topic, event_tx)
                        });
                        let _ = response_tx.send(Ok(id));
                    }
                    CommandExecutionRequest::PubSubUnsubscribe { subscription_id, response_tx } => {
                        let result = cx.update(|cx| {
                            cx.default_global::<ExtensionEventBus>().unsubscribe(subscription_id)
                        });
                        let _ = response_tx.send(result);
                    }
                    CommandExecutionRequest::PubSubPublish { topic, source_extension_id, data, response_tx } => {
                        cx.update(|cx| {
                            cx.default_global::<ExtensionEventBus>()
                                .publish(&topic, source_extension_id.to_string(), data);
                        });
                        let _ = response_tx.send(Ok(()));
                    }
                    CommandExecutionRequest::QueryRegisterHandler { topic, source_extension_id, query_tx: handler_tx, response_tx } => {
                        let id = cx.update(|cx| {
                            cx.default_global::<ExtensionEventBus>()
                                .register_query_handler(topic, source_extension_id, handler_tx)
                        });
                        let _ = response_tx.send(Ok(id));
                    }
                    CommandExecutionRequest::QueryUnregisterHandler { handler_id, response_tx } => {
                        let result = cx.update(|cx| {
                            cx.default_global::<ExtensionEventBus>()
                                .unregister_query_handler(handler_id)
                        });
                        let _ = response_tx.send(result);
                    }
                    CommandExecutionRequest::QueryRequest { topic, source_extension_id, data, timeout_ms, response_tx } => {
                        let host_response: Option<QueryResponse> = match topic.as_str() {
                            "zed.project-root" => _this.upgrade().and_then(|view| {
                                cx.read_entity(&view, |view, cx| {
                                    view.workspace.upgrade().and_then(|workspace| {
                                        workspace.read(cx)
                                            .project()
                                            .read(cx)
                                            .worktrees(cx)
                                            .next()
                                            .map(|wt| wt.read(cx).abs_path().to_string_lossy().into_owned())
                                    })
                                })
                            }).map(|path| QueryResponse {
                                source: "zed".to_string(),
                                data: serde_json::Value::String(path).to_string(),
                            }),
                            "zed.active-file" => _this.upgrade().and_then(|view| {
                                cx.read_entity(&view, |view, cx| {
                                    view.workspace.upgrade().and_then(|workspace| {
                                        workspace.read(cx)
                                            .active_item(cx)
                                            .and_then(|item| item.project_path(cx))
                                            .and_then(|pp| {
                                                workspace.read(cx)
                                                    .project()
                                                    .read(cx)
                                                    .absolute_path(&pp, cx)
                                                    .map(|path| path.to_string_lossy().into_owned())
                                            })
                                    })
                                })
                            }).map(|path| QueryResponse {
                                source: "zed".to_string(),
                                data: serde_json::Value::String(path).to_string(),
                            }),
                            _ => None,
                        };
                        let handlers = cx.update(|cx| {
                            cx.default_global::<ExtensionEventBus>().get_query_handlers(&topic)
                        });
                        let source = source_extension_id.to_string();
                        let mut handler_responses = Vec::new();
                        for (i, (ext_id, handler_tx)) in handlers.into_iter().enumerate() {
                            let (resp_tx, resp_rx) = futures::channel::oneshot::channel::<Result<String, String>>();
                            let delivery = QueryDelivery {
                                query_id: i as u64,
                                topic: topic.clone(),
                                source: source.clone(),
                                data: data.clone(),
                                response_tx: resp_tx,
                            };
                            handler_tx.unbounded_send(delivery).ok();
                            handler_responses.push((ext_id, resp_rx));
                        }
                        let executor = cx.update(|cx| cx.background_executor().clone());
                        let timeout_dur = std::time::Duration::from_millis(timeout_ms as u64);
                        let all_fut = Box::pin(futures::future::join_all(
                            handler_responses.into_iter().map(|(ext_id, rx)| async move {
                                rx.await.ok().and_then(|r| r.ok()).map(|d| QueryResponse {
                                    source: ext_id.to_string(),
                                    data: d,
                                })
                            }),
                        ));
                        let timer_fut = Box::pin(executor.timer(timeout_dur));
                        let mut collected = match futures::future::select(all_fut, timer_fut).await {
                            futures::future::Either::Left((results, _)) => {
                                results.into_iter().flatten().collect::<Vec<QueryResponse>>()
                            }
                            futures::future::Either::Right(_) => Vec::new(),
                        };
                        if let Some(host_resp) = host_response {
                            collected.insert(0, host_resp);
                        }
                        let _ = response_tx.send(Ok(collected));
                    }
                }
            }
        })
        .detach();

        // Spawn pub-sub delivery task: receives events from the bus, calls the WASM export,
        // then re-renders so the panel reflects any state changes.
        let wasm_for_delivery = wasm.clone();
        cx.spawn(async move |this, cx| {
            let mut rx = pub_sub_rx;
            while let Some(event) = rx.next().await {
                let wit_event = wit::since_v0_9_0::pub_sub::PubSubEvent {
                    topic: event.topic,
                    source: event.source,
                    data: event.data,
                };
                wasm_for_delivery.call_on_pub_sub_event(wit_event).await.log_err();
                match wasm_for_delivery.call_gui_render().await {
                    Ok(tree) => {
                        this.update(cx, |view, cx| {
                            view.ui_tree = Some(tree);
                            cx.notify();
                        })
                        .log_err();
                    }
                    Err(err) => log::error!("gui_render failed after pub-sub: {err}"),
                }
            }
        })
        .detach();

        // Spawn query delivery task: receives queries from the bus and calls the WASM handler export
        let wasm_for_query = wasm.clone();
        cx.spawn(async move |_, _| {
            let mut query_rx = query_rx;
            while let Some(delivery) = query_rx.next().await {
                let result = wasm_for_query
                    .call_on_query(delivery.query_id, delivery.topic, delivery.source, delivery.data)
                    .await
                    .unwrap_or_else(|err| Err(err.to_string()));
                delivery.response_tx.send(result).ok();
            }
        })
        .detach();

        // Observe settings store to re-deliver theme whenever it changes.
        cx.observe_global::<SettingsStore>(|this, cx| {
            let theme = current_wit_theme(cx);
            let wasm = this.wasm.clone();
            cx.spawn(async move |_, _| {
                wasm.call_gui_on_theme_change(theme).await.log_err();
            })
            .detach();
        })
        .detach();

        let wasm_for_init = wasm.clone();
        cx.spawn(async move |this, cx| {
            // Set up message channel
            if let Err(err) = wasm_for_init.inject_gui_panel_tx(tx).await {
                log::error!("inject_gui_panel_tx failed: {err}");
            }

            // Set up command execution channel
            if let Err(err) = wasm_for_init.inject_command_execution_tx(cmd_tx).await {
                log::error!("inject_command_execution_tx failed: {err}");
            }

            // Set up pub-sub event delivery channel
            if let Err(err) = wasm_for_init.inject_pub_sub_event_tx(pub_sub_tx).await {
                log::error!("inject_pub_sub_event_tx failed: {err}");
            }

            // Set up query delivery channel
            if let Err(err) = wasm_for_init.inject_query_tx(query_tx).await {
                log::error!("inject_query_tx failed: {err}");
            }

            log::info!("extension gui: calling gui_init");
            if let Err(err) = wasm_for_init.call_gui_init().await {
                log::error!("gui_init failed: {err}");
            }

            // Deliver the initial theme immediately after init.
            let theme = cx.update(|cx| current_wit_theme(cx));
            wasm_for_init.call_gui_on_theme_change(theme).await.log_err();

            // Deliver initial workspace context via pub-sub directly to this WASM instance.
            // The extension auto-subscribed during gui_init, so on_pub_sub_event will fire.
            let (open_files_json, project_root, active_file) = this
                .upgrade()
                .and_then(|view| {
                    cx.read_entity(&view, |view, cx| {
                        view.workspace.upgrade().map(|workspace| {
                            let project = workspace.read(cx).project().clone();
                            let open_files: Vec<String> = workspace
                                .read(cx)
                                .items(cx)
                                .filter_map(|item| item.project_path(cx))
                                .filter_map(|pp| {
                                    project
                                        .read(cx)
                                        .absolute_path(&pp, cx)
                                        .map(|p| p.to_string_lossy().into_owned())
                                })
                                .collect();
                            let project_root = project
                                .read(cx)
                                .worktrees(cx)
                                .next()
                                .map(|wt| wt.read(cx).abs_path().to_string_lossy().into_owned())
                                .unwrap_or_default();
                            let active_file = workspace
                                .read(cx)
                                .active_item(cx)
                                .and_then(|item| item.project_path(cx))
                                .and_then(|pp| {
                                    project
                                        .read(cx)
                                        .absolute_path(&pp, cx)
                                        .map(|p| p.to_string_lossy().into_owned())
                                })
                                .unwrap_or_default();
                            let open_files_json =
                                serde_json::to_string(&open_files).unwrap_or_default();
                            (open_files_json, project_root, active_file)
                        })
                    })
                })
                .unwrap_or_default();
            for (topic, data) in [
                ("zed.project-root-changed", project_root),
                ("zed.active-file-changed", active_file),
                ("zed.open-files-changed", open_files_json),
            ] {
                let evt = wit::since_v0_9_0::pub_sub::PubSubEvent {
                    topic: topic.to_string(),
                    source: "zed".to_string(),
                    data,
                };
                wasm_for_init.call_on_pub_sub_event(evt).await.log_err();
            }

            log::info!("extension gui: calling gui_render");
            match wasm_for_init.call_gui_render().await {
                Ok(tree) => {
                    log::info!(
                        "extension gui: gui_render ok, nodes={}, root={}",
                        tree.nodes.len(),
                        tree.root
                    );
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
            workspace,
            ui_tree: None,
            message_rx,
            focus_handles: Arc::new(Mutex::new(std::collections::HashMap::new())),
        }
    }

    /// Ensure FocusHandles exist for all focusable elements in the tree
    fn ensure_focus_handles(&mut self, tree: &wit::since_v0_9_0::ui_elements::UiTree, cx: &mut Context<Self>) {
        use wit::since_v0_9_0::ui_elements::UiNode;

        let mut handles = self.focus_handles.lock().unwrap();

        // Recursively walk the tree and create FocusHandles for any elements that need them
        for node in &tree.nodes {
            if let UiNode::Div(div_node) = node {
                if let Some(handle_id) = div_node.focus_handle_id {
                    // Create a FocusHandle if we don't have one yet
                    handles.entry(handle_id).or_insert_with(|| cx.focus_handle());
                }
            }
        }
    }

    fn process_pending_messages(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        // Process all pending messages without blocking
        if let Ok(rx) = self.message_rx.lock() {
            while let Ok(msg) = rx.try_recv() {
                match msg {
                    GuiPanelMessage::RequestFocus(handle_id) => {
                        // Focus the element with the given handle ID
                        if let Ok(handles) = self.focus_handles.lock() {
                            if let Some(focus_handle) = handles.get(&handle_id) {
                                window.focus(focus_handle, cx);
                            }
                        }
                    }
                    GuiPanelMessage::Call { method, params, .. } => {
                        if method == "open_file" {
                            #[derive(Deserialize)]
                            struct OpenFileParams {
                                path: String,
                            }

                            if let Ok(data) = serde_json::from_str::<OpenFileParams>(&params) {
                                let path = PathBuf::from(data.path);
                                cx.emit(ExtensionViewEvent::OpenFile(path));
                            }
                        }
                    }
                    _ => {}
                }
            }
        }
    }
}

impl Focusable for ExtensionGuiView {
    fn focus_handle(&self, _cx: &App) -> FocusHandle {
        self.focus_handle.clone()
    }
}

impl EventEmitter<ItemEvent> for ExtensionGuiView {}
impl EventEmitter<ExtensionViewEvent> for ExtensionGuiView {}

impl Render for ExtensionGuiView {
    fn render(&mut self, window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        // Process any pending messages and emit events
        self.process_pending_messages(window, cx);

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

                // Create focus handles for any new focusable elements in the tree
                self.ensure_focus_handles(&tree, cx);

                let focus_handles = self.focus_handles.clone();

                div()
                    .size_full()
                    .track_focus(&self.focus_handle)
                    .on_mouse_down(gpui::MouseButton::Left, cx.listener(move |this, _, window, cx| {
                        // Capture focus when clicking anywhere in extension panel
                        window.focus(&this.focus_handle, cx);
                    }))
                    .child(ui_renderer::render_ui_tree(&tree, on_event, focus_handles, window, cx))
                    .into_any_element()
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
    workspace: WeakEntity<Workspace>,
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
        let pane = new_extension_pane(workspace.clone(), project, window, cx);
        Self {
            active_pane: pane,
            workspace,
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
    ) -> gpui::Entity<ExtensionGuiView> {
        let workspace = self.workspace.clone();
        let view = cx.new(|cx| ExtensionGuiView::new(manifest, wasm, workspace, cx));
        self.active_pane.update(cx, |pane, cx| {
            pane.add_item(Box::new(view.clone()), true, true, None, window, cx);
        });
        view
    }

    /// Opens the extension in a new tab, or focuses the existing tab if already open.
    /// Emits `PanelEvent::Activate` to make the panel visible.
    pub fn open_or_focus(
        &mut self,
        manifest: Arc<ExtensionManifest>,
        wasm: WasmExtension,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) -> Option<gpui::Entity<ExtensionGuiView>> {
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
        let view = if let Some(ix) = existing_ix {
            self.active_pane.update(cx, |pane, cx| {
                pane.activate_item(ix, true, true, window, cx);
            });
            None
        } else {
            Some(self.add_view(manifest, wasm, window, cx))
        };
        cx.emit(PanelEvent::Activate);
        view
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

        // Custom tab bar buttons: only show zoom button when focused
        pane.set_render_tab_bar_buttons(cx, |pane, window, cx| {
            // Only show buttons when pane has focus
            if !pane.has_focus(window, cx) && !pane.context_menu_focused(window, cx) {
                return (None, None);
            }

            let zoomed = pane.is_zoomed();
            let zoom_button = ui::IconButton::new("toggle_zoom", ui::IconName::Maximize)
                .icon_size(ui::IconSize::Small)
                .toggle_state(zoomed)
                .selected_icon(ui::IconName::Minimize)
                .on_click(cx.listener(|pane, _, window, cx| {
                    pane.toggle_zoom(&ToggleZoom, window, cx);
                }))
                .tooltip(move |_window, cx| {
                    ui::Tooltip::for_action(
                        if zoomed { "Zoom Out" } else { "Zoom In" },
                        &ToggleZoom,
                        cx,
                    )
                })
                .into_any_element()
                .into();
            (None, zoom_button)
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

                            // Open/focus panel and subscribe to events
                            let view = panel.update(cx, |panel, cx| {
                                panel.open_or_focus(manifest, wasm_extension, window, cx)
                            });

                            // Subscribe to file open events if a new view was created
                            if let Some(view) = view {
                                cx.subscribe_in(
                                    &view,
                                    window,
                                    |workspace: &mut Workspace, _view, event: &ExtensionViewEvent, window, cx| {
                                        match event {
                                            ExtensionViewEvent::OpenFile(path) => {
                                                workspace.open_abs_path(
                                                    path.clone(),
                                                    OpenOptions::default(),
                                                    window,
                                                    cx,
                                                ).detach();
                                            }
                                        }
                                    },
                                )
                                .detach();
                            }

                            // Run command asynchronously
                            cx.spawn_in(window, async move |_, _| {
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

            // Push workspace change events to the extension event bus so that
            // extensions relying on the pub-sub cache get zero-latency reads.
            let workspace_entity = cx.entity();
            cx.subscribe_in(
                &workspace_entity,
                window,
                |workspace, _entity, event: &WorkspaceEvent, _window, cx| {
                    if let WorkspaceEvent::ActiveItemChanged = event {
                        let active_file = workspace
                            .active_item(cx)
                            .and_then(|item| item.project_path(cx))
                            .and_then(|pp| {
                                workspace
                                    .project()
                                    .read(cx)
                                    .absolute_path(&pp, cx)
                                    .map(|p| p.to_string_lossy().into_owned())
                            })
                            .unwrap_or_default();
                        let project_root = workspace
                            .project()
                            .read(cx)
                            .worktrees(cx)
                            .next()
                            .map(|wt| wt.read(cx).abs_path().to_string_lossy().into_owned())
                            .unwrap_or_default();
                        let open_files: Vec<String> = workspace
                            .items(cx)
                            .filter_map(|item| item.project_path(cx))
                            .filter_map(|pp| {
                                workspace
                                    .project()
                                    .read(cx)
                                    .absolute_path(&pp, cx)
                                    .map(|p| p.to_string_lossy().into_owned())
                            })
                            .collect();

                        // Notify reactive extensions via pub-sub.
                        // Each view's delivery task re-renders after receiving the event.
                        // Use default_global to avoid panicking when no extension panel is open yet.
                        let bus = cx.default_global::<ExtensionEventBus>();
                        bus.publish("zed.active-file-changed", "zed".to_string(), active_file);
                        bus.publish("zed.project-root-changed", "zed".to_string(), project_root);
                        bus.publish(
                            "zed.open-files-changed",
                            "zed".to_string(),
                            serde_json::to_string(&open_files).unwrap_or_default(),
                        );
                    }
                },
            )
            .detach();

            let extension_store = ExtensionStore::global(cx);
            cx.subscribe_in(
                &extension_store,
                window,
                |workspace, _, event, window, cx| match event {
                    extension_host::Event::GuiExtensionLoaded(manifest, wasm_extension) => {
                        if let Some(panel) = workspace.panel::<ExtensionGuiPanel>(cx) {
                            let manifest = manifest.clone();
                            let wasm = wasm_extension.clone();

                            // Add view and subscribe to events
                            let view = panel.update(cx, |panel, cx| {
                                panel.add_view(manifest, wasm, window, cx)
                            });

                            cx.subscribe_in(
                                &view,
                                window,
                                |workspace: &mut Workspace, _view, event: &ExtensionViewEvent, window, cx| {
                                    match event {
                                        ExtensionViewEvent::OpenFile(path) => {
                                            workspace.open_abs_path(
                                                path.clone(),
                                                OpenOptions::default(),
                                                window,
                                                cx,
                                            ).detach();
                                        }
                                    }
                                },
                            )
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
