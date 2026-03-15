use crate::wasm_host::WasmState;
use anyhow::Result;
use extension::WorktreeDelegate;
use gpui::BackgroundExecutor;
use semver::Version;
use std::sync::{Arc, OnceLock};
use wasmtime::component::{Linker, Resource};

use super::latest;

pub const MIN_VERSION: Version = Version::new(0, 9, 0);
pub const MAX_VERSION: Version = Version::new(0, 9, 0);

wasmtime::component::bindgen!({
    async: true,
    trappable_imports: true,
    path: "../extension_api/wit/since_v0.9.0",
    world: "panel-ui",
    with: {
        "worktree": super::since_v0_8_0::ExtensionWorktree,
        "project": super::since_v0_8_0::ExtensionProject,
        "key-value-store": super::since_v0_8_0::ExtensionKeyValueStore,
        "zed:extension/common": super::since_v0_8_0::common,
        "zed:extension/lsp": super::since_v0_8_0::lsp,
        "zed:extension/http-client": super::since_v0_8_0::http_client,
        "zed:extension/github": super::since_v0_8_0::github,
        "zed:extension/process": super::since_v0_8_0::process,
        "zed:extension/platform": super::since_v0_8_0::platform,
        "zed:extension/nodejs": super::since_v0_8_0::nodejs,
        "zed:extension/context-server": super::since_v0_8_0::context_server,
        "zed:extension/dap": super::since_v0_8_0::dap,
        "zed:extension/slash-command": super::since_v0_8_0::slash_command,
    }
});

pub use self::zed::extension::*;

impl From<CodeLabel> for latest::CodeLabel {
    fn from(value: CodeLabel) -> Self {
        Self {
            code: value.code,
            spans: value.spans.into_iter().map(Into::into).collect(),
            filter_range: value.filter_range,
        }
    }
}

impl From<CodeLabelSpan> for latest::CodeLabelSpan {
    fn from(value: CodeLabelSpan) -> Self {
        match value {
            CodeLabelSpan::CodeRange(range) => Self::CodeRange(range),
            CodeLabelSpan::Literal(literal) => Self::Literal(literal.into()),
        }
    }
}

impl From<CodeLabelSpanLiteral> for latest::CodeLabelSpanLiteral {
    fn from(value: CodeLabelSpanLiteral) -> Self {
        Self {
            text: value.text,
            highlight_name: value.highlight_name,
        }
    }
}

impl HostKeyValueStore for WasmState {
    async fn insert(
        &mut self,
        kv_store: Resource<super::since_v0_8_0::ExtensionKeyValueStore>,
        key: String,
        value: String,
    ) -> wasmtime::Result<Result<(), String>> {
        latest::HostKeyValueStore::insert(self, kv_store, key, value).await
    }

    async fn drop(
        &mut self,
        _kv_store: Resource<super::since_v0_8_0::ExtensionKeyValueStore>,
    ) -> Result<()> {
        Ok(())
    }
}

impl HostProject for WasmState {
    async fn worktree_ids(
        &mut self,
        project: Resource<super::since_v0_8_0::ExtensionProject>,
    ) -> wasmtime::Result<Vec<u64>> {
        latest::HostProject::worktree_ids(self, project).await
    }

    async fn drop(
        &mut self,
        _project: Resource<super::since_v0_8_0::ExtensionProject>,
    ) -> Result<()> {
        Ok(())
    }
}

impl HostWorktree for WasmState {
    async fn id(
        &mut self,
        delegate: Resource<Arc<dyn WorktreeDelegate>>,
    ) -> wasmtime::Result<u64> {
        latest::HostWorktree::id(self, delegate).await
    }

    async fn root_path(
        &mut self,
        delegate: Resource<Arc<dyn WorktreeDelegate>>,
    ) -> wasmtime::Result<String> {
        latest::HostWorktree::root_path(self, delegate).await
    }

    async fn read_text_file(
        &mut self,
        delegate: Resource<Arc<dyn WorktreeDelegate>>,
        path: String,
    ) -> wasmtime::Result<Result<String, String>> {
        latest::HostWorktree::read_text_file(self, delegate, path).await
    }

    async fn shell_env(
        &mut self,
        delegate: Resource<Arc<dyn WorktreeDelegate>>,
    ) -> wasmtime::Result<EnvVars> {
        latest::HostWorktree::shell_env(self, delegate).await
    }

    async fn which(
        &mut self,
        delegate: Resource<Arc<dyn WorktreeDelegate>>,
        binary_name: String,
    ) -> wasmtime::Result<Option<String>> {
        latest::HostWorktree::which(self, delegate, binary_name).await
    }

    async fn drop(
        &mut self,
        _worktree: Resource<Arc<dyn WorktreeDelegate>>,
    ) -> Result<()> {
        Ok(())
    }
}

mod settings {
    #![allow(dead_code)]
    include!(concat!(env!("OUT_DIR"), "/since_v0.9.0/settings.rs"));
}

pub fn linker_with_gui(executor: &BackgroundExecutor) -> &'static Linker<WasmState> {
    static LINKER: OnceLock<Linker<WasmState>> = OnceLock::new();
    LINKER.get_or_init(|| super::new_linker(executor, PanelUi::add_to_linker))
}

impl PanelUiImports for WasmState {
    async fn get_settings(
        &mut self,
        location: Option<SettingsLocation>,
        category: String,
        key: Option<String>,
    ) -> wasmtime::Result<Result<String, String>> {
        let location = location.map(|loc| super::since_v0_8_0::SettingsLocation {
            worktree_id: loc.worktree_id,
            path: loc.path,
        });
        <WasmState as super::since_v0_8_0::ExtensionImports>::get_settings(
            self, location, category, key,
        )
        .await
    }

    async fn set_language_server_installation_status(
        &mut self,
        server_name: String,
        status: LanguageServerInstallationStatus,
    ) -> wasmtime::Result<()> {
        let status = match status {
            LanguageServerInstallationStatus::None => {
                super::since_v0_8_0::LanguageServerInstallationStatus::None
            }
            LanguageServerInstallationStatus::Downloading => {
                super::since_v0_8_0::LanguageServerInstallationStatus::Downloading
            }
            LanguageServerInstallationStatus::CheckingForUpdate => {
                super::since_v0_8_0::LanguageServerInstallationStatus::CheckingForUpdate
            }
            LanguageServerInstallationStatus::Failed(msg) => {
                super::since_v0_8_0::LanguageServerInstallationStatus::Failed(msg)
            }
        };
        <WasmState as super::since_v0_8_0::ExtensionImports>::set_language_server_installation_status(
            self,
            server_name,
            status,
        )
        .await
    }

    async fn download_file(
        &mut self,
        url: String,
        path: String,
        file_type: DownloadedFileType,
    ) -> wasmtime::Result<Result<(), String>> {
        let file_type = match file_type {
            DownloadedFileType::Gzip => super::since_v0_8_0::DownloadedFileType::Gzip,
            DownloadedFileType::GzipTar => super::since_v0_8_0::DownloadedFileType::GzipTar,
            DownloadedFileType::Zip => super::since_v0_8_0::DownloadedFileType::Zip,
            DownloadedFileType::Uncompressed => {
                super::since_v0_8_0::DownloadedFileType::Uncompressed
            }
        };
        <WasmState as super::since_v0_8_0::ExtensionImports>::download_file(
            self, url, path, file_type,
        )
        .await
    }

    async fn make_file_executable(
        &mut self,
        filepath: String,
    ) -> wasmtime::Result<Result<(), String>> {
        <WasmState as super::since_v0_8_0::ExtensionImports>::make_file_executable(
            self, filepath,
        )
        .await
    }

    async fn register_command(&mut self, id: String, label: String) -> wasmtime::Result<()> {
        let extension_id = self.manifest.id.clone();
        let display_name = format!(
            "{}: {}",
            self.manifest.name.to_lowercase().replace(' ', "_"),
            label
        );
        self.host
            .command_registrations_tx
            .unbounded_send((extension_id, display_name, id.into()))
            .ok();
        Ok(())
    }

    async fn execute_command(
        &mut self,
        command: String,
        args: Option<String>,
    ) -> wasmtime::Result<Result<String, String>> {
        let Some(tx) = &self.command_execution_tx else {
            return Ok(Err("Command execution not available".to_string()));
        };

        let (response_tx, response_rx) = futures::channel::oneshot::channel();

        if tx.unbounded_send(crate::wasm_host::CommandExecutionRequest::ExecuteCommand {
            command,
            args,
            response_tx,
        }).is_err() {
            return Ok(Err("Failed to send command execution request".to_string()));
        }

        match response_rx.await {
            Ok(result) => Ok(result),
            Err(_) => Ok(Err("Command execution channel closed".to_string())),
        }
    }

    async fn execute_slash_command(
        &mut self,
        command: String,
        args: Vec<String>,
    ) -> wasmtime::Result<Result<String, String>> {
        let Some(tx) = &self.command_execution_tx else {
            return Ok(Err("Slash command execution not available".to_string()));
        };

        let (response_tx, response_rx) = futures::channel::oneshot::channel();

        if tx.unbounded_send(crate::wasm_host::CommandExecutionRequest::ExecuteSlashCommand {
            command,
            args,
            response_tx,
        }).is_err() {
            return Ok(Err("Failed to send slash command execution request".to_string()));
        }

        match response_rx.await {
            Ok(result) => Ok(result),
            Err(_) => Ok(Err("Slash command execution channel closed".to_string())),
        }
    }
}

impl ui_elements::Host for WasmState {}

impl query::Host for WasmState {
    async fn query(
        &mut self,
        topic: String,
        data: String,
        timeout_ms: u32,
    ) -> wasmtime::Result<Result<Vec<query::QueryResponse>, String>> {
        let Some(tx) = &self.command_execution_tx else {
            return Ok(Err("Query not available".to_string()));
        };
        let source_extension_id = self.manifest.id.clone();
        let (response_tx, response_rx) = futures::channel::oneshot::channel();
        if tx
            .unbounded_send(crate::wasm_host::CommandExecutionRequest::QueryRequest {
                topic,
                source_extension_id,
                data,
                timeout_ms,
                response_tx,
            })
            .is_err()
        {
            return Ok(Err("Failed to send query request".to_string()));
        }
        match response_rx.await {
            Ok(Ok(responses)) => Ok(Ok(responses
                .into_iter()
                .map(|r| query::QueryResponse {
                    source: r.source,
                    data: r.data,
                })
                .collect())),
            Ok(Err(e)) => Ok(Err(e)),
            Err(_) => Ok(Err("Channel closed".to_string())),
        }
    }

    async fn register_query_handler(
        &mut self,
        topic: String,
    ) -> wasmtime::Result<Result<u64, String>> {
        let Some(tx) = &self.command_execution_tx else {
            return Ok(Err("Query not available".to_string()));
        };
        let Some(query_tx) = &self.query_tx else {
            return Ok(Err("Query delivery channel not available".to_string()));
        };
        let query_tx = query_tx.clone();
        let source_extension_id = self.manifest.id.clone();
        let (response_tx, response_rx) = futures::channel::oneshot::channel();
        if tx
            .unbounded_send(crate::wasm_host::CommandExecutionRequest::QueryRegisterHandler {
                topic,
                source_extension_id,
                query_tx,
                response_tx,
            })
            .is_err()
        {
            return Ok(Err("Failed to register query handler".to_string()));
        }
        Ok(response_rx.await.unwrap_or_else(|_| Err("Channel closed".to_string())))
    }

    async fn unregister_query_handler(
        &mut self,
        handler_id: u64,
    ) -> wasmtime::Result<Result<(), String>> {
        let Some(tx) = &self.command_execution_tx else {
            return Ok(Err("Query not available".to_string()));
        };
        let (response_tx, response_rx) = futures::channel::oneshot::channel();
        if tx
            .unbounded_send(crate::wasm_host::CommandExecutionRequest::QueryUnregisterHandler {
                handler_id,
                response_tx,
            })
            .is_err()
        {
            return Ok(Err("Failed to send unregister request".to_string()));
        }
        Ok(response_rx.await.unwrap_or_else(|_| Err("Channel closed".to_string())))
    }
}


impl pub_sub::Host for WasmState {
    async fn subscribe(&mut self, topic: String) -> wasmtime::Result<Result<u64, String>> {
        let Some(tx) = &self.command_execution_tx else {
            return Ok(Err("Pub-sub not available".to_string()));
        };
        let Some(event_tx) = &self.pub_sub_event_tx else {
            return Ok(Err("Pub-sub event channel not available".to_string()));
        };
        let event_tx = event_tx.clone();
        let source_extension_id = self.manifest.id.clone();
        let (response_tx, response_rx) = futures::channel::oneshot::channel();
        if tx
            .unbounded_send(crate::wasm_host::CommandExecutionRequest::PubSubSubscribe {
                topic,
                source_extension_id,
                event_tx,
                response_tx,
            })
            .is_err()
        {
            return Ok(Err("Failed to send subscribe request".to_string()));
        }
        Ok(response_rx.await.unwrap_or_else(|_| Err("Channel closed".to_string())))
    }

    async fn unsubscribe(&mut self, subscription_id: u64) -> wasmtime::Result<Result<(), String>> {
        let Some(tx) = &self.command_execution_tx else {
            return Ok(Err("Pub-sub not available".to_string()));
        };
        let (response_tx, response_rx) = futures::channel::oneshot::channel();
        if tx
            .unbounded_send(crate::wasm_host::CommandExecutionRequest::PubSubUnsubscribe {
                subscription_id,
                response_tx,
            })
            .is_err()
        {
            return Ok(Err("Failed to send unsubscribe request".to_string()));
        }
        Ok(response_rx.await.unwrap_or_else(|_| Err("Channel closed".to_string())))
    }

    async fn publish(
        &mut self,
        topic: String,
        data: String,
    ) -> wasmtime::Result<Result<(), String>> {
        let Some(tx) = &self.command_execution_tx else {
            return Ok(Err("Pub-sub not available".to_string()));
        };
        let source_extension_id = self.manifest.id.clone();
        let (response_tx, response_rx) = futures::channel::oneshot::channel();
        if tx
            .unbounded_send(crate::wasm_host::CommandExecutionRequest::PubSubPublish {
                topic,
                source_extension_id,
                data,
                response_tx,
            })
            .is_err()
        {
            return Ok(Err("Failed to send publish request".to_string()));
        }
        Ok(response_rx.await.unwrap_or_else(|_| Err("Channel closed".to_string())))
    }
}

impl gui::Host for WasmState {
    async fn create_focus_handle(&mut self) -> wasmtime::Result<u32> {
        let id = self.next_focus_handle_id;
        self.next_focus_handle_id = self.next_focus_handle_id.saturating_add(1);
        Ok(id)
    }

    async fn request_focus(&mut self, handle_id: u32) -> wasmtime::Result<()> {
        if let Some(tx) = &self.gui_panel_tx {
            let _ =
                tx.send(crate::wasm_host::GuiPanelMessage::RequestFocus(handle_id));
        }
        Ok(())
    }

    async fn drop_focus_handle(&mut self, _handle_id: u32) -> wasmtime::Result<()> {
        Ok(())
    }

    async fn emit(&mut self, name: String, data: String) -> wasmtime::Result<()> {
        if let Some(tx) = &self.gui_panel_tx {
            let _ = tx.send(crate::wasm_host::GuiPanelMessage::Emit { name, data });
        }
        Ok(())
    }

    async fn request_data(&mut self, key: String) -> wasmtime::Result<()> {
        if let Some(tx) = &self.gui_panel_tx {
            let _ = tx.send(crate::wasm_host::GuiPanelMessage::RequestData(key));
        }
        Ok(())
    }

    async fn call(
        &mut self,
        key: String,
        method: String,
        params: String,
    ) -> wasmtime::Result<Result<(), String>> {
        if let Some(tx) = &self.gui_panel_tx {
            let _ = tx.send(crate::wasm_host::GuiPanelMessage::Call {
                key,
                method,
                params,
            });
        }
        Ok(Ok(()))
    }

}
