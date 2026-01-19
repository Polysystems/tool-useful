//! Schema generation and provider-specific formats.

use serde::{Deserialize, Serialize};
use serde_json::{json, Value};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ParameterSchema {
    pub name: String,
    pub description: Option<String>,
    #[serde(rename = "type")]
    pub type_name: String,
    #[serde(default)]
    pub required: bool,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub default: Option<Value>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub enum_values: Option<Vec<Value>>,
}

impl ParameterSchema {
    pub fn new(name: impl Into<String>, type_name: impl Into<String>) -> Self {
        Self {
            name: name.into(),
            description: None,
            type_name: type_name.into(),
            required: false,
            default: None,
            enum_values: None,
        }
    }

    pub fn with_description(mut self, description: impl Into<String>) -> Self {
        self.description = Some(description.into());
        self
    }

    pub fn required(mut self) -> Self {
        self.required = true;
        self
    }

    pub fn with_default(mut self, default: Value) -> Self {
        self.default = Some(default);
        self
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ToolSchema {
    pub name: String,
    pub description: String,
    pub parameters: Vec<ParameterSchema>,
}

impl ToolSchema {
    pub fn new(name: impl Into<String>, description: impl Into<String>) -> Self {
        Self {
            name: name.into(),
            description: description.into(),
            parameters: Vec::new(),
        }
    }

    pub fn with_parameter(mut self, param: ParameterSchema) -> Self {
        self.parameters.push(param);
        self
    }

    pub fn with_parameters<I>(mut self, params: I) -> Self
    where
        I: IntoIterator<Item = ParameterSchema>,
    {
        self.parameters.extend(params);
        self
    }
}

/// Provider-specific schema formats
pub trait ProviderSchema {
    fn to_openai_schema(&self) -> Value;
    fn to_anthropic_schema(&self) -> Value;
    fn to_gemini_schema(&self) -> Value;
    fn to_json_schema(&self) -> Value;
}

impl ProviderSchema for ToolSchema {
    fn to_openai_schema(&self) -> Value {
        let mut properties = serde_json::Map::new();
        let mut required = Vec::new();

        for param in &self.parameters {
            let mut param_schema = serde_json::Map::new();
            param_schema.insert("type".to_string(), json!(param.type_name));
            if let Some(desc) = &param.description {
                param_schema.insert("description".to_string(), json!(desc));
            }
            if let Some(enum_vals) = &param.enum_values {
                param_schema.insert("enum".to_string(), json!(enum_vals));
            }
            properties.insert(param.name.clone(), Value::Object(param_schema));
            if param.required {
                required.push(param.name.clone());
            }
        }

        json!({
            "type": "function",
            "function": {
                "name": self.name,
                "description": self.description,
                "parameters": {
                    "type": "object",
                    "properties": properties,
                    "required": required,
                }
            }
        })
    }

    fn to_anthropic_schema(&self) -> Value {
        let mut properties = serde_json::Map::new();
        let mut required = Vec::new();

        for param in &self.parameters {
            let mut param_schema = serde_json::Map::new();
            param_schema.insert("type".to_string(), json!(param.type_name));
            if let Some(desc) = &param.description {
                param_schema.insert("description".to_string(), json!(desc));
            }
            properties.insert(param.name.clone(), Value::Object(param_schema));
            if param.required {
                required.push(param.name.clone());
            }
        }

        json!({
            "name": self.name,
            "description": self.description,
            "input_schema": {
                "type": "object",
                "properties": properties,
                "required": required,
            }
        })
    }

    fn to_gemini_schema(&self) -> Value {
        let mut properties = serde_json::Map::new();
        let mut required = Vec::new();

        for param in &self.parameters {
            let mut param_schema = serde_json::Map::new();
            param_schema.insert("type".to_string(), json!(param.type_name));
            if let Some(desc) = &param.description {
                param_schema.insert("description".to_string(), json!(desc));
            }
            properties.insert(param.name.clone(), Value::Object(param_schema));
            if param.required {
                required.push(param.name.clone());
            }
        }

        json!({
            "name": self.name,
            "description": self.description,
            "parameters": {
                "type": "object",
                "properties": properties,
                "required": required,
            }
        })
    }

    fn to_json_schema(&self) -> Value {
        let mut properties = serde_json::Map::new();
        let mut required = Vec::new();

        for param in &self.parameters {
            let mut param_schema = serde_json::Map::new();
            param_schema.insert("type".to_string(), json!(param.type_name));
            if let Some(desc) = &param.description {
                param_schema.insert("description".to_string(), json!(desc));
            }
            if let Some(default) = &param.default {
                param_schema.insert("default".to_string(), default.clone());
            }
            properties.insert(param.name.clone(), Value::Object(param_schema));
            if param.required {
                required.push(param.name.clone());
            }
        }

        json!({
            "type": "object",
            "properties": properties,
            "required": required,
        })
    }
}
