use crate::args::AccountCommands;
use crate::error::{CliError, CliResult};
use crate::output::{self, AccountClearOutput, OutputFormat, PublicKeyOutput};
use crate::platform;
use crate::{key_exists, to_hex, AUTH_ACCESS_TOKEN_ID, AUTH_REFRESH_TOKEN_ID};
use taskmanager_core::{init_account, Platform, ACCOUNT_DATA_KEY_ID, DEVICE_PRIVATE_KEY_ID};

pub(crate) fn run(
    command: AccountCommands,
    output_format: OutputFormat,
    offline: bool,
) -> CliResult<Option<String>> {
    match command {
        AccountCommands::Init => {
            let platform = platform::CliPlatform::new(offline);
            if key_exists(&platform, DEVICE_PRIVATE_KEY_ID)?
                || key_exists(&platform, ACCOUNT_DATA_KEY_ID)?
            {
                return Err(CliError::Conflict("account already exists".into()));
            }

            let public_key = init_account(&platform).map_err(CliError::from)?;
            output::format_command_result(
                output_format,
                &PublicKeyOutput {
                    public_key: to_hex(&public_key),
                },
            )
            .map(Some)
        }
        AccountCommands::Clear => {
            let platform = platform::CliPlatform::new(offline);
            platform
                .delete_key(AUTH_ACCESS_TOKEN_ID)
                .map_err(CliError::from)?;
            platform
                .delete_key(AUTH_REFRESH_TOKEN_ID)
                .map_err(CliError::from)?;
            platform
                .delete_key(DEVICE_PRIVATE_KEY_ID)
                .map_err(CliError::from)?;
            platform
                .delete_key(ACCOUNT_DATA_KEY_ID)
                .map_err(CliError::from)?;
            output::format_command_result(
                output_format,
                &AccountClearOutput {
                    auth_tokens_cleared: true,
                    device_private_key_cleared: true,
                    account_data_key_cleared: true,
                },
            )
            .map(Some)
        }
    }
}
