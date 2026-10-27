//! OpenAI/Foundry strict schema transform pinned to the local Go baseline.
//! This is a schema adapter, not an assertion of live service compatibility.
use crate::AgentError;
use serde_json::{Value, json};

const UNSUPPORTED: &[&str] = &[
    "$anchor",
    "$dynamicAnchor",
    "$dynamicRef",
    "$recursiveAnchor",
    "$recursiveRef",
    "allOf",
    "contains",
    "contentEncoding",
    "contentMediaType",
    "contentSchema",
    "dependentRequired",
    "dependentSchemas",
    "dependencies",
    "else",
    "if",
    "maxContains",
    "maxProperties",
    "minContains",
    "minProperties",
    "not",
    "patternProperties",
    "prefixItems",
    "propertyNames",
    "then",
    "unevaluatedItems",
    "unevaluatedProperties",
    "uniqueItems",
];

pub fn openai_strict(schema: &Value) -> Result<Value, AgentError> {
    let mut value = schema.clone();
    let root = value
        .as_object_mut()
        .ok_or_else(|| AgentError::validation("strict root must be an object"))?;
    if root.get("type") == Some(&json!(["object"])) {
        root.insert("type".into(), json!("object"));
    }
    if root.get("type") != Some(&json!("object")) || root.contains_key("anyOf") {
        return Err(AgentError::validation(
            "strict root must have object type and no anyOf",
        ));
    }
    transform(&mut value, 0)?;
    Ok(value)
}

fn transform(value: &mut Value, depth: usize) -> Result<(), AgentError> {
    if depth > 128 {
        return Err(AgentError::validation("schema nesting exceeds limit"));
    }
    let schema = value
        .as_object_mut()
        .ok_or_else(|| AgentError::validation("boolean/non-object schemas are unsupported"))?;
    for key in UNSUPPORTED {
        if schema.contains_key(*key) {
            return Err(AgentError::validation(format!(
                "unsupported strict keyword: {key}"
            )));
        }
    }
    let property_names = match schema.get("properties") {
        Some(v) => Some(
            v.as_object()
                .ok_or_else(|| AgentError::validation("properties must be an object"))?
                .keys()
                .cloned()
                .collect::<Vec<_>>(),
        ),
        None => None,
    };
    let mut required = Vec::new();
    if let Some(value) = schema.get("required") {
        for name in value
            .as_array()
            .ok_or_else(|| AgentError::validation("required must be an array"))?
        {
            let name = name
                .as_str()
                .ok_or_else(|| AgentError::validation("required entry must be string"))?;
            if !property_names
                .as_ref()
                .is_some_and(|names| names.iter().any(|n| n == name))
            {
                return Err(AgentError::validation("required property is not declared"));
            }
            if !required.contains(&json!(name)) {
                required.push(json!(name));
            }
        }
    }
    if let Some(additional) = schema.get("additionalProperties")
        && additional != &Value::Bool(false)
    {
        return Err(AgentError::validation("additionalProperties must be false"));
    }
    let object = schema.get("type").is_some_and(|t| {
        t == "object" || t.as_array().is_some_and(|a| a.contains(&json!("object")))
    });
    if object
        && property_names.as_ref().is_none_or(Vec::is_empty)
        && !schema.contains_key("additionalProperties")
    {
        return Err(AgentError::validation(
            "empty object must explicitly prohibit additional properties",
        ));
    }
    for key in ["properties", "$defs", "definitions"] {
        if let Some(values) = schema.get_mut(key) {
            for child in values
                .as_object_mut()
                .ok_or_else(|| AgentError::validation("schema map must be object"))?
                .values_mut()
            {
                transform(child, depth + 1)?;
            }
        }
    }
    if let Some(items) = schema.get_mut("items") {
        transform(items, depth + 1)?;
    }
    for key in ["anyOf", "oneOf"] {
        if let Some(values) = schema.get_mut(key) {
            for child in values
                .as_array_mut()
                .ok_or_else(|| AgentError::validation("union must be array"))?
            {
                transform(child, depth + 1)?;
            }
        }
    }
    if let Some(names) = property_names {
        for name in names {
            if !required.contains(&json!(name)) {
                required.push(json!(name));
            }
        }
        schema.insert("required".into(), Value::Array(required));
        schema.insert("additionalProperties".into(), Value::Bool(false));
    }
    if let Some(default) = schema.remove("default") {
        let default = format!("Default value: {default}");
        let description = match schema.get("description") {
            Some(Value::String(s)) => format!("{s} ({default})"),
            Some(Value::Null) | None => default,
            _ => return Err(AgentError::validation("description must be string")),
        };
        schema.insert("description".into(), Value::String(description));
    }
    Ok(())
}
