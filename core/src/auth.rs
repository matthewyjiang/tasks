use crate::crypto::public_key_from_private_key;
use crate::error::{CoreError, CoreResult, PlatformError};
use crate::platform::{Platform, ACCOUNT_DATA_KEY_ID, DEVICE_PRIVATE_KEY_ID};

pub const AUTH_ACCESS_TOKEN_ID: &str = "auth_access_token";
pub const AUTH_REFRESH_TOKEN_ID: &str = "auth_refresh_token";
pub const AUTH_SYNC_ORIGIN_ID: &str = "auth_sync_origin";
pub const AUTH_ACCOUNT_ID_ID: &str = "auth_account_id";

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct AuthCredentials {
    pub email: String,
    pub password: String,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct RegisterRequest {
    pub email: String,
    pub password: String,
    pub pub_key: String,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct LoginRequest {
    pub email: String,
    pub password: String,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct RefreshTokenRequest {
    pub refresh_token: String,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct DeleteSessionRequest {
    pub refresh_token: String,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct PutCurrentDeviceKeyRequest {
    pub pub_key: String,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct TokenResponse {
    pub jwt: String,
    pub refresh_token: String,
    pub user_id: Option<String>,
}

pub trait AuthClient {
    fn register(&self, server_url: &str, request: RegisterRequest) -> CoreResult<TokenResponse>;
    fn login(&self, server_url: &str, request: LoginRequest) -> CoreResult<TokenResponse>;
    fn refresh(&self, server_url: &str, request: RefreshTokenRequest) -> CoreResult<TokenResponse>;
    fn delete_session(&self, server_url: &str, request: DeleteSessionRequest) -> CoreResult<()>;
    fn put_current_device_key(
        &self,
        server_url: &str,
        access_token: &str,
        request: PutCurrentDeviceKeyRequest,
    ) -> CoreResult<()>;
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum SyncAuthState {
    LocalOnlyReady,
    AuthenticatedEnrollmentPending,
    SyncReady,
    AuthRequired,
    MisconfiguredOrigin,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ConfigureSyncAuthResult {
    pub server_url: String,
    pub sync_origin: String,
    pub account_id: Option<String>,
    pub state: SyncAuthState,
}

pub fn normalize_sync_server_url(server_url: &str) -> CoreResult<String> {
    let server_url = server_url.trim().trim_end_matches('/').to_owned();
    if server_url.is_empty() {
        return Err(config_error("server URL is required"));
    }

    let parsed = ParsedSyncUrl::parse(&server_url)?;
    if parsed.has_credentials {
        return Err(config_error("server URL must not include credentials"));
    }
    if parsed.has_query_or_fragment {
        return Err(config_error(
            "server URL must not include a query or fragment",
        ));
    }
    match parsed.scheme.as_str() {
        "https" => Ok(server_url),
        "http" if host_is_loopback(&parsed.host) => Ok(server_url),
        "http" => Err(config_error(
            "server URL must use HTTPS unless it targets localhost",
        )),
        _ => Err(config_error("server URL must use HTTPS")),
    }
}

pub fn sync_server_origin(server_url: &str) -> CoreResult<String> {
    let server_url = normalize_sync_server_url(server_url)?;
    let parsed = ParsedSyncUrl::parse(&server_url)?;
    let port = parsed.port.unwrap_or(match parsed.scheme.as_str() {
        "https" => 443,
        "http" => 80,
        _ => return Err(config_error("server URL must include a valid port")),
    });
    let host = if parsed.host.contains(':') && !parsed.host.starts_with('[') {
        format!("[{}]", parsed.host)
    } else {
        parsed.host
    };
    Ok(format!("{}://{}:{}", parsed.scheme, host, port))
}

pub fn sync_auth_state(platform: &dyn Platform, server_url: &str) -> SyncAuthState {
    let Ok(settings_origin) = sync_server_origin(server_url) else {
        return SyncAuthState::MisconfiguredOrigin;
    };
    let Ok(stored_origin) = load_utf8_key(platform, AUTH_SYNC_ORIGIN_ID) else {
        return SyncAuthState::LocalOnlyReady;
    };
    if stored_origin != settings_origin {
        return SyncAuthState::MisconfiguredOrigin;
    }
    if platform.load_key(AUTH_ACCESS_TOKEN_ID).is_err()
        || platform.load_key(AUTH_REFRESH_TOKEN_ID).is_err()
        || platform.load_key(DEVICE_PRIVATE_KEY_ID).is_err()
    {
        return SyncAuthState::AuthRequired;
    }
    if platform.load_key(ACCOUNT_DATA_KEY_ID).is_err() {
        return SyncAuthState::AuthenticatedEnrollmentPending;
    }
    SyncAuthState::SyncReady
}

pub fn sync_auth_configured(platform: &dyn Platform, server_url: &str) -> bool {
    sync_auth_state(platform, server_url) == SyncAuthState::SyncReady
}

pub fn configure_sync_auth(
    platform: &dyn Platform,
    auth_client: &dyn AuthClient,
    server_url: &str,
    credentials: AuthCredentials,
    register_public_key_base64: String,
) -> CoreResult<ConfigureSyncAuthResult> {
    let server_url = normalize_sync_server_url(server_url)?;
    let sync_origin = sync_server_origin(&server_url)?;
    let email = credentials.email.trim().to_owned();
    if email.is_empty() || credentials.password.is_empty() {
        return Err(config_error("email and password are required"));
    }

    let register = RegisterRequest {
        email: email.clone(),
        password: credentials.password.clone(),
        pub_key: register_public_key_base64.clone(),
    };
    let tokens = match auth_client.register(&server_url, register) {
        Ok(tokens) => tokens,
        Err(_) => {
            let tokens = auth_client.login(
                &server_url,
                LoginRequest {
                    email,
                    password: credentials.password,
                },
            )?;
            let access_token = tokens.jwt.clone();
            auth_client.put_current_device_key(
                &server_url,
                &access_token,
                PutCurrentDeviceKeyRequest {
                    pub_key: register_public_key_base64,
                },
            )?;
            tokens
        }
    };

    store_tokens(platform, &tokens)?;
    platform.store_key(AUTH_SYNC_ORIGIN_ID, sync_origin.as_bytes())?;
    if let Some(account_id) = &tokens.user_id {
        platform.store_key(AUTH_ACCOUNT_ID_ID, account_id.as_bytes())?;
    }

    Ok(ConfigureSyncAuthResult {
        server_url,
        sync_origin,
        account_id: tokens.user_id,
        state: if platform.load_key(ACCOUNT_DATA_KEY_ID).is_ok() {
            SyncAuthState::SyncReady
        } else {
            SyncAuthState::AuthenticatedEnrollmentPending
        },
    })
}

pub fn refresh_auth(
    platform: &dyn Platform,
    auth_client: &dyn AuthClient,
    server_url: &str,
) -> CoreResult<TokenResponse> {
    let server_url = normalize_sync_server_url(server_url)?;
    let refresh_token = load_utf8_key(platform, AUTH_REFRESH_TOKEN_ID)?;
    let tokens = auth_client.refresh(&server_url, RefreshTokenRequest { refresh_token })?;
    store_tokens(platform, &tokens)?;
    Ok(tokens)
}

pub fn logout_sync_auth(
    platform: &dyn Platform,
    auth_client: &dyn AuthClient,
    server_url: &str,
) -> CoreResult<()> {
    let server_url = normalize_sync_server_url(server_url)?;
    if let Ok(refresh_token) = load_utf8_key(platform, AUTH_REFRESH_TOKEN_ID) {
        auth_client.delete_session(&server_url, DeleteSessionRequest { refresh_token })?;
    }
    clear_sync_auth(platform)
}

pub fn clear_sync_auth(platform: &dyn Platform) -> CoreResult<()> {
    platform.delete_key(AUTH_ACCESS_TOKEN_ID)?;
    platform.delete_key(AUTH_REFRESH_TOKEN_ID)?;
    let _ = platform.delete_key(AUTH_SYNC_ORIGIN_ID);
    let _ = platform.delete_key(AUTH_ACCOUNT_ID_ID);
    Ok(())
}

pub fn device_public_key_base64_from_platform(platform: &dyn Platform) -> CoreResult<String> {
    let private_key = platform.load_key(DEVICE_PRIVATE_KEY_ID)?;
    let public_key = public_key_from_private_key(&private_key)?;
    Ok(base64_standard_encode(&public_key))
}

pub fn store_tokens(platform: &dyn Platform, tokens: &TokenResponse) -> CoreResult<()> {
    platform.store_key(AUTH_ACCESS_TOKEN_ID, tokens.jwt.as_bytes())?;
    platform.store_key(AUTH_REFRESH_TOKEN_ID, tokens.refresh_token.as_bytes())?;
    Ok(())
}

pub fn load_access_token(platform: &dyn Platform) -> CoreResult<String> {
    load_utf8_key(platform, AUTH_ACCESS_TOKEN_ID)
}

fn load_utf8_key(platform: &dyn Platform, key_id: &str) -> CoreResult<String> {
    String::from_utf8(platform.load_key(key_id)?).map_err(|error| {
        PlatformError::OperationFailed(format!("stored {key_id} is not UTF-8: {error}")).into()
    })
}

fn config_error(message: &str) -> CoreError {
    PlatformError::OperationFailed(message.to_owned()).into()
}

#[derive(Debug, PartialEq, Eq)]
struct ParsedSyncUrl {
    scheme: String,
    host: String,
    port: Option<u16>,
    has_credentials: bool,
    has_query_or_fragment: bool,
}

impl ParsedSyncUrl {
    fn parse(input: &str) -> CoreResult<Self> {
        let (scheme, rest) = input
            .split_once("://")
            .ok_or_else(|| config_error("server URL is invalid"))?;
        let scheme = scheme.to_ascii_lowercase();
        if scheme.is_empty() || rest.is_empty() {
            return Err(config_error("server URL is invalid"));
        }
        let has_query_or_fragment = rest.contains('?') || rest.contains('#');
        let authority = rest
            .split(['/', '?', '#'])
            .next()
            .ok_or_else(|| config_error("server URL must include a host"))?;
        if authority.is_empty() {
            return Err(config_error("server URL must include a host"));
        }
        let has_credentials = authority.rsplit('@').count() > 1;
        let authority = authority.rsplit('@').next().unwrap_or(authority);
        let (host, port) = parse_host_port(authority)?;
        if host.is_empty() {
            return Err(config_error("server URL must include a host"));
        }
        Ok(Self {
            scheme,
            host,
            port,
            has_credentials,
            has_query_or_fragment,
        })
    }
}

fn parse_host_port(authority: &str) -> CoreResult<(String, Option<u16>)> {
    if let Some(rest) = authority.strip_prefix('[') {
        let (host, after) = rest
            .split_once(']')
            .ok_or_else(|| config_error("server URL is invalid"))?;
        let port = if let Some(port) = after.strip_prefix(':') {
            Some(
                port.parse::<u16>()
                    .map_err(|_| config_error("server URL must include a valid port"))?,
            )
        } else if after.is_empty() {
            None
        } else {
            return Err(config_error("server URL is invalid"));
        };
        return Ok((host.to_owned(), port));
    }

    if authority.matches(':').count() > 1 {
        return Ok((authority.to_owned(), None));
    }
    let Some((host, port)) = authority.rsplit_once(':') else {
        return Ok((authority.to_owned(), None));
    };
    if port.is_empty() {
        return Err(config_error("server URL must include a valid port"));
    }
    Ok((
        host.to_owned(),
        Some(
            port.parse::<u16>()
                .map_err(|_| config_error("server URL must include a valid port"))?,
        ),
    ))
}

fn host_is_loopback(host: &str) -> bool {
    host.eq_ignore_ascii_case("localhost")
        || host
            .parse::<std::net::IpAddr>()
            .is_ok_and(|ip| ip.is_loopback())
}

fn base64_standard_encode(bytes: &[u8]) -> String {
    const TABLE: &[u8; 64] = b"ABCDEFGHIJKLMNOPQRSTUVWXYZabcdefghijklmnopqrstuvwxyz0123456789+/";
    let mut out = String::with_capacity(bytes.len().div_ceil(3) * 4);
    for chunk in bytes.chunks(3) {
        let b0 = chunk[0];
        let b1 = *chunk.get(1).unwrap_or(&0);
        let b2 = *chunk.get(2).unwrap_or(&0);
        out.push(TABLE[(b0 >> 2) as usize] as char);
        out.push(TABLE[(((b0 & 0b0000_0011) << 4) | (b1 >> 4)) as usize] as char);
        if chunk.len() > 1 {
            out.push(TABLE[(((b1 & 0b0000_1111) << 2) | (b2 >> 6)) as usize] as char);
        } else {
            out.push('=');
        }
        if chunk.len() > 2 {
            out.push(TABLE[(b2 & 0b0011_1111) as usize] as char);
        } else {
            out.push('=');
        }
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::platform::{init_account, init_device_keypair, MockPlatform};
    use std::sync::Mutex;

    #[derive(Default)]
    struct FakeAuthClient {
        register_response: Mutex<Option<CoreResult<TokenResponse>>>,
        login_response: Mutex<Option<TokenResponse>>,
        refresh_response: Mutex<Option<TokenResponse>>,
        registered_device_key: Mutex<Option<String>>,
        deleted_refresh_token: Mutex<Option<String>>,
    }

    impl AuthClient for FakeAuthClient {
        fn register(
            &self,
            _server_url: &str,
            _request: RegisterRequest,
        ) -> CoreResult<TokenResponse> {
            self.register_response
                .lock()
                .unwrap()
                .take()
                .unwrap_or_else(|| {
                    Ok(TokenResponse {
                        jwt: "registered-jwt".to_owned(),
                        refresh_token: "registered-refresh".to_owned(),
                        user_id: Some("user-1".to_owned()),
                    })
                })
        }

        fn login(&self, _server_url: &str, _request: LoginRequest) -> CoreResult<TokenResponse> {
            Ok(self
                .login_response
                .lock()
                .unwrap()
                .take()
                .unwrap_or(TokenResponse {
                    jwt: "login-jwt".to_owned(),
                    refresh_token: "login-refresh".to_owned(),
                    user_id: Some("user-1".to_owned()),
                }))
        }

        fn refresh(
            &self,
            _server_url: &str,
            _request: RefreshTokenRequest,
        ) -> CoreResult<TokenResponse> {
            Ok(self
                .refresh_response
                .lock()
                .unwrap()
                .take()
                .unwrap_or(TokenResponse {
                    jwt: "rotated-jwt".to_owned(),
                    refresh_token: "rotated-refresh".to_owned(),
                    user_id: None,
                }))
        }

        fn delete_session(
            &self,
            _server_url: &str,
            request: DeleteSessionRequest,
        ) -> CoreResult<()> {
            *self.deleted_refresh_token.lock().unwrap() = Some(request.refresh_token);
            Ok(())
        }

        fn put_current_device_key(
            &self,
            _server_url: &str,
            _access_token: &str,
            request: PutCurrentDeviceKeyRequest,
        ) -> CoreResult<()> {
            *self.registered_device_key.lock().unwrap() = Some(request.pub_key);
            Ok(())
        }
    }

    #[test]
    fn server_url_normalization_matches_sync_policy() {
        assert_eq!(
            normalize_sync_server_url(" https://example.com/api/ ").unwrap(),
            "https://example.com/api"
        );
        assert_eq!(
            normalize_sync_server_url("http://localhost:8080/").unwrap(),
            "http://localhost:8080"
        );
        assert!(normalize_sync_server_url("http://example.com").is_err());
        assert!(normalize_sync_server_url("https://user@example.com").is_err());
        assert!(normalize_sync_server_url("https://example.com?x=1").is_err());
    }

    #[test]
    fn sync_origin_uses_default_ports_and_ipv6_brackets() {
        assert_eq!(
            sync_server_origin("https://example.com/api").unwrap(),
            "https://example.com:443"
        );
        assert_eq!(
            sync_server_origin("http://127.0.0.1:8080").unwrap(),
            "http://127.0.0.1:8080"
        );
        assert_eq!(
            sync_server_origin("http://[::1]:8080").unwrap(),
            "http://[::1]:8080"
        );
    }

    #[test]
    fn register_flow_stores_tokens_origin_and_reports_ready_when_data_key_exists() {
        let platform = MockPlatform::new();
        init_account(&platform).unwrap();
        let client = FakeAuthClient::default();
        let result = configure_sync_auth(
            &platform,
            &client,
            "https://example.com",
            AuthCredentials {
                email: "me@example.com".to_owned(),
                password: "secret".to_owned(),
            },
            "public-key".to_owned(),
        )
        .unwrap();
        assert_eq!(result.state, SyncAuthState::SyncReady);
        assert_eq!(load_access_token(&platform).unwrap(), "registered-jwt");
        assert_eq!(
            String::from_utf8(platform.load_key(AUTH_REFRESH_TOKEN_ID).unwrap()).unwrap(),
            "registered-refresh"
        );
        assert_eq!(
            String::from_utf8(platform.load_key(AUTH_SYNC_ORIGIN_ID).unwrap()).unwrap(),
            "https://example.com:443"
        );
        assert!(sync_auth_configured(&platform, "https://example.com"));
    }

    #[test]
    fn login_fallback_registers_device_and_does_not_create_account_data_key() {
        let platform = MockPlatform::new();
        init_device_keypair(&platform).unwrap();
        let client = FakeAuthClient::default();
        *client.register_response.lock().unwrap() = Some(Err(config_error("already exists")));
        let result = configure_sync_auth(
            &platform,
            &client,
            "https://example.com",
            AuthCredentials {
                email: "me@example.com".to_owned(),
                password: "secret".to_owned(),
            },
            "public-key".to_owned(),
        )
        .unwrap();
        assert_eq!(result.state, SyncAuthState::AuthenticatedEnrollmentPending);
        assert_eq!(
            *client.registered_device_key.lock().unwrap(),
            Some("public-key".to_owned())
        );
        assert!(platform.load_key(ACCOUNT_DATA_KEY_ID).is_err());
        assert!(!sync_auth_configured(&platform, "https://example.com"));
    }

    #[test]
    fn refresh_rotates_stored_tokens() {
        let platform = MockPlatform::new();
        platform
            .store_key(AUTH_REFRESH_TOKEN_ID, b"old-refresh")
            .unwrap();
        refresh_auth(&platform, &FakeAuthClient::default(), "https://example.com").unwrap();
        assert_eq!(load_access_token(&platform).unwrap(), "rotated-jwt");
        assert_eq!(
            String::from_utf8(platform.load_key(AUTH_REFRESH_TOKEN_ID).unwrap()).unwrap(),
            "rotated-refresh"
        );
    }

    #[test]
    fn logout_deletes_auth_secrets() {
        let platform = MockPlatform::new();
        platform.store_key(AUTH_ACCESS_TOKEN_ID, b"jwt").unwrap();
        platform
            .store_key(AUTH_REFRESH_TOKEN_ID, b"refresh")
            .unwrap();
        platform.store_key(AUTH_SYNC_ORIGIN_ID, b"origin").unwrap();
        logout_sync_auth(&platform, &FakeAuthClient::default(), "https://example.com").unwrap();
        assert!(platform.load_key(AUTH_ACCESS_TOKEN_ID).is_err());
        assert!(platform.load_key(AUTH_REFRESH_TOKEN_ID).is_err());
    }
}
