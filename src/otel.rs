//! Support for OpenTelemetry.

use std::sync::{Arc, LazyLock};

use derive_where::derive_where;

use opentelemetry::{
    global::{BoxedTracer, ObjectSafeTracerProvider},
    trace::{Tracer, TracerProvider},
};

use crate::options::ClientOptions;

#[cfg(test)]
pub(crate) mod testing;

/// Configuration for OpenTelemetry.
#[derive(Clone, serde::Deserialize, typed_builder::TypedBuilder)]
#[derive_where(Debug, PartialEq)]
#[builder(field_defaults(default, setter(into)))]
#[serde(rename_all = "camelCase")]
#[non_exhaustive]
pub struct OpentelemetryOptions {
    /// Enables or disables OpenTelemtry for this client instance.  If unset, will use the value of
    /// the `OTEL_RUST_INSTRUMENTATION_MONGODB_ENABLED` environment variable.
    pub enabled: Option<bool>,
    /// Maximum length of the `db.query.text` attribute of command spans.  If unset, will use the
    /// value of the `OTEL_RUST_INSTRUMENTATION_MONGODB_QUERY_TEXT_MAX_LENGTH` environment
    /// variable.
    pub query_text_max_length: Option<usize>,
    /// Tracer provider to use.  If unset, will use the global instance.
    #[serde(skip)]
    #[derive_where(skip)]
    #[builder(
        setter(
            fn transform<S, T, P>(provider: P) -> Option<Arc<dyn ObjectSafeTracerProvider + Send + Sync>>
                where
                    S: opentelemetry::trace::Span + Send + Sync + 'static,
                    T: Tracer<Span = S> + Send + Sync + 'static,
                    P: TracerProvider<Tracer = T> + Send + Sync + 'static,
            {
                Some(Arc::new(provider))
            },
        )
    )]
    pub tracer_provider: Option<Arc<dyn ObjectSafeTracerProvider + Send + Sync>>,
}

impl ClientOptions {
    pub(crate) fn tracer(&self) -> BoxedTracer {
        let provider: &dyn ObjectSafeTracerProvider = match self
            .tracing
            .as_ref()
            .and_then(|t| t.tracer_provider.as_ref())
        {
            Some(provider) => &**provider,
            None => &opentelemetry::global::tracer_provider(),
        };
        BoxedTracer::new(
            provider.boxed_tracer(
                opentelemetry::InstrumentationScope::builder("mongodb")
                    .with_version(env!("CARGO_PKG_VERSION"))
                    .build(),
            ),
        )
    }

    pub(crate) fn otel_enabled(&self) -> bool {
        static ENABLED_ENV: LazyLock<bool> = LazyLock::new(|| {
            match std::env::var("OTEL_RUST_INSTRUMENTATION_MONGODB_ENABLED").as_deref() {
                Ok("1" | "true" | "yes") => true,
                _ => false,
            }
        });

        self.tracing
            .as_ref()
            .and_then(|t| t.enabled)
            .unwrap_or_else(|| *ENABLED_ENV)
    }

    pub(crate) fn otel_query_text_max_length(&self) -> usize {
        static MAX_LENGTH_ENV: LazyLock<usize> = LazyLock::new(|| {
            std::env::var("OTEL_RUST_INSTRUMENTATION_MONGODB_QUERY_TEXT_MAX_LENGTH")
                .ok()
                .and_then(|s| s.parse().ok())
                .unwrap_or(0)
        });

        self.tracing
            .as_ref()
            .and_then(|t| t.query_text_max_length)
            .unwrap_or_else(|| *MAX_LENGTH_ENV)
    }
}
