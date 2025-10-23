pub(crate) use functions::{add_tracing_info, link_to_job_create_span};

#[cfg(not(any(feature = "opentelemetry_0_30", feature = "opentelemetry",)))]
mod functions {
    pub(crate) fn add_tracing_info(_payload: &mut serde_json::Value) {}
    pub(crate) fn link_to_job_create_span(_payload: serde_json::Value) {}
}

#[cfg(any(feature = "opentelemetry_0_30", feature = "opentelemetry",))]
mod functions {
    use opentelemetry::trace::TraceContextExt;
    #[cfg(feature = "opentelemetry_0_30")]
    use opentelemetry_30 as opentelemetry;
    use tracing::Span;
    #[cfg(feature = "opentelemetry_0_30")]
    use tracing_opentelemetry_30 as tracing_opentelemetry;

    #[derive(Debug, serde::Serialize, serde::Deserialize)]
    struct TraceInfo {
        flags: u8,
        trace_id: String,
        span_id: String,
    }

    pub(crate) fn add_tracing_info(payload: &mut serde_json::Value) {
        if let Some(payload) = payload.as_object_mut() {
            use opentelemetry::trace::TraceContextExt;
            use tracing_opentelemetry::OpenTelemetrySpanExt;

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

    pub(crate) fn link_to_job_create_span(payload: serde_json::Value) {
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

        use opentelemetry::trace::{SpanContext, TraceFlags, TraceId, SpanId, TraceState};
        use tracing_opentelemetry::OpenTelemetrySpanExt;

        let trace_id = TraceId::from_hex(&trace_info.trace_id);
        let span_id = SpanId::from_hex(&trace_info.span_id);
        let trace_flags = TraceFlags::new(trace_info.flags);

        let (trace_id, span_id) = match (trace_id, span_id) {
            (Ok(trace_id), Ok(span_id)) => (trace_id, span_id),
            _ => return,
        };

        let span_context = SpanContext::new(
            trace_id,
            span_id,
            trace_flags,
            true,
            TraceState::default(),
        );

        Span::current().add_link(span_context);
    }
}
