pub(crate) use functions::link_to_job_create_span;

#[cfg(any(
    all(feature = "opentelemetry_0_30", feature = "opentelemetry_0_31"),
    all(feature = "opentelemetry_0_30", feature = "opentelemetry_0_32"),
    all(feature = "opentelemetry_0_31", feature = "opentelemetry_0_32")
))]
compile_error!(
    "Only one OpenTelemetry compatibility feature can be enabled at a time: \
     opentelemetry_0_30, opentelemetry_0_31, or opentelemetry_0_32"
);

#[cfg(not(any(
    feature = "opentelemetry_0_30",
    feature = "opentelemetry_0_31",
    feature = "opentelemetry_0_32"
)))]
mod functions {
    pub(crate) fn link_to_job_create_span(_payload: &serde_json::Value) {}
}

#[cfg(any(
    feature = "opentelemetry_0_30",
    feature = "opentelemetry_0_31",
    feature = "opentelemetry_0_32"
))]
mod functions {
    #[cfg(feature = "opentelemetry_0_30")]
    use opentelemetry_30 as opentelemetry;
    #[cfg(all(not(feature = "opentelemetry_0_30"), feature = "opentelemetry_0_32"))]
    use opentelemetry_32 as opentelemetry;
    #[cfg(feature = "opentelemetry_0_30")]
    use tracing_opentelemetry_30 as tracing_opentelemetry;
    #[cfg(all(not(feature = "opentelemetry_0_30"), feature = "opentelemetry_0_32"))]
    use tracing_opentelemetry_32 as tracing_opentelemetry;

    use tracing::Span;

    #[derive(Debug, serde::Serialize, serde::Deserialize)]
    struct TraceInfo {
        flags: u8,
        trace_id: String,
        span_id: String,
    }

    pub(crate) fn link_to_job_create_span(payload: &serde_json::Value) {
        let obj = match payload.as_object() {
            Some(obj) => obj,
            None => return,
        };
        let trace_value = match obj.get("_trace") {
            Some(value) => value,
            None => return,
        };
        let trace_info = match serde_json::from_value::<TraceInfo>(trace_value.clone()) {
            Ok(info) => info,
            Err(_) => return,
        };

        use opentelemetry::trace::{SpanContext, SpanId, TraceFlags, TraceId, TraceState};
        use tracing_opentelemetry::OpenTelemetrySpanExt;

        let trace_id = TraceId::from_hex(&trace_info.trace_id);
        let span_id = SpanId::from_hex(&trace_info.span_id);
        let trace_flags = TraceFlags::new(trace_info.flags);

        let (trace_id, span_id) = match (trace_id, span_id) {
            (Ok(trace_id), Ok(span_id)) => (trace_id, span_id),
            _ => return,
        };

        let span_context =
            SpanContext::new(trace_id, span_id, trace_flags, true, TraceState::default());

        Span::current().add_link(span_context);
    }
}
