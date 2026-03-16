use anyhow::{Context, Result, anyhow};
use reqwest::header::{AUTHORIZATION, CONTENT_TYPE, HeaderMap, HeaderValue};
use reqwest::{Client, Response};
use serde::Serialize;
use serde_json::Value;
use std::env;

const OPENAI_RESPONSES_URL: &str = "https://api.openai.com/v1/responses";

#[derive(Debug, Clone)]
pub struct OpenAiWrapper {
    client: Client,
    api_key: String,
    endpoint: String,
    extra_headers: HeaderMap,
}

impl OpenAiWrapper {
    pub fn from_env() -> Result<Self> {
        let api_key = env::var("OPENAI_API_KEY")
            .context("Missing OPENAI_API_KEY in environment")?
            .trim()
            .to_owned();

        if api_key.is_empty() {
            return Err(anyhow!("AI API key cannot be empty"));
        }

        Ok(Self {
            client: Client::new(),
            api_key,
            endpoint: OPENAI_RESPONSES_URL.to_owned(),
            extra_headers: HeaderMap::new(),
        })
    }

    pub async fn responses<T>(&self, payload: &T) -> Result<Value>
    where
        T: Serialize + ?Sized,
    {
        let response = self.post_json(&self.endpoint, payload).await?;
        Self::parse_json_response(response).await
    }

    async fn post_json<T>(&self, url: &str, payload: &T) -> Result<Response>
    where
        T: Serialize + ?Sized,
    {
        let mut headers = HeaderMap::new();
        headers.insert(CONTENT_TYPE, HeaderValue::from_static("application/json"));

        let authorization = format!("Bearer {}", self.api_key);
        headers.insert(
            AUTHORIZATION,
            HeaderValue::from_str(&authorization).context("Invalid Authorization header")?,
        );

        headers.extend(self.extra_headers.clone());

        let response = self
            .client
            .post(url)
            .headers(headers)
            .json(payload)
            .send()
            .await
            .with_context(|| format!("Failed to call {url}"))?;

        Ok(response)
    }

    async fn parse_json_response(response: Response) -> Result<Value> {
        let status = response.status();
        let text = response.text().await.context("Failed to read response body")?;
        let value = serde_json::from_str::<Value>(&text)
            .with_context(|| format!("Response was not valid JSON: {text}"))?;

        if !status.is_success() {
            let message = value
                .get("error")
                .and_then(|error| error.get("message"))
                .and_then(Value::as_str)
                .unwrap_or("Unknown API error");
            return Err(anyhow!("Request failed with status {status}: {message}"));
        }

        if let Some(message) = value
            .get("error")
            .and_then(|error| error.get("message"))
            .and_then(Value::as_str)
        {
            return Err(anyhow!(message.to_owned()));
        }

        Ok(value)
    }
}

pub fn resolve_model_for_provider(model: &str) -> Result<String> {
    if model.trim().is_empty() {
        return Err(anyhow!("Model must be a non-empty string"));
    }

    Ok(model.to_owned())
}

pub fn extract_response_text(response: &Value) -> Option<String> {
    if let Some(text) = response.get("output_text").and_then(Value::as_str) {
        let trimmed = text.trim();
        if !trimmed.is_empty() {
            return Some(trimmed.to_owned());
        }
    }

    let output = response.get("output")?.as_array()?;
    for item in output {
        if item.get("type").and_then(Value::as_str) != Some("message") {
            continue;
        }

        let contents = item.get("content")?.as_array()?;
        for content in contents {
            if let Some(text) = content.get("text").and_then(Value::as_str) {
                let trimmed = text.trim();
                if !trimmed.is_empty() {
                    return Some(trimmed.to_owned());
                }
            }
        }
    }

    None
}
