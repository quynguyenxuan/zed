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
}

impl gui::Host for WasmState {
    async fn set_view(&mut self, view_json: String) -> wasmtime::Result<()> {
        if let Some(tx) = &self.gui_panel_tx {
            let _ = tx.unbounded_send(crate::wasm_host::GuiPanelMessage::SetView(view_json));
        }
        Ok(())
    }

    async fn emit(&mut self, name: String, data: String) -> wasmtime::Result<()> {
        if let Some(tx) = &self.gui_panel_tx {
            let _ = tx.unbounded_send(crate::wasm_host::GuiPanelMessage::Emit { name, data });
        }
        Ok(())
    }

    async fn request_data(&mut self, key: String) -> wasmtime::Result<()> {
        if let Some(tx) = &self.gui_panel_tx {
            let _ = tx.unbounded_send(crate::wasm_host::GuiPanelMessage::RequestData(key));
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
            let _ = tx.unbounded_send(crate::wasm_host::GuiPanelMessage::Call {
                key,
                method,
                params,
            });
        }
        Ok(Ok(()))
    }
}
