//! FFI client implementation.
//!
//! This module provides the C-compatible API for creating and destroying MongoDB clients.

use std::path::PathBuf;

use crate::{
    client::auth::{AuthMechanism, Credential},
    error::Result,
    options::{ClientOptions, ServerAddress, Tls, TlsOptions},
    Client,
};

use super::{
    types::{AuthSettingsFFI, ConnectionSettingsFFI, TlsSettingsFFI},
    utils::{c_char_to_string, i32_to_option_u32, i64_to_duration_ms, parse_hosts},
};

/// Opaque pointer type for MongoClient.
///
/// This wraps the Rust Client along with a reference to the shared global Tokio runtime.
pub struct MongoClient {
    #[allow(dead_code)] // Will be used when operations are implemented
    client: Client,
    // TODO: Add runtime handle when global runtime is implemented
    // runtime: Arc<Runtime>,
}

/// Create a new MongoClient. Returns pointer on success, null on error.
///
/// # Safety
///
/// - `connection_settings` must be a valid pointer to a ConnectionSettingsFFI struct
/// - `auth_settings` can be null or a valid pointer to an AuthSettingsFFI struct
/// - `tls_settings` can be null or a valid pointer to a TlsSettingsFFI struct
/// - All C string pointers in the settings structs must be valid null-terminated strings
#[no_mangle]
pub unsafe extern "C" fn mongo_client_new(
    connection_settings: *const ConnectionSettingsFFI,
    auth_settings: *const AuthSettingsFFI,
    tls_settings: *const TlsSettingsFFI,
) -> *mut MongoClient {
    // Convert the FFI settings to Rust ClientOptions
    let result = build_client_options(connection_settings, auth_settings, tls_settings);

    match result {
        Ok(options) => {
            // Create the client synchronously (it doesn't do I/O in the constructor)
            match Client::with_options(options) {
                Ok(client) => {
                    let inner = MongoClient {
                        client,
                        // TODO: Clone Arc to global runtime when implemented
                    };
                    Box::into_raw(Box::new(inner)) as *mut MongoClient
                }
                Err(_) => std::ptr::null_mut(),
            }
        }
        Err(_) => std::ptr::null_mut(),
    }
}

/// Destroy a MongoClient. All sessions, cursors, and handles become invalid.
///
/// # Safety
///
/// - `client` must be a valid pointer returned from `mongo_client_new`
/// - `client` must not be used after this call
/// - This function must only be called once per client
#[no_mangle]
pub unsafe extern "C" fn mongo_client_destroy(client: *mut MongoClient) {
    if client.is_null() {
        return;
    }

    // Convert back to a Box and drop it
    // This will decrement the runtime refcount when the Arc is dropped
    drop(Box::from_raw(client));
}

/// Build ClientOptions from FFI settings.
///
/// # Safety
///
/// All pointers must be valid as described in `mongo_client_new`.
unsafe fn build_client_options(
    connection_settings: *const ConnectionSettingsFFI,
    auth_settings: *const AuthSettingsFFI,
    tls_settings: *const TlsSettingsFFI,
) -> Result<ClientOptions> {
    if connection_settings.is_null() {
        return Err(crate::error::Error::invalid_argument(
            "connection_settings cannot be null",
        ));
    }

    // Fully destructure ConnectionSettingsFFI to ensure all fields are handled
    let ConnectionSettingsFFI {
        hosts,
        app_name,
        compressors,
        direct_connection,
        load_balanced,
        max_pool_size,
        min_pool_size,
        max_idle_time_ms,
        connect_timeout_ms,
        socket_timeout_ms,
        server_selection_timeout_ms,
        local_threshold_ms,
        heartbeat_frequency_ms,
        replica_set,
        read_preference_mode,
        srv_service_name,
        srv_max_hosts,
    } = &*connection_settings;

    // Parse hosts (required field)
    let hosts_str = c_char_to_string(*hosts)?
        .ok_or_else(|| crate::error::Error::invalid_argument("hosts cannot be null"))?;
    let host_strings = parse_hosts(&hosts_str)?;
    let parsed_hosts: Result<Vec<ServerAddress>> = host_strings
        .iter()
        .map(|h| ServerAddress::parse(h))
        .collect();

    // Parse authentication if provided
    let credential = if !auth_settings.is_null() {
        // Fully destructure AuthSettingsFFI to ensure all fields are handled
        let AuthSettingsFFI {
            mechanism,
            username,
            password,
            source,
        } = &*auth_settings;

        let parsed_mechanism = c_char_to_string(*mechanism)?
            .map(|s| parse_auth_mechanism(&s))
            .transpose()?;

        Some(Credential {
            username: c_char_to_string(*username)?,
            source: c_char_to_string(*source)?,
            password: c_char_to_string(*password)?,
            mechanism: parsed_mechanism,
            mechanism_properties: None,
            oidc_callback: Default::default(),
        })
    } else {
        None
    };

    // Parse TLS if provided
    let tls = if !tls_settings.is_null() {
        // Fully destructure TlsSettingsFFI to ensure all fields are handled
        let TlsSettingsFFI {
            enabled,
            allow_invalid_certificates,
            allow_invalid_hostnames: _allow_invalid_hostnames,
            ca_file,
            cert_file,
            cert_key_file: _cert_key_file,
        } = &*tls_settings;

        if *enabled {
            Some(Tls::Enabled(TlsOptions {
                allow_invalid_certificates: Some(*allow_invalid_certificates),
                ca_file_path: c_char_to_string(*ca_file)?.map(PathBuf::from),
                cert_key_file_path: c_char_to_string(*cert_file)?.map(PathBuf::from),
                #[cfg(feature = "openssl-tls")]
                allow_invalid_hostnames: None,
                #[cfg(feature = "cert-key-password")]
                tls_certificate_key_file_password: None,
            }))
        } else {
            Some(Tls::Disabled)
        }
    } else {
        None
    };

    // TODO: Handle these fields when implementing full functionality
    let _compressors = compressors;
    let _load_balanced = load_balanced;
    let _socket_timeout_ms = socket_timeout_ms;
    let _read_preference_mode = read_preference_mode;

    // Build ClientOptions directly
    Ok(ClientOptions {
        hosts: parsed_hosts?,
        app_name: c_char_to_string(*app_name)?,
        repl_set_name: c_char_to_string(*replica_set)?,
        srv_service_name: c_char_to_string(*srv_service_name)?,
        direct_connection: Some(*direct_connection),
        max_pool_size: i32_to_option_u32(*max_pool_size),
        min_pool_size: i32_to_option_u32(*min_pool_size),
        srv_max_hosts: i32_to_option_u32(*srv_max_hosts),
        max_idle_time: i64_to_duration_ms(*max_idle_time_ms),
        connect_timeout: i64_to_duration_ms(*connect_timeout_ms),
        server_selection_timeout: i64_to_duration_ms(*server_selection_timeout_ms),
        local_threshold: i64_to_duration_ms(*local_threshold_ms),
        heartbeat_freq: i64_to_duration_ms(*heartbeat_frequency_ms),
        credential,
        tls,
        ..Default::default()
    })
}

/// Parse an authentication mechanism string.
fn parse_auth_mechanism(mechanism: &str) -> Result<AuthMechanism> {
    match mechanism {
        "SCRAM-SHA-1" => Ok(AuthMechanism::ScramSha1),
        "SCRAM-SHA-256" => Ok(AuthMechanism::ScramSha256),
        "MONGODB-CR" => Ok(AuthMechanism::MongoDbCr),
        "MONGODB-X509" => Ok(AuthMechanism::MongoDbX509),
        "PLAIN" => Ok(AuthMechanism::Plain),
        "MONGODB-OIDC" => Ok(AuthMechanism::MongoDbOidc),
        #[cfg(feature = "gssapi-auth")]
        "GSSAPI" => Ok(AuthMechanism::Gssapi),
        #[cfg(feature = "aws-auth")]
        "MONGODB-AWS" => Ok(AuthMechanism::MongoDbAws),
        _ => Err(crate::error::Error::invalid_argument(format!(
            "Unknown or unsupported authentication mechanism: {}",
            mechanism
        ))),
    }
}
