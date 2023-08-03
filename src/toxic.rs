//! Represents a [Toxic] - an effect on the network connection.
//!
//! [Toxic]: https://github.com/Shopify/toxiproxy#toxics

use serde::{Deserialize, Serialize};
use std::collections::HashMap;

pub type ToxicValueType = u32;

pub const TOXIC_CONDITION_MATCHER_TYPE: &str = "httpRequestHeaderMatcher";

/// Config of a Toxic.
#[derive(Serialize, Deserialize, Debug)]
pub struct ToxicPack {
    pub name: String,
    pub r#type: String,
    pub stream: String,
    pub toxicity: f32,
    pub attributes: HashMap<String, ToxicValueType>,
    pub condition: Option<ToxicCondition>,
}

impl ToxicPack {
    pub(crate) fn new(
        r#type: String,
        stream: String,
        toxicity: f32,
        attributes: HashMap<String, ToxicValueType>,
    ) -> Self {
        Self::new_with_condition(r#type, stream, toxicity, attributes, None)
    }

    pub(crate) fn new_with_condition(
        r#type: String,
        stream: String,
        toxicity: f32,
        attributes: HashMap<String, ToxicValueType>,
        condition: Option<ToxicCondition>,
    ) -> Self {
        let name = format!("{}_{}", r#type, stream);
        Self {
            name,
            r#type,
            stream,
            toxicity,
            attributes,
            condition,
        }
    }
}

// Config of a ToxicCondition.
#[derive(Serialize, Deserialize, Debug)]
#[serde(rename_all = "camelCase")]
pub struct ToxicCondition {
    matcher_type: String,
    matcher_parameters: HashMap<String, String>,
}

impl ToxicCondition {
    pub fn new_http_request_header_matcher(header_key: String, header_value_regex: String) -> Self {
        let mut matcher_parameters = HashMap::new();
        matcher_parameters.insert("headerKey".into(), header_key);
        matcher_parameters.insert("headerValueRegex".into(), header_value_regex);

        Self {
            matcher_type: TOXIC_CONDITION_MATCHER_TYPE.into(),
            matcher_parameters,
        }
    }
}
