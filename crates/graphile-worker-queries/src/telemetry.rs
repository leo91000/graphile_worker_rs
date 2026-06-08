pub(crate) use functions::current_trace_info;

#[derive(Clone, Debug, Eq, PartialEq, serde::Deserialize, serde::Serialize)]
pub(crate) struct TraceInfo {
    pub(crate) flags: u8,
    pub(crate) trace_id: String,
    pub(crate) span_id: String,
}

pub(crate) fn add_tracing_info_with_trace(payload: &mut serde_json::Value, trace_info: &TraceInfo) {
    match payload {
        serde_json::Value::Object(payload) => {
            payload.insert("_trace".into(), serde_json::json!(trace_info));
        }
        serde_json::Value::Array(items) => {
            for item in items {
                if item.is_object() {
                    add_tracing_info_with_trace(item, trace_info);
                }
            }
        }
        _ => {}
    }
}

#[cfg(not(any(feature = "opentelemetry_0_30", feature = "opentelemetry_0_31")))]
mod functions {
    use super::TraceInfo;

    pub(crate) fn current_trace_info() -> Option<TraceInfo> {
        None
    }
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

    use super::TraceInfo;

    pub(crate) fn current_trace_info() -> Option<TraceInfo> {
        let context = Span::current().context();
        let span = context.span();
        let span_context = span.span_context();
        if !span_context.is_valid() {
            return None;
        }

        Some(TraceInfo {
            flags: span_context.trace_flags().to_u8(),
            trace_id: span_context.trace_id().to_string(),
            span_id: span_context.span_id().to_string(),
        })
    }
}

#[cfg(test)]
mod tests {
    use serde_json::json;

    use super::{add_tracing_info_with_trace, TraceInfo};

    fn trace_info() -> TraceInfo {
        TraceInfo {
            flags: 1,
            trace_id: "4bf92f3577b34da6a3ce929d0e0e4736".to_string(),
            span_id: "00f067aa0ba902b7".to_string(),
        }
    }

    #[test]
    fn adds_trace_to_object_payload() {
        let trace_info = trace_info();
        let mut payload = json!({ "value": 1 });

        add_tracing_info_with_trace(&mut payload, &trace_info);

        assert_eq!(payload["_trace"], json!(trace_info));
    }

    #[test]
    fn overwrites_existing_trace_payload() {
        let trace_info = trace_info();
        let mut payload = json!({
            "value": 1,
            "_trace": {
                "flags": 0,
                "trace_id": "old",
                "span_id": "old"
            }
        });

        add_tracing_info_with_trace(&mut payload, &trace_info);

        assert_eq!(payload["_trace"], json!(trace_info));
    }

    #[test]
    fn adds_trace_to_object_array_items_only() {
        let trace_info = trace_info();
        let mut payload = json!([
            { "value": 1 },
            "skip",
            null,
            { "value": 2 }
        ]);

        add_tracing_info_with_trace(&mut payload, &trace_info);

        assert_eq!(payload[0]["_trace"], json!(trace_info));
        assert!(payload[1].get("_trace").is_none());
        assert!(payload[2].get("_trace").is_none());
        assert_eq!(payload[3]["_trace"], json!(trace_info));
    }

    #[test]
    fn leaves_scalar_payload_unchanged() {
        let trace_info = trace_info();
        let mut payload = json!("value");

        add_tracing_info_with_trace(&mut payload, &trace_info);

        assert_eq!(payload, json!("value"));
    }
}
