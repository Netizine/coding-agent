// Adapted from Microsoft Agent Framework Go message/content.go (MIT).
use crate::{
    Content, ContentBody,
    content::{ContentHeader, decode_data_uri},
};
use std::collections::BTreeMap;

/// Coalesce in the reference's pass order; annotations prevent adjacent merges.
pub fn coalesce(mut contents: Vec<Content>) -> Vec<Content> {
    contents = adjacent(contents, 0);
    contents = adjacent(contents, 1);
    let mut images = BTreeMap::new();
    let mut image_output: Vec<Option<Content>> = Vec::new();
    for content in contents {
        if let ContentBody::ImageResult { call_id, .. } = &content.body {
            if let Some(&index) = images.get(call_id) {
                image_output[index] = Some(content);
                continue;
            }
            images.insert(call_id.clone(), image_output.len());
        }
        image_output.push(Some(content));
    }
    contents = image_output.into_iter().flatten().collect();
    let mut calls = BTreeMap::new();
    let mut web_output: Vec<Content> = Vec::new();
    for content in contents {
        if let ContentBody::WebCall { call_id, queries } = &content.body {
            if let Some(&index) = calls.get(call_id) {
                let first: &mut Content = &mut web_output[index];
                if let ContentBody::WebCall { queries: old, .. } = &mut first.body {
                    old.extend(queries.iter().cloned());
                }
                if first.header.raw.is_empty() {
                    first.header.raw = content.header.raw;
                }
                if first.header.annotations.is_empty() {
                    first.header.annotations = content.header.annotations;
                }
                if first.header.additional_properties.is_empty() {
                    first.header.additional_properties = content.header.additional_properties;
                }
                continue;
            }
            calls.insert(call_id.clone(), web_output.len());
        }
        web_output.push(content);
    }
    contents = adjacent(web_output, 2);
    contents = adjacent(contents, 3);
    adjacent(contents, 4)
}

fn adjacent(contents: Vec<Content>, pass: u8) -> Vec<Content> {
    let mut result: Vec<Content> = Vec::new();
    for mut content in contents {
        if content.header.annotations.is_empty() {
            // Even singleton code payloads coalesce recursively.
            match (&mut content.body, pass) {
                (ContentBody::CodeCall { inputs, .. }, 3) => {
                    *inputs = coalesce(std::mem::take(inputs))
                }
                (ContentBody::CodeResult { outputs, .. }, 4) => {
                    *outputs = coalesce(std::mem::take(outputs))
                }
                _ => {}
            }
        }
        let merged = result.last_mut().is_some_and(|last| {
            if !last.header.annotations.is_empty() || !content.header.annotations.is_empty() {
                return false;
            }
            let merged = match (&mut last.body, &content.body, pass) {
                (ContentBody::Text { text: a }, ContentBody::Text { text: b }, 0) => {
                    a.push_str(b);
                    true
                }
                (
                    ContentBody::Reasoning {
                        text: a,
                        protected_data: p,
                    },
                    ContentBody::Reasoning {
                        text: b,
                        protected_data: q,
                    },
                    1,
                ) if p.is_empty() => {
                    a.push_str(b);
                    *p = q.clone();
                    true
                }
                (ContentBody::Data { uri: a }, ContentBody::Data { uri: b }, 2)
                    if last.extra.get("Name") == content.extra.get("Name") =>
                {
                    match (
                        decode_data_uri(a, usize::MAX),
                        decode_data_uri(b, usize::MAX),
                    ) {
                        (Ok((mut bytes, ma)), Ok((more, mb)))
                            if ma == mb && ma.to_ascii_lowercase().starts_with("text/") =>
                        {
                            bytes.extend(more);
                            if let ContentBody::Data { uri } = Content::data(&bytes, &ma)
                                .expect("valid existing MIME")
                                .body
                            {
                                *a = uri;
                            }
                            last.header = ContentHeader::default();
                            true
                        }
                        _ => false,
                    }
                }
                (
                    ContentBody::CodeCall {
                        call_id: a,
                        inputs: x,
                    },
                    ContentBody::CodeCall {
                        call_id: b,
                        inputs: y,
                    },
                    3,
                ) if a == b => {
                    x.extend(y.iter().cloned());
                    *x = coalesce(std::mem::take(x));
                    true
                }
                (
                    ContentBody::CodeResult {
                        call_id: a,
                        outputs: x,
                    },
                    ContentBody::CodeResult {
                        call_id: b,
                        outputs: y,
                    },
                    4,
                ) if a == b => {
                    x.extend(y.iter().cloned());
                    *x = coalesce(std::mem::take(x));
                    true
                }
                _ => false,
            };
            if merged {
                last.header.raw = Default::default();
            }
            merged
        });
        if !merged {
            result.push(content);
        }
    }
    result
}
