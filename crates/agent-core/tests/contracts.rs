use agent_core::{
    Content, ContentBody, Message, Response, ResponseUpdate, Role, Source, Usage,
    content::{Annotation, Field, decode_data_uri},
    filter::MessageFilter,
    tool::{ModelRequest, ProviderCapabilities, RunOptions, Schema, ToolDescriptor},
};
use schemars::JsonSchema;
use serde::{Deserialize, Serialize};
use serde_json::{Value, json};

#[test]
fn response_reference_fixtures() {
    let cases: Vec<Value> = serde_json::from_str(include_str!(
        "../../../tests/conformance/response-cases.json"
    ))
    .unwrap();
    for case in cases {
        let mut response = Response::default();
        let updates: Vec<ResponseUpdate> = serde_json::from_value(case["updates"].clone()).unwrap();
        for update in updates {
            response.update(update);
        }
        response.coalesce();
        let usage = response.usage();
        let actual = json!({"text":response.text(),"message_ids":response.messages.iter().map(|m| &m.id).collect::<Vec<_>>(),
            "finish_reason":response.finish_reason,"continuation_token":response.continuation_token,
            "conversation_id":response.conversation_id,"input_tokens":usage.input_token_count,
            "output_tokens":usage.output_token_count,"created_at":response.created_at.map(|t|t.to_rfc3339())});
        assert_eq!(actual, case["expected"], "{}", case["name"]);
    }
}

#[test]
fn unknown_content_and_annotation_roundtrip() {
    let raw = json!({"Type":"futureContent","nested":{"value":null},"number":9007199254740993_u64});
    let content: Content = serde_json::from_value(raw.clone()).unwrap();
    assert!(matches!(content.body, ContentBody::Raw(_)));
    assert_eq!(serde_json::to_value(content).unwrap(), raw);
    let raw = json!({"Type":"futureCitation","payload":[1,null,"ok"]});
    let annotation: Annotation = serde_json::from_value(raw.clone()).unwrap();
    assert_eq!(serde_json::to_value(annotation).unwrap(), raw);
}

#[test]
fn known_content_retains_extensions_and_annotations() {
    let value = json!({"Type":"text","Text":"hello","vendorFlag":{"enabled":true},"AdditionalProperties":{"tag":1},
        "Annotations":[{"Type":"citation","Title":"source","AnnotatedRegions":[{"Type":"textSpan","StartIndex":null,"EndIndex":4}],"newField":42}]});
    let content: Content = serde_json::from_value(value.clone()).unwrap();
    assert_eq!(serde_json::to_value(content).unwrap(), value);
}

#[test]
fn malformed_known_content_is_not_downgraded_to_raw() {
    for value in [
        json!({"Type":"text","Text":7}),
        json!({"Type":"data","URI":"not-data"}),
        json!({"Type":"toolApprovalResponse","RequestID":"r","Approved":"yes"}),
        json!(5),
    ] {
        assert!(serde_json::from_value::<Content>(value).is_err());
    }
    assert!(serde_json::from_value::<Annotation>(json!({"Type":"citation","Title":42})).is_err());
}

#[test]
fn missing_null_and_value_are_distinct() {
    #[derive(Debug, PartialEq, Serialize, Deserialize)]
    struct Patch {
        #[serde(default, skip_serializing_if = "Field::is_missing")]
        value: Field<String>,
    }
    for (json, expected) in [
        (json!({}), Field::Missing),
        (json!({"value":null}), Field::Null),
        (json!({"value":""}), Field::Value(String::new())),
    ] {
        let patch: Patch = serde_json::from_value(json.clone()).unwrap();
        assert_eq!(patch.value, expected);
        assert_eq!(serde_json::to_value(patch).unwrap(), json);
    }
}

#[test]
fn binary_and_percent_data_uri_roundtrip() {
    let bytes = [0, 127, 128, 255];
    let content = Content::data(&bytes, "application/octet-stream").unwrap();
    let restored: Content =
        serde_json::from_value(serde_json::to_value(&content).unwrap()).unwrap();
    assert_eq!(restored, content);
    if let ContentBody::Data { uri } = restored.body {
        assert_eq!(decode_data_uri(&uri, 4).unwrap().0, bytes);
    }
    assert_eq!(
        decode_data_uri("data:,hello%20world+%FF", 100).unwrap().0,
        b"hello world+\xff"
    );
    assert_eq!(
        decode_data_uri("DATA:;BASE64,", 0).unwrap().0,
        Vec::<u8>::new()
    );
}

#[test]
fn invalid_and_oversized_data_rejected() {
    for uri in [
        "data:no-comma",
        "data:,bad%GG",
        "data:;base64,@@@",
        "data:invalid,abc",
        "http://example.com",
    ] {
        assert!(decode_data_uri(uri, 100).is_err(), "{uri}");
    }
    assert!(decode_data_uri("data:,abcd", 3).is_err());
    assert!(decode_data_uri("data:;base64,YWJjZA==", 3).is_err());
    assert!(Content::data(b"x", "bad mime").is_err());
}

#[test]
fn uri_requires_absolute_reference() {
    assert!(Content::uri("relative/file.png", "image/png").is_err());
    assert!(Content::uri("https://example.com/file.png", "image/png").is_ok());
}

#[test]
fn cloning_messages_isolates_nested_mutations() {
    let original: Message = serde_json::from_value(
        json!({"Role":"vendor-role","Contents":[{"Type":"text","Text":"x","nested":{"a":1}}]}),
    )
    .unwrap();
    let mut clone = original.with_source(Source {
        id: "run".into(),
        kind: "agent".into(),
    });
    clone.contents[0].extra["nested"]["a"] = json!(2);
    assert_eq!(original.contents[0].extra["nested"]["a"], 1);
    assert_eq!(original.role, Role("vendor-role".into()));
    assert!(original.source.kind.is_empty());
}

#[test]
fn filters_preserve_input_positions_and_union_order() {
    let external = Message::user("same");
    let internal = external.with_source(Source {
        id: "1".into(),
        kind: "agent".into(),
    });
    let messages = [external.clone(), internal, external];
    let filter = MessageFilter::Or(vec![
        MessageFilter::ExternalOnly,
        MessageFilter::Sources(vec![Source::default()]),
    ]);
    let selected = filter.apply(&messages);
    assert_eq!(selected.len(), 2);
    assert!(std::ptr::eq(selected[0], &messages[0]));
    assert!(std::ptr::eq(selected[1], &messages[2]));
    assert!(MessageFilter::Or(vec![]).apply(&messages).is_empty());
    assert_eq!(MessageFilter::And(vec![]).apply(&messages).len(), 3);
}

#[test]
fn metadata_scope_and_late_values_match_reference() {
    let mut response = Response::default();
    for update in [
        json!({"MessageID":"m","AdditionalProperties":{"same":1},"AgentID":"a"}),
        json!({"MessageID":"m","AdditionalProperties":{"same":2},"AgentID":"b"}),
        json!({"AdditionalProperties":{"response":true}}),
    ] {
        response.update(serde_json::from_value(update).unwrap());
    }
    assert_eq!(response.agent_id, "b");
    assert_eq!(response.messages.len(), 1);
    assert_eq!(
        response.messages[0].additional_properties,
        json!({"same":2}).as_object().unwrap().clone()
    );
    assert_eq!(
        response.additional_properties,
        json!({"response":true}).as_object().unwrap().clone()
    );
}

#[test]
fn coalescing_keeps_reasoning_separate_and_annotations_intact() {
    let annotated: Content = serde_json::from_value(json!({"Type":"text","Text":"citation","Annotations":[{"Type":"citation","Title":"source"}]})).unwrap();
    let mut response = Response::default();
    response.update(ResponseUpdate {
        contents: vec![
            Content::text("a"),
            Content::text("b"),
            Content::reasoning("r1"),
            Content::reasoning("r2"),
            annotated.clone(),
            Content::text("z"),
        ],
        ..ResponseUpdate::default()
    });
    response.coalesce();
    assert_eq!(response.messages[0].contents.len(), 4);
    assert_eq!(response.messages[0].contents[2], annotated);
    assert_eq!(response.text(), "abcitationz");
}

#[test]
fn identical_annotations_and_protected_reasoning_do_not_merge() {
    for content in [
        json!({"Type":"text","Text":"x","Annotations":[{"Type":"citation","Title":"same"}]}),
        json!({"Type":"reasoning","Text":"r","ProtectedData":"opaque"}),
    ] {
        let content: Content = serde_json::from_value(content).unwrap();
        let mut response = Response::default();
        response.update(ResponseUpdate {
            contents: vec![content.clone(), content],
            ..ResponseUpdate::default()
        });
        response.coalesce();
        assert_eq!(response.messages[0].contents.len(), 2);
    }
}

#[test]
fn usage_adds_additional_counts_without_guessing_total() {
    let mut usage = Usage {
        input_token_count: 1,
        additional_counts: [("cached".into(), 3)].into(),
        ..Usage::default()
    };
    usage.add(&Usage {
        output_token_count: 2,
        additional_counts: [("cached".into(), 4)].into(),
        ..Usage::default()
    });
    assert_eq!(usage.additional_counts["cached"], 7);
    assert_eq!(usage.total_token_count, 0);
}

#[derive(Debug, PartialEq, Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
struct Arguments {
    city: String,
    count: u32,
}

#[test]
fn schema_generation_and_validation_reject_bad_arguments() {
    let schema = Schema::for_type::<Arguments>().unwrap();
    let valid = json!({"city":"London","count":1});
    assert_eq!(schema.decode::<Arguments>(valid).unwrap().count, 1);
    for bad in [
        json!({"city":"London"}),
        json!({"city":"London","count":-1}),
        json!({"city":"London","count":1,"extra":true}),
    ] {
        assert!(schema.decode::<Arguments>(bad).is_err());
    }
    assert!(Schema::new(json!({"type":42})).is_err());
    assert!(Schema::new(json!({"$ref":"https://example.invalid/schema.json"})).is_err());
    assert!(Schema::new(json!({"$ref":"file:///tmp/schema.json"})).is_err());
    assert!(ToolDescriptor::typed::<Arguments>(" ", "blank").is_err());
}

#[test]
fn capabilities_fail_explicitly_and_zero_is_not_default() {
    let request = ModelRequest {
        options: RunOptions {
            stream: true,
            ..RunOptions::default()
        },
        ..ModelRequest::default()
    };
    assert_eq!(
        ProviderCapabilities::default()
            .validate(&request)
            .unwrap_err()
            .kind,
        agent_core::ErrorKind::UnsupportedCapability
    );
    assert_eq!(RunOptions::default().effective_tool_iterations(), 40);
    assert_eq!(
        RunOptions {
            max_tool_iterations: Some(0),
            ..RunOptions::default()
        }
        .effective_tool_iterations(),
        0
    );
}
