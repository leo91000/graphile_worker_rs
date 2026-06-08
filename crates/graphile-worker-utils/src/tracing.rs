pub(crate) use functions::add_tracing_info;

#[cfg(not(any(feature = "opentelemetry_0_30", feature = "opentelemetry_0_31")))]
mod functions {
    pub(crate) fn add_tracing_info(_payload: &mut serde_json::Value) {}
}

#[cfg(any(feature = "opentelemetry_0_30", feature = "opentelemetry_0_31"))]
mod functions {
    #[cfg(feature = "opentelemetry_0_30")]
    use opentelemetry_30 as opentelemetry;
    #[cfg(feature = "opentelemetry_0_30")]
    use tracing_opentelemetry_30 as tracing_opentelemetry;

    use opentelemetry::trace::TraceContextExt;
    use tracing::Span;
    use tracing_opentelemetry::OpenTelemetrySpanExt;

    #[derive(Debug, serde::Serialize, serde::Deserialize)]
    struct TraceInfo {
        flags: u8,
        trace_id: String,
        span_id: String,
    }

    pub(crate) fn add_tracing_info(payload: &mut serde_json::Value) {
        let Some(payload) = payload.as_object_mut() else {
            return;
        };

        let context = Span::current().context();
        let span = context.span();
        let span_context = span.span_context();
        if !span_context.is_valid() {
            return;
        }

        let flags = span_context.trace_flags().to_u8();
        let span_id = span_context.span_id().to_string();
        let trace_id = span_context.trace_id().to_string();

        let value = TraceInfo {
            flags,
            trace_id,
            span_id,
        };

        payload.insert("_trace".into(), serde_json::json!(value));
    }
}
