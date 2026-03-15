use anyhow::{Context, Result};
use reqwest::Client;
use serde_json::{Value, json};
use std::env;
use std::time::Duration;
use tokio::time::sleep;

const VERIFY_URL: &str = "https://hub.ag3nts.org/verify";
const ROUTE: &str = "x-01";

#[tokio::main]
async fn main() -> Result<()> {
    dotenvy::dotenv().ok();

    tracing_subscriber::fmt()
        .with_env_filter(
            tracing_subscriber::EnvFilter::try_from_default_env().unwrap_or_else(|_| "info".into()),
        )
        .init();
    let api_key = env::var("DEVS_KEY")
        .context("Missing DEVS_KEY")?
        .trim()
        .to_string();
    let client = Client::new();

    println!("🔧 Step 1 — reconfigure route {}", ROUTE);
    let resp = send_with_retry(
        &client,
        &api_key,
        json!({
            "action": "reconfigure",
            "route": ROUTE,
        }),
    )
    .await?;
    println!("   Response: {}\n", resp);

    println!("🟢 Step 2 — setstatus {} → RTOPEN", ROUTE);
    let resp = send_with_retry(
        &client,
        &api_key,
        json!({
            "action": "setstatus",
            "route": ROUTE,
            "value": "RTOPEN",
        }),
    )
    .await?;
    println!("   Response: {}\n", resp);

    println!("💾 Step 3 — save route {}", ROUTE);
    let resp = send_with_retry(
        &client,
        &api_key,
        json!({
            "action": "save",
            "route": ROUTE,
        }),
    )
    .await?;
    println!("   Response: {}\n", resp);

    let resp_str = resp.to_string();
    if resp_str.contains("FLG:") {
        println!("🎉 Flag found!");
    } else {
        println!("🔍 Checking status…");
        let resp = send_with_retry(
            &client,
            &api_key,
            json!({
                "action": "getstatus",
                "route": ROUTE,
            }),
        )
        .await?;
        println!("   Status: {}\n", resp);

        if resp.to_string().contains("FLG:") {
            println!("🎉 Flag found!");
        }
    }
    
    println!("\nSecret hunt\n");
    hunt_secret(&client, &api_key).await?;
    Ok(())
}

async fn send_with_retry(client: &Client, api_key: &str, answer: Value) -> Result<Value> {
    let payload = json!({
        "apikey": api_key,
        "task": "railway",
        "answer": answer,
    });

    let max_retries = 30;
    for attempt in 1..=max_retries {
        let resp = client
            .post(VERIFY_URL)
            .json(&payload)
            .send()
            .await
            .context("Failed to send request")?;

        let status = resp.status();
        let headers = resp.headers().clone();

        for (name, value) in headers.iter() {
            if let Ok(val) = value.to_str() {
                if val.contains("FLG:") || val.contains("{FLG") {
                    println!("   🚨 FLAG IN HEADER '{}': {}", name, val);
                }
                if name.as_str().starts_with("x-")
                    || name.as_str().starts_with("flag")
                    || name.as_str().starts_with("secret")
                {
                    println!("   🔎 Header {}: {}", name, val);
                }
            }
        }

        let retry_after = headers
            .get("retry-after")
            .and_then(|v| v.to_str().ok())
            .and_then(|v| v.parse::<u64>().ok());

        let rate_reset = headers
            .get("x-ratelimit-reset")
            .and_then(|v| v.to_str().ok())
            .and_then(|v| v.parse::<u64>().ok());

        let body = resp.text().await.unwrap_or_default();

        if body.contains("FLG:") || body.contains("{FLG") {
            println!("   🚨 FLAG IN BODY (HTTP {}): {}", status, body);
        }

        if status == 503 || status == 429 {
            println!(
                "   ⏳ {} (attempt {}/{}) body: {}",
                status,
                attempt,
                max_retries,
                &body[..body.len().min(500)]
            );
            let wait = retry_after.or(rate_reset).unwrap_or(2).max(1);
            println!("   ⏳ Waiting {}s…", wait);
            sleep(Duration::from_secs(wait)).await;
            continue;
        }

        if let Some(wait) = retry_after.or(rate_reset) {
            if wait > 0 {
                println!("   ⏳ Rate limit: waiting {}s…", wait);
                sleep(Duration::from_secs(wait)).await;
            }
        }

        let parsed: Value = serde_json::from_str(&body).unwrap_or(Value::String(body));

        if !status.is_success() {
            println!("   ⚠️  HTTP {} — {}", status, parsed);
            sleep(Duration::from_secs(3)).await;
            continue;
        }

        return Ok(parsed);
    }

    anyhow::bail!("Max retries ({}) exceeded", max_retries);
}

async fn hunt_secret(client: &Client, api_key: &str) -> Result<()> {
    let actions = vec![
        json!({ "action": "help" }),
        json!({ "action": "getstatus", "route": "x-01" }),
        json!({ "action": "reconfigure", "route": "x-01" }),
        json!({ "action": "setstatus", "route": "x-01", "value": "RTOPEN" }),
        json!({ "action": "save", "route": "x-01" }),
    ];

    let mut action_idx = 0;

    for i in 1..=200 {
        let answer = &actions[action_idx % actions.len()];
        action_idx += 1;

        let payload = json!({
            "apikey": api_key,
            "task": "railway",
            "answer": answer,
        });

        match client.post(VERIFY_URL).json(&payload).send().await {
            Ok(resp) => {
                let status = resp.status();
                let body = resp.text().await.unwrap_or_default();

                if body.contains("FLG:") && !body.contains("COUNTRYROADS") {
                    println!("🚨 SECRET FLAG #{}: [{}] {}", i, status, body);
                    return Ok(());
                }

                if body.contains("annoy") || body.contains("Annoy") {
                    println!("😤 #{}: [{}] {}", i, status, body);
                }

                if i % 10 == 0 {
                    println!("   #{}: [{}] {}", i, status, &body[..body.len().min(120)]);
                }
            }
            Err(e) => println!("   #{}: error: {}", i, e),
        }

        sleep(Duration::from_millis(100)).await;
    }

    println!("No secret flag found in 200 attempts.");
    Ok(())
}
