//! FFI client implementation.
//!
//! This module provides the C-compatible API for creating and destroying MongoDB clients.

use std::{path::PathBuf, sync::Arc};

use tokio::runtime::Runtime;

use crate::{
    client::auth::Credential,
    error::Result,
    options::{ClientOptions, ServerAddress, Tls, TlsOptions},
    Client,
};

use super::{
    error::Error,
    runtime::acquire_runtime,
    types::{AuthSettings, ConnectionSettings, TlsSettings},
    utils::{
        c_char_to_string,
        i32_to_option_u32,
        i64_to_duration_ms,
        parse_auth_mechanism,
        parse_hosts,
        parse_read_preference_mode,
    },
};

#[cfg(any(
    feature = "zstd-compression",
    feature = "zlib-compression",
    feature = "snappy-compression"
))]
use super::utils::parse_compressors;

/// Opaque pointer type for MongoClient.
///
/// This wraps the Rust Client along with a reference to the shared global Tokio runtime.
pub struct MongoClient {
    #[allow(dead_code)] // Will be used when operations are implemented
    client: Client,
    #[allow(dead_code)] // Will be used when operations are implemented
    runtime: Arc<Runtime>,
}

/// Create a new MongoClient. Returns pointer on success, null on error.
///
/// # Safety
///
/// - `connection_settings` must be a valid pointer to a ConnectionSettings struct
/// - `auth_settings` can be null or a valid pointer to an AuthSettings struct
/// - `tls_settings` can be null or a valid pointer to a TlsSettings struct
/// - `error_out` can be null or a valid pointer to store error information
/// - All C string pointers in the settings structs must be valid null-terminated strings
///
/// If the function returns null and `error_out` is not null, `*error_out` will be set to
/// a pointer to an Error that must be freed with `error_free()`.
#[no_mangle]
pub unsafe extern "C" fn mongo_client_new(
    connection_settings: *const ConnectionSettings,
    auth_settings: *const AuthSettings,
    tls_settings: *const TlsSettings,
    error_out: *mut *mut Error,
) -> *mut MongoClient {
    let result = build_client_options(connection_settings, auth_settings, tls_settings);

    match result {
        Ok(options) => {
            let runtime = acquire_runtime();

            // Client::with_options spawns tasks, so it needs a runtime context
            let _guard = runtime.enter();

            match Client::with_options(options) {
                Ok(client) => {
                    let inner = MongoClient { client, runtime };
                    Box::into_raw(Box::new(inner)) as *mut MongoClient
                }
                Err(e) => {
                    if !error_out.is_null() {
                        *error_out = Box::into_raw(Error::from_error(&e));
                    }
                    std::ptr::null_mut()
                }
            }
        }
        Err(e) => {
            if !error_out.is_null() {
                *error_out = Box::into_raw(Error::from_error(&e));
            }
            std::ptr::null_mut()
        }
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
    drop(Box::from_raw(client));
}

/// Build ClientOptions from FFI settings.
///
/// # Safety
///
/// All pointers must be valid as described in `mongo_client_new`.
unsafe fn build_client_options(
    connection_settings: *const ConnectionSettings,
    auth_settings: *const AuthSettings,
    tls_settings: *const TlsSettings,
) -> Result<ClientOptions> {
    if connection_settings.is_null() {
        return Err(crate::error::Error::invalid_argument(
            "connection_settings cannot be null",
        ));
    }

    // Fully destructure ConnectionSettings to ensure all fields are handled
    let ConnectionSettings {
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

    let host_strings = parse_hosts(*hosts)?;
    let parsed_hosts: Result<Vec<ServerAddress>> = host_strings
        .iter()
        .map(|h| ServerAddress::parse(h))
        .collect();

    let credential = if !auth_settings.is_null() {
        // Fully destructure AuthSettings to ensure all fields are handled
        let AuthSettings {
            mechanism,
            username,
            password,
            source,
        } = &*auth_settings;

        Some(Credential {
            username: c_char_to_string(*username)?,
            source: c_char_to_string(*source)?,
            password: c_char_to_string(*password)?,
            mechanism: parse_auth_mechanism(*mechanism)?,
            mechanism_properties: None,
            oidc_callback: Default::default(),
        })
    } else {
        None
    };

    let tls = if !tls_settings.is_null() {
        // Fully destructure TlsSettings to ensure all fields are handled
        let TlsSettings {
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

    #[cfg(any(
        feature = "zstd-compression",
        feature = "zlib-compression",
        feature = "snappy-compression"
    ))]
    let compressors_parsed = parse_compressors(*compressors)?;
    #[cfg(not(any(
        feature = "zstd-compression",
        feature = "zlib-compression",
        feature = "snappy-compression"
    )))]
    let _compressors = compressors;

    let selection_criteria = parse_read_preference_mode(*read_preference_mode)?
        .map(crate::selection_criteria::SelectionCriteria::ReadPreference);

    // socket_timeout is deprecated and not supported in ClientOptions, so we ignore it
    let _socket_timeout_ms = socket_timeout_ms;

    Ok(ClientOptions {
        hosts: parsed_hosts?,
        app_name: c_char_to_string(*app_name)?,
        repl_set_name: c_char_to_string(*replica_set)?,
        srv_service_name: c_char_to_string(*srv_service_name)?,
        direct_connection: Some(*direct_connection),
        load_balanced: Some(*load_balanced),
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
        selection_criteria,
        #[cfg(any(
            feature = "zstd-compression",
            feature = "zlib-compression",
            feature = "snappy-compression"
        ))]
        compressors: compressors_parsed,
        ..Default::default()
    })
}
