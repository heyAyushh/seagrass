mod command;
mod config;
mod constants;
mod labels;
mod lsp_execute;
mod slash;
mod uri;

use {
    crate::{
        command::server_command_for_worktree,
        config::{
            agent_mode, diagnostics_transport, initialization_options, workspace_configuration,
        },
        constants::SERVER_ID,
        labels::{label_for_completion, label_for_symbol},
        slash::{run_seagrass_slash_command, slash_command_spec, unsupported_slash_command},
    },
    zed_extension_api::{self as zed, lsp, settings::LspSettings},
};

struct SeagrassExtension;

impl zed::Extension for SeagrassExtension {
    fn new() -> Self {
        Self
    }

    fn language_server_command(
        &mut self,
        language_server_id: &zed::LanguageServerId,
        worktree: &zed::Worktree,
    ) -> zed::Result<zed::Command> {
        if language_server_id.as_ref() != SERVER_ID {
            return Err(format!(
                "unsupported language server `{language_server_id}`"
            ));
        }

        set_installation_status(
            language_server_id,
            zed::LanguageServerInstallationStatus::CheckingForUpdate,
        );
        let command = server_command_for_worktree(worktree);
        match &command {
            Ok(_) => set_installation_status(
                language_server_id,
                zed::LanguageServerInstallationStatus::None,
            ),
            Err(error) => set_installation_status(
                language_server_id,
                zed::LanguageServerInstallationStatus::Failed(error.clone()),
            ),
        }
        command
    }

    fn language_server_initialization_options(
        &mut self,
        language_server_id: &zed::LanguageServerId,
        worktree: &zed::Worktree,
    ) -> zed::Result<Option<zed::serde_json::Value>> {
        if language_server_id.as_ref() != SERVER_ID {
            return Ok(None);
        }

        let settings = LspSettings::for_worktree(language_server_id.as_ref(), worktree)?;
        let agent_mode_enabled = agent_mode(settings.settings.as_ref());
        let transport = diagnostics_transport(settings.settings.as_ref());

        Ok(Some(initialization_options(agent_mode_enabled, &transport)))
    }

    fn language_server_workspace_configuration(
        &mut self,
        language_server_id: &zed::LanguageServerId,
        worktree: &zed::Worktree,
    ) -> zed::Result<Option<zed::serde_json::Value>> {
        if language_server_id.as_ref() != SERVER_ID {
            return Ok(None);
        }

        let settings = LspSettings::for_worktree(language_server_id.as_ref(), worktree)?;
        Ok(Some(workspace_configuration(settings.settings)))
    }

    fn complete_slash_command_argument(
        &self,
        command: zed::SlashCommand,
        args: Vec<String>,
    ) -> zed::Result<Vec<zed::SlashCommandArgumentCompletion>> {
        slash_command_spec(&command.name)
            .map(|spec| slash::complete_slash_command_argument(spec, &args))
            .ok_or_else(|| unsupported_slash_command(&command.name))
    }

    fn run_slash_command(
        &self,
        command: zed::SlashCommand,
        args: Vec<String>,
        worktree: Option<&zed::Worktree>,
    ) -> zed::Result<zed::SlashCommandOutput> {
        let spec = slash_command_spec(&command.name)
            .ok_or_else(|| unsupported_slash_command(&command.name))?;
        Ok(run_seagrass_slash_command(spec, args, worktree))
    }

    fn label_for_completion(
        &self,
        language_server_id: &zed::LanguageServerId,
        completion: lsp::Completion,
    ) -> Option<zed::CodeLabel> {
        label_for_completion(language_server_id, completion)
    }

    fn label_for_symbol(
        &self,
        language_server_id: &zed::LanguageServerId,
        symbol: lsp::Symbol,
    ) -> Option<zed::CodeLabel> {
        label_for_symbol(language_server_id, symbol)
    }
}

#[cfg(not(test))]
fn set_installation_status(
    language_server_id: &zed::LanguageServerId,
    status: zed::LanguageServerInstallationStatus,
) {
    zed::set_language_server_installation_status(language_server_id, &status);
}

#[cfg(test)]
fn set_installation_status(
    _language_server_id: &zed::LanguageServerId,
    _status: zed::LanguageServerInstallationStatus,
) {
}

zed::register_extension!(SeagrassExtension);
