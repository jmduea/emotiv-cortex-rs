use serde_json::Value;

use super::fixtures::ContractStep;

pub(super) fn rpc_id(request: &Value) -> u64 {
    match request.get("id").and_then(Value::as_u64) {
        Some(id) => id,
        None => panic!("request missing numeric id: {request}"),
    }
}

pub(super) fn assert_request_matches_step(request: &Value, step: &ContractStep) {
    let method = request.get("method").and_then(Value::as_str);
    assert_eq!(
        method,
        Some(step.method),
        "unexpected method for {}::{}",
        step.domain,
        step.name
    );

    let params = match request.get("params").and_then(Value::as_object) {
        Some(params) => params,
        None => panic!(
            "request missing params object for {}::{}",
            step.domain, step.name
        ),
    };

    let expected = match step.expected_params.as_object() {
        Some(expected) => expected,
        None => panic!(
            "expected_params must be an object for {}::{}",
            step.domain, step.name
        ),
    };

    for (key, expected_value) in expected {
        assert_eq!(
            params.get(key),
            Some(expected_value),
            "mismatch in {}::{} param '{}'",
            step.domain,
            step.name,
            key
        );
    }

    for key in &step.absent_params {
        assert!(
            !params.contains_key(*key),
            "{}::{} unexpectedly included param '{}'",
            step.domain,
            step.name,
            key
        );
    }
}
