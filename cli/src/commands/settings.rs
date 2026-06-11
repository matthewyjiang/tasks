use std::path::PathBuf;

use crate::args::SettingsCommands;
use crate::error::{CliError, CliResult};
use crate::output::{self, OutputFormat};
use crate::resolve_config_path;
use taskmanager_core::{AuthMethod, PlaintextSettings, PlaintextSettingsSyncPayload};

pub(crate) fn run(
    command: SettingsCommands,
    output_format: OutputFormat,
    config_path: Option<PathBuf>,
    profile: &str,
) -> CliResult<Option<String>> {
    let path = resolve_config_path(config_path, profile)?;
    match command {
        SettingsCommands::Get(args) => {
            let settings = PlaintextSettings::read_from_file(&path).map_err(CliError::from)?;
            if let Some(key) = args.key {
                let value = plaintext_setting_value(&settings, &key)?;
                output::format_command_result(output_format, &value).map(Some)
            } else {
                output::format_command_result(output_format, &settings).map(Some)
            }
        }
        SettingsCommands::Set(args) => {
            let mut settings = PlaintextSettings::read_from_file(&path).map_err(CliError::from)?;
            set_plaintext_setting(&mut settings, &args.key, &args.value)?;
            settings.write_to_file(&path).map_err(CliError::from)?;
            output::format_command_result(output_format, &settings).map(Some)
        }
        SettingsCommands::PullPlaintext => {
            let settings = PlaintextSettings::read_from_file(&path).map_err(CliError::from)?;
            output::format_command_result(output_format, &settings.sync_payload()).map(Some)
        }
        SettingsCommands::PushPlaintext(args) => {
            let payload: PlaintextSettingsSyncPayload =
                serde_json::from_str(&args.json).map_err(CliError::from)?;
            let mut settings = PlaintextSettings::read_from_file(&path).map_err(CliError::from)?;
            apply_plaintext_sync_payload(&mut settings, payload)?;
            settings.write_to_file(&path).map_err(CliError::from)?;
            output::format_command_result(output_format, &settings).map(Some)
        }
        SettingsCommands::Migrate => {
            let settings = PlaintextSettings::read_from_file(&path).map_err(CliError::from)?;
            settings.write_to_file(&path).map_err(CliError::from)?;
            output::format_command_result(output_format, &settings).map(Some)
        }
    }
}

fn apply_plaintext_sync_payload(
    settings: &mut PlaintextSettings,
    payload: PlaintextSettingsSyncPayload,
) -> CliResult<()> {
    if payload.schema_version != taskmanager_core::PLAINTEXT_SETTINGS_SCHEMA_VERSION {
        return Err(CliError::Input(format!(
            "unsupported plaintext settings schema_version: {}",
            payload.schema_version
        )));
    }
    set_plaintext_setting(settings, "server_url", &payload.server_url)?;
    settings.auth_method = payload.auth_method;
    set_plaintext_setting(settings, "language", &payload.language)?;
    Ok(())
}

fn plaintext_setting_value(
    settings: &PlaintextSettings,
    key: &str,
) -> CliResult<serde_json::Value> {
    match key {
        "schema_version" => Ok(settings.schema_version.into()),
        "server_url" => Ok(settings.server_url.clone().into()),
        "auth_method" => serde_json::to_value(settings.auth_method).map_err(CliError::from),
        "language" => Ok(settings.language.clone().into()),
        "last_sync_cursor" => Ok(settings.last_sync_cursor.into()),
        _ => Err(CliError::Input(format!("unknown settings key: {key}"))),
    }
}

pub(crate) fn set_plaintext_setting(
    settings: &mut PlaintextSettings,
    key: &str,
    value: &str,
) -> CliResult<()> {
    match key {
        "server_url" => {
            if !(value.is_empty() || value.starts_with("http://") || value.starts_with("https://"))
            {
                return Err(CliError::Input(
                    "server_url must be empty or start with http:// or https://".into(),
                ));
            }
            settings.server_url = value.to_owned();
        }
        "auth_method" => {
            settings.auth_method = match value {
                "biometric" => AuthMethod::Biometric,
                "pin" => AuthMethod::Pin,
                "password" => AuthMethod::Password,
                _ => {
                    return Err(CliError::Input(
                        "auth_method must be one of: biometric, pin, password".into(),
                    ))
                }
            };
        }
        "language" => {
            if value.trim().is_empty() {
                return Err(CliError::Input("language must not be empty".into()));
            }
            settings.language = value.to_owned();
        }
        "last_sync_cursor" => {
            settings.last_sync_cursor = value
                .parse::<i64>()
                .map_err(|_| CliError::Input("last_sync_cursor must be a signed integer".into()))?;
        }
        "schema_version" => {
            return Err(CliError::Input(
                "schema_version is managed by settings migrate".into(),
            ));
        }
        _ => return Err(CliError::Input(format!("unknown settings key: {key}"))),
    }
    Ok(())
}
