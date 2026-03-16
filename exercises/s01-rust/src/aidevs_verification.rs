use anyhow::{Context, Result, anyhow};
use reqwest::Client;
use serde::Serialize;
use serde_json::Value;
use std::env;

const VERIFY_URL: &str = "https://hub.ag3nts.org/verify";

#[derive(Debug, Clone)]
pub struct AiDevsVerification {
    client: Client,
    api_key: String,
}

impl AiDevsVerification {
    pub fn from_env() -> Result<Self> {
        let api_key = env::var("DEVS_KEY")
            .context("Missing DEVS_KEY in environment")?
            .trim()
            .to_owned();

        if api_key.is_empty() {
            return Err(anyhow!("DEVS_KEY cannot be empty"));
        }

        Ok(Self {
            client: Client::new(),
            api_key,
        })
    }

    pub async fn verify<T>(&self, task: &str, answer: &T) -> Result<Value>
    where
        T: Serialize + ?Sized,
    {
        if task.trim().is_empty() {
            return Err(anyhow!("Task cannot be empty"));
        }

        #[derive(Serialize)]
        struct VerifyPayload<'a, TAnswer>
        where
            TAnswer: Serialize + ?Sized,
        {
            apikey: &'a str,
            task: &'a str,
            answer: &'a TAnswer,
        }

        let payload = VerifyPayload {
            apikey: &self.api_key,
            task,
            answer,
        };

        let response = self
            .client
            .post(VERIFY_URL)
            .json(&payload)
            .send()
            .await
            .context("Failed to call verification endpoint")?;

        let status = response.status();
        let body = response
            .text()
            .await
            .context("Failed to read verification response body")?;

        if !status.is_success() {
            return Err(anyhow!(
                "Verify request failed ({status}): {body}"
            ));
        }

        match serde_json::from_str::<Value>(&body) {
            Ok(value) => Ok(value),
            Err(_) => Ok(Value::String(body)),
        }
    }
}
