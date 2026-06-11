use crate::args::DeviceCommands;
use crate::error::{CliError, CliResult};
use crate::output::{self, OutputFormat, PublicKeyOutput, UnwrappedKeyOutput, WrappedKeyOutput};
use crate::platform;
use crate::{from_hex, to_hex};
use taskmanager_core::{
    init_device_keypair, unwrap_data_key, wrap_data_key, Blob, Platform, ACCOUNT_DATA_KEY_ID,
    DEVICE_PRIVATE_KEY_ID,
};

pub(crate) fn run(
    command: DeviceCommands,
    output_format: OutputFormat,
    offline: bool,
) -> CliResult<Option<String>> {
    let platform = platform::CliPlatform::new(offline);
    match command {
        DeviceCommands::InitKeypair => {
            let public_key = init_device_keypair(&platform).map_err(CliError::from)?;
            output::format_command_result(
                output_format,
                &PublicKeyOutput {
                    public_key: to_hex(&public_key),
                },
            )
            .map(Some)
        }
        DeviceCommands::Register => Err(CliError::UnsupportedPlatform(
            "device register is not implemented until server auth is wired".into(),
        )),
        DeviceCommands::List => Err(CliError::UnsupportedPlatform(
            "device list is not implemented until server auth is wired".into(),
        )),
        DeviceCommands::WrapKey(args) => {
            let target_public_key = from_hex(&args.target)?;
            let data_key = platform
                .load_key(ACCOUNT_DATA_KEY_ID)
                .map_err(CliError::from)?;
            let private_key = platform
                .load_key(DEVICE_PRIVATE_KEY_ID)
                .map_err(CliError::from)?;
            let wrapped = wrap_data_key(&data_key, &target_public_key, &private_key)
                .map_err(CliError::from)?;
            output::format_command_result(
                output_format,
                &WrappedKeyOutput {
                    ciphertext: to_hex(&wrapped.ciphertext),
                    nonce: to_hex(&wrapped.nonce),
                },
            )
            .map(Some)
        }
        DeviceCommands::UnwrapKey(args) => {
            let from_public_key = from_hex(&args.from_device)?;
            let ciphertext = from_hex(&args.ciphertext)?;
            let nonce_bytes = from_hex(&args.nonce)?;
            let nonce: [u8; 12] = nonce_bytes.try_into().map_err(|bytes: Vec<u8>| {
                CliError::Crypto(format!(
                    "bad nonce length: expected 12 bytes, got {}",
                    bytes.len()
                ))
            })?;
            let private_key = platform
                .load_key(DEVICE_PRIVATE_KEY_ID)
                .map_err(CliError::from)?;
            let data_key =
                unwrap_data_key(&Blob { ciphertext, nonce }, &from_public_key, &private_key)
                    .map_err(CliError::from)?;
            platform
                .store_key(ACCOUNT_DATA_KEY_ID, &data_key)
                .map_err(CliError::from)?;
            output::format_command_result(output_format, &UnwrappedKeyOutput { stored: true })
                .map(Some)
        }
    }
}
