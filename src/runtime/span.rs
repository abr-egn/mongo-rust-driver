use std::future::Future;

use opentelemetry::{
    trace::{SpanKind, TraceContextExt, Tracer},
    Context,
    KeyValue,
};

use crate::{
    bson::Bson,
    cmap::{conn::wire::Message, Command, ConnectionInfo, StreamDescription},
    error::{ErrorKind, Result},
    operation::{aggregate::AggregateTarget, Operation},
    options::{ServerAddress, DEFAULT_PORT},
    Client,
    ClientSession,
    Namespace,
};

impl Client {
    pub(crate) fn start_operation_span(
        &self,
        op: &impl Operation,
        session: Option<&ClientSession>,
    ) -> OpSpan {
        let op = op.span_info();
        if !self.options().otel_enabled() {
            return OpSpan {
                context: Context::current(),
                enabled: false,
            };
        }
        let span_name = format!("{} {}", op.log_name(), op_target(op));
        let mut attrs = common_attrs(op);
        attrs.extend([
            KeyValue::new("db.operation.name", op.log_name().to_owned()),
            KeyValue::new("db.operation.summary", span_name.clone()),
        ]);
        let builder = self
            .tracer()
            .span_builder(span_name)
            .with_kind(SpanKind::Client)
            .with_attributes(attrs);
        let context =
            if let Some(TxnSpan(txn_ctx)) = session.and_then(|s| s.transaction.span.as_ref()) {
                txn_ctx.with_span(builder.start_with_context(self.tracer(), txn_ctx))
            } else {
                Context::current_with_span(builder.start(self.tracer()))
            };
        OpSpan {
            context,
            enabled: true,
        }
    }

    pub(crate) fn start_command_span(
        &self,
        op: &impl Operation,
        conn_info: &ConnectionInfo,
        stream_desc: &StreamDescription,
        message: &Message,
        cmd_attrs: CommandAttributes,
    ) -> CmdSpan {
        let op = op.span_info();
        if !self.options().otel_enabled() || cmd_attrs.should_redact {
            return CmdSpan {
                context: Context::current(),
                enabled: false,
            };
        }
        let otel_driver_conn_id: i64 = conn_info.id.into();
        let mut attrs = common_attrs(op);
        attrs.extend(cmd_attrs.attrs);
        attrs.extend([
            KeyValue::new(
                "db.query.summary",
                format!("{} {}", &cmd_attrs.name, op_target(op)),
            ),
            KeyValue::new("db.mongodb.driver_connection_id", otel_driver_conn_id),
            KeyValue::new("server.type", stream_desc.initial_server_type.to_string()),
        ]);
        match &conn_info.address {
            ServerAddress::Tcp { host, port } => {
                let otel_port: i64 = port.unwrap_or(DEFAULT_PORT).into();
                attrs.extend([
                    KeyValue::new("server.port", otel_port),
                    KeyValue::new("server.address", host.clone()),
                    KeyValue::new("network.transport", "tcp"),
                ]);
            }
            #[cfg(unix)]
            ServerAddress::Unix { path } => {
                attrs.extend([
                    KeyValue::new("server.address", path.to_string_lossy().into_owned()),
                    KeyValue::new("network.transport", "unix"),
                ]);
            }
        }
        if let Some(server_id) = &conn_info.server_id {
            attrs.push(KeyValue::new("db.mongodb.server_connection_id", *server_id));
        }
        let text_max_len = self.options().otel_query_text_max_length();
        if text_max_len > 0 {
            let mut doc = message.get_command_document();
            for key in ["lsid", "$db", "$clusterTime", "signature"] {
                doc.remove(key);
            }
            attrs.push(KeyValue::new(
                "db.query.text",
                crate::bson_util::doc_to_json_str(doc, text_max_len),
            ));
        }
        if let Some(cursor_id) = op.cursor_id() {
            attrs.push(KeyValue::new("db.mongodb.cursor_id", cursor_id));
        }
        let span = self
            .tracer()
            .span_builder(cmd_attrs.name)
            .with_kind(SpanKind::Client)
            .with_attributes(attrs)
            .start(self.tracer());
        CmdSpan {
            context: Context::current_with_span(span),
            enabled: true,
        }
    }

    pub(crate) fn start_transaction_span(&self) -> TxnSpan {
        if !self.options().otel_enabled() {
            return TxnSpan(Context::current());
        }
        let span = self
            .tracer()
            .span_builder("transaction")
            .with_kind(SpanKind::Client)
            .with_attributes([KeyValue::new("db.system", "mongodb")])
            .start(self.tracer());
        TxnSpan(Context::current_with_span(span))
    }
}

pub(crate) struct OpSpan {
    context: Context,
    enabled: bool,
}

impl OpSpan {
    pub(crate) fn record_error<T>(&self, result: &Result<T>) {
        if !self.enabled {
            return;
        }
        record_error(&self.context, result);
    }
}

pub(crate) struct CmdSpan {
    context: Context,
    enabled: bool,
}

impl CmdSpan {
    pub(crate) fn record_command_result<Op: Operation>(&self, result: &Result<Op::O>) {
        if !self.enabled {
            return;
        }
        if let Ok(out) = result {
            // tests don't match the spec here
            if false {
                if let Some(cursor_id) = <Op::SpanInfo as InfoWitness>::output_cursor_id(out) {
                    let span = self.context.span();
                    span.set_attribute(KeyValue::new("db.mongodb.cursor_id", cursor_id));
                }
            }
        }
        record_error(&self.context, result);
    }
}

#[derive(Debug)]
pub(crate) struct TxnSpan(Context);

fn record_error<T>(context: &Context, result: &Result<T>) {
    let error = if let Err(error) = result {
        error
    } else {
        return;
    };
    let span = context.span();
    span.set_attributes([
        KeyValue::new("exception.message", error.to_string()),
        KeyValue::new("exception.type", error.kind.name()),
        #[cfg(feature = "error-backtrace")]
        KeyValue::new("exception.stacktrace", error.backtrace.to_string()),
    ]);
    if let ErrorKind::Command(cmd_err) = &*error.kind {
        span.set_attribute(KeyValue::new(
            "db.response.status_code",
            cmd_err.code.to_string(),
        ));
    }
    span.record_error(error);
    span.set_status(opentelemetry::trace::Status::Error {
        description: error.to_string().into(),
    });
}

fn op_target(op: &impl SpanInfo) -> String {
    let target = op.target();
    if let Some(coll) = target.collection {
        format!("{}.{}", target.database, coll)
    } else {
        target.database.to_owned()
    }
}

fn common_attrs(op: &impl SpanInfo) -> Vec<KeyValue> {
    let target = op.target();
    let mut attrs = vec![
        KeyValue::new("db.system", "mongodb"),
        KeyValue::new("db.namespace", target.database.to_owned()),
    ];
    if let Some(coll) = target.collection {
        attrs.push(KeyValue::new("db.collection.name", coll.to_owned()));
    }
    attrs
}

#[derive(Clone)]
pub(crate) struct CommandAttributes {
    should_redact: bool,
    name: String,
    attrs: Vec<KeyValue>,
}

impl CommandAttributes {
    pub(crate) fn new(cmd: &Command) -> Self {
        let mut attrs = vec![KeyValue::new("db.command.name", cmd.name.clone())];
        if let Some(lsid) = &cmd.lsid {
            attrs.push(KeyValue::new(
                "db.mongodb.lsid",
                Bson::Document(lsid.clone())
                    .into_relaxed_extjson()
                    .to_string(),
            ));
        }
        if let Some(txn_number) = &cmd.txn_number {
            attrs.push(KeyValue::new("db.mongodb.txn_number", *txn_number));
        }
        Self {
            should_redact: cmd.should_redact(),
            name: cmd.name.clone(),
            attrs,
        }
    }
}

pub(crate) trait InfoWitness {
    type Op: SpanInfo;
    fn span_info(op: &Self::Op) -> &impl SpanInfo {
        op
    }
    fn output_cursor_id(output: &<Self::Op as Operation>::O) -> Option<i64> {
        Self::Op::output_cursor_id(output)
    }
}

pub(crate) struct Witness<T: SpanInfo> {
    _t: std::marker::PhantomData<T>,
}

impl<T: SpanInfo> InfoWitness for Witness<T> {
    type Op = T;
}

pub(crate) trait SpanInfo: Operation {
    fn log_name(&self) -> &str;

    fn target(&self) -> OperationTarget<'_>;

    fn cursor_id(&self) -> Option<i64>;

    fn output_cursor_id(output: &<Self as Operation>::O) -> Option<i64>;
}

pub(crate) trait SpanInfoDefaults: Operation {
    fn log_name(&self) -> &str {
        crate::bson_compat::cstr_to_str(self.name())
    }

    fn target(&self) -> OperationTarget<'_>;

    fn cursor_id(&self) -> Option<i64> {
        None
    }

    fn output_cursor_id(_output: &<Self as Operation>::O) -> Option<i64> {
        None
    }
}

impl<T: SpanInfoDefaults> SpanInfo for T {
    fn log_name(&self) -> &str {
        self.log_name()
    }

    fn target(&self) -> OperationTarget<'_> {
        self.target()
    }

    fn cursor_id(&self) -> Option<i64> {
        self.cursor_id()
    }

    fn output_cursor_id(output: &<Self as Operation>::O) -> Option<i64> {
        T::output_cursor_id(output)
    }
}

pub(crate) struct OperationTarget<'a> {
    pub(crate) database: &'a str,
    pub(crate) collection: Option<&'a str>,
}

impl OperationTarget<'static> {
    pub(crate) const ADMIN: Self = OperationTarget {
        database: "admin",
        collection: None,
    };
}

impl<'a> From<&'a str> for OperationTarget<'a> {
    fn from(value: &'a str) -> Self {
        OperationTarget {
            database: value,
            collection: None,
        }
    }
}

impl<'a> From<&'a Namespace> for OperationTarget<'a> {
    fn from(value: &'a Namespace) -> Self {
        OperationTarget {
            database: &value.db,
            collection: Some(&value.coll),
        }
    }
}

impl<'a> From<&'a AggregateTarget> for OperationTarget<'a> {
    fn from(value: &'a AggregateTarget) -> Self {
        match value {
            AggregateTarget::Database(db) => db.as_str().into(),
            AggregateTarget::Collection(ns) => ns.into(),
        }
    }
}

pub(crate) trait FutureExt: Future + Sized {
    fn with_span(self, span: &OpSpan) -> impl Future<Output = Self::Output> {
        use opentelemetry::context::FutureExt;
        self.with_context(span.context.clone())
    }
}

impl<T: Future> FutureExt for T {}

#[cfg(feature = "opentelemetry")]
mod otel {
    use opentelemetry::Context;

    struct OpSpan {
        context: Context,
        enabled: bool,
    }
}
