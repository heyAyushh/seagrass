mod command;
mod config;
mod constants;
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
        slash::{run_seagrass_slash_command, slash_command_spec, unsupported_slash_command},
    },
    zed_extension_api::{self as zed, settings::LspSettings},
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

        server_command_for_worktree(worktree)
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
        _args: Vec<String>,
    ) -> zed::Result<Vec<zed::SlashCommandArgumentCompletion>> {
        slash_command_spec(&command.name)
            .map(|_| Vec::new())
            .ok_or_else(|| unsupported_slash_command(&command.name))
    }

    fn run_slash_command(
        &self,
        command: zed::SlashCommand,
        _args: Vec<String>,
        worktree: Option<&zed::Worktree>,
    ) -> zed::Result<zed::SlashCommandOutput> {
        let spec = slash_command_spec(&command.name)
            .ok_or_else(|| unsupported_slash_command(&command.name))?;
        Ok(run_seagrass_slash_command(spec, worktree))
    }
}

zed::register_extension!(SeagrassExtension);
