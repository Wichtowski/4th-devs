use serde_json::{Value, json};

#[derive(Debug, Clone)]
pub struct JsonSchemaFormat {
    pub name: String,
    pub schema: Value,
    pub strict: bool,
}

#[derive(Debug, Clone)]
pub struct LlmPayloadService;

impl LlmPayloadService {
    pub fn build_responses_payload(
        model: &str,
        system_prompt: &str,
        user_prompt: &str,
        json_schema_format: JsonSchemaFormat,
    ) -> Value {
        json!({
            "model": model,
            "input": [
                {
                    "role": "system",
                    "content": [
                        {
                            "type": "input_text",
                            "text": system_prompt,
                        }
                    ]
                },
                {
                    "role": "user",
                    "content": [
                        {
                            "type": "input_text",
                            "text": user_prompt,
                        }
                    ]
                }
            ],
            "text": {
                "format": {
                    "type": "json_schema",
                    "name": json_schema_format.name,
                    "strict": json_schema_format.strict,
                    "schema": json_schema_format.schema,
                }
            }
        })
    }
}
