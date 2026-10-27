use agent_core::{
    Content, ContentBody, Message, Response, ResponseUpdate,
    content::Field,
    id::{AgentId, CallId},
    raw::RawRepresentations,
    strict_schema::openai_strict,
    tool::{
        ModelRequest, ProviderCapabilities, RunOptions, RunOptionsPatch, ToolDescriptor, ToolMode,
    },
};
use serde_json::{Value, json};

#[test]
fn content_reference_fixtures() {
    let cases: Vec<Value> = serde_json::from_str(include_str!(
        "../../../tests/conformance/content-cases.json"
    ))
    .unwrap();
    for case in cases {
        let contents: Vec<Content> = serde_json::from_value(case["input"].clone()).unwrap();
        let output = agent_core::coalesce::coalesce(contents);
        assert_eq!(
            serde_json::to_value(output).unwrap(),
            case["expected"],
            "{}",
            case["name"]
        );
    }
}

#[test]
fn malformed_native_payloads_and_noncall_approvals_rejected() {
    for value in [
        json!({"Type":"mcpServerToolCall","CallID":"x","Name":"n","Arguments":{}}),
        json!({"Type":"codeInterpreterToolCall","CallID":"x","Inputs":4}),
        json!({"Type":"toolApprovalRequest","RequestID":"r","ToolCall":{"Type":"text","Text":"not a call"}}),
        json!({"Type":"alwaysApproveToolApprovalResponse","innerResponse":{"Type":"text","Text":"no"}}),
    ] {
        assert!(serde_json::from_value::<Content>(value).is_err());
    }
    let request: Content = serde_json::from_value(
        json!({"Type":"toolApprovalRequest","RequestID":"r","ToolCall":null}),
    )
    .unwrap();
    assert!(matches!(
        request.body,
        ContentBody::ApprovalRequest {
            tool_call: None,
            ..
        }
    ));
}

#[test]
fn response_projection_preserves_property_scopes_and_raw_objects() {
    let mut response = Response::default();
    let mut raw = RawRepresentations::default();
    raw.push("secret SDK object".to_owned());
    response.update(ResponseUpdate {
        message_id: "m".into(),
        raw: raw.clone(),
        contents: vec![Content::text("x")],
        additional_properties: json!({"message":1}).as_object().unwrap().clone(),
        ..Default::default()
    });
    response.update(ResponseUpdate {
        additional_properties: json!({"response":2}).as_object().unwrap().clone(),
        continuation_token: "pending".into(),
        ..Default::default()
    });
    let updates = response.to_updates();
    assert_eq!(updates.len(), 2);
    assert!(updates[1].message_id.is_empty());
    assert_eq!(
        updates[0].raw.get::<String>(0).unwrap(),
        "secret SDK object"
    );
    assert!(!format!("{:?}", response.raw).contains("secret"));
    assert!(!serde_json::to_string(&response).unwrap().contains("secret"));
    let mut rebuilt = Response::default();
    for u in updates {
        rebuilt.update(u);
    }
    assert_eq!(rebuilt, response);
    assert!(
        serde_json::from_str::<Response>(&serde_json::to_string(&response).unwrap())
            .unwrap()
            .raw
            .is_empty()
    );
}

#[test]
fn response_projection_empty_and_metadata_only_edges() {
    let response = Response {
        finish_reason: "stop".into(),
        ..Default::default()
    };
    assert!(response.to_updates().is_empty());
    let response = Response {
        continuation_token: "pending".into(),
        ..Default::default()
    };
    assert_eq!(response.to_updates().len(), 1);
    let created = "2026-10-27T12:00:00Z".parse().unwrap();
    let mut response = Response {
        created_at: Some(created),
        messages: vec![Message::default()],
        ..Default::default()
    };
    assert_eq!(response.to_updates()[0].created_at, Some(created));
    response.messages[0].created_at = Some("0001-01-01T00:00:00Z".parse().unwrap());
    assert_eq!(response.to_updates()[0].created_at, Some(created));
    response.messages[0].created_at = Some(chrono::DateTime::UNIX_EPOCH);
    assert_eq!(
        response.to_updates()[0].created_at,
        Some(chrono::DateTime::UNIX_EPOCH)
    );
}

#[test]
fn options_layering_preserves_false_zero_empty_and_clear() {
    let mut options = RunOptions {
        stream: true,
        instructions: Some("base".into()),
        max_tool_iterations: Some(9),
        ..Default::default()
    };
    options.overlay(&RunOptionsPatch {
        stream: Some(false),
        instructions: Field::Value(String::new()),
        max_tool_iterations: Field::Value(0),
        ..Default::default()
    });
    options.overlay(&RunOptionsPatch::default());
    assert!(!options.stream);
    assert_eq!(options.instructions.as_deref(), Some(""));
    assert_eq!(options.effective_tool_iterations(), 0);
    options.overlay(&RunOptionsPatch {
        instructions: Field::Null,
        max_tool_iterations: Field::Null,
        ..Default::default()
    });
    assert!(options.instructions.is_none());
    assert_eq!(options.effective_tool_iterations(), 40);
}

#[test]
fn identifiers_validate_on_construction_and_deserialization() {
    for invalid in ["", "  ", "a\n"] {
        assert!(AgentId::new(invalid).is_err());
        assert!(serde_json::from_value::<CallId>(json!(invalid)).is_err());
    }
    let id = AgentId::new("agent-1").unwrap();
    assert_eq!(serde_json::to_value(&id).unwrap(), "agent-1");
    assert_eq!(id.as_str(), "agent-1");
}

#[test]
fn tool_modes_duplicates_and_modalities_are_checked() {
    let caps = ProviderCapabilities {
        tools: true,
        ..Default::default()
    };
    let tool = ToolDescriptor {
        name: "a".into(),
        description: String::new(),
        input_schema: json!({"type":"object"}),
        approval_required: false,
    };
    let mut request = ModelRequest {
        tools: vec![tool.clone()],
        options: RunOptions {
            tool_mode: ToolMode::Named("a".into()),
            ..Default::default()
        },
        ..Default::default()
    };
    assert!(caps.validate(&request).is_ok());
    request.options.tool_mode = ToolMode::Named("missing".into());
    assert!(caps.validate(&request).is_err());
    request.options.tool_mode = ToolMode::Auto;
    request.tools.push(tool);
    assert!(caps.validate(&request).is_err());
    let request = ModelRequest {
        messages: vec![Message {
            contents: vec![Content::data(b"image", "image/png").unwrap()],
            ..Default::default()
        }],
        ..Default::default()
    };
    assert!(caps.validate(&request).is_err());
    assert!(
        ProviderCapabilities {
            images: true,
            ..caps
        }
        .validate(&request)
        .is_ok()
    );
}

#[test]
fn strict_schema_transforms_recursively_without_mutating_input() {
    let input = json!({"type":"object","properties":{"name":{"type":["string","null"],"default":null},"items":{"type":"array","items":{"type":"object","properties":{"x":{"type":"integer"}}}}},"required":["name"],"$defs":{"T":{"type":"object","properties":{"enabled":{"type":"boolean"}}}}});
    let original = input.clone();
    let result = openai_strict(&input).unwrap();
    assert_eq!(input, original);
    assert_eq!(result["required"], json!(["name", "items"]));
    assert_eq!(result["additionalProperties"], false);
    assert_eq!(
        result["properties"]["items"]["items"]["required"],
        json!(["x"])
    );
    assert_eq!(result["$defs"]["T"]["additionalProperties"], false);
    assert_eq!(
        result["properties"]["name"]["description"],
        "Default value: null"
    );
    assert!(result["properties"]["name"].get("default").is_none());
}

#[test]
fn strict_schema_rejects_unsupported_shapes_and_preserves_required_order() {
    for input in [
        json!(true),
        json!({"type":"string"}),
        json!({"type":"object"}),
        json!({"type":"object","properties":{"x":true}}),
        json!({"type":"object","additionalProperties":true}),
        json!({"type":"object","additionalProperties":false,"anyOf":[]}),
        json!({"type":"object","properties":{},"required":["x"]}),
        json!({"type":"object","additionalProperties":false,"allOf":[]}),
    ] {
        assert!(openai_strict(&input).is_err(), "{input}");
    }
    let value=openai_strict(&json!({"type":["object"],"properties":{"b":{"type":"string"},"a":{"type":"string"}},"required":["b","b"]})).unwrap();
    assert_eq!(value["required"], json!(["b", "a"]));
}
