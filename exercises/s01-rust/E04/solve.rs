use anyhow::{Context, Result};
use reqwest::Client;
use serde_json::{Value, json};
use std::collections::HashSet;
use std::path::PathBuf;
use tokio::fs;

use exercises::aidevs_verification::AiDevsVerification;
use exercises::llm_payload_service::{JsonSchemaFormat, LlmPayloadService};
use exercises::openai_wrapper::{OpenAiWrapper, extract_response_text};

const DOC_BASE: &str = "https://hub.ag3nts.org/dane/doc/";
const MODEL: &str = "gpt-5.4";

fn docs_dir() -> PathBuf {
    PathBuf::from("E04/docs")
}

#[tokio::main]
async fn main() -> Result<()> {
    exercises::env::load_shared_env().ok();

    tracing_subscriber::fmt()
        .with_env_filter(
            tracing_subscriber::EnvFilter::try_from_default_env().unwrap_or_else(|_| "info".into()),
        )
        .init();
    let http = Client::new();
    let openai = OpenAiWrapper::from_env()?;
    let verifier = AiDevsVerification::from_env()?;

    fs::create_dir_all(docs_dir()).await?;

    println!("📚 Phase 1 — crawling documentation from {}\n", DOC_BASE);
    let docs = crawl_docs(&http, &openai).await?;
    println!("\n🔍 Phase 1b — hunting for secrets in HTTP headers…\n");
    inspect_headers(&http).await?;

    async fn inspect_headers(http: &Client) -> Result<()> {
        let files: &[&str] = &[
            "index.md",
            "zalacznik-A.md",
            "zalacznik-B.md",
            "zalacznik-C.md",
            "zalacznik-D.md",
            "zalacznik-E.md",
            "zalacznik-F.md",
            "zalacznik-G.md",
            "zalacznik-H.md",
            "zalacznik-I.md",
            "trasy-wylaczone.png",
            "dodatkowe-wagony.md",
        ];
        
        for file in files {
            let url = format!("{}{}", DOC_BASE, file);
        
            for method in ["HEAD", "GET"] {
                let resp = match method {
                    "HEAD" => http.head(&url).send().await,
                    _ => http.get(&url).send().await,
                };
        
                if let Ok(resp) = resp {
                    for (name, value) in resp.headers().iter() {
                        if let Ok(val) = value.to_str() {
                            if val.contains("{FLG:") || val.contains("FLG:") {
                                println!("🚨 FOUND FLAG in {} {} → header '{}': {}", method, file, name, val);
                            }
                        }
                    }
                }
            }
        }
        
        Ok(())
        }

    let all_docs_text = docs
        .iter()
        .map(|(name, content)| format!("### FILE: {}\n{}", name, content))
        .collect::<Vec<_>>()
        .join("\n\n---\n\n");

    println!("\n============================================================");
    println!("📄 Collected {} document(s).\n", docs.len());

    println!("📝 Phase 2 — constructing declaration…\n");
    let mut declaration = build_declaration(&openai, &all_docs_text).await?;
    println!("Generated declaration:\n---\n{}\n---\n", declaration);

    for attempt in 1..=8 {
        println!("📤 Attempt {} — submitting…", attempt);

        let answer = json!({ "declaration": &declaration });
        let result = verifier.verify("sendit", &answer).await;

        match result {
            Ok(resp) => {
                let resp_str = resp.to_string();
                println!("Response: {}\n", resp_str);

                if resp_str.contains("FLG:") || resp.get("code") == Some(&json!(0)) {
                    println!("🎉 Success!");
                    return Ok(());
                }

                if attempt < 8 {
                    let error_msg = resp
                        .get("message")
                        .and_then(Value::as_str)
                        .unwrap_or(&resp_str);
                    println!("🔧 Fixing based on: {}\n", error_msg);
                    declaration =
                        fix_declaration(&openai, &all_docs_text, &declaration, error_msg).await?;
                    println!("Fixed declaration:\n---\n{}\n---\n", declaration);
                }
            }
            Err(e) => {
                let error_msg = format!("{:#}", e);
                println!("Error: {}\n", error_msg);

                if error_msg.contains("FLG:") {
                    println!("🎉 Flag found in error response!");
                    return Ok(());
                }

                if attempt < 8 {
                    println!("🔧 Fixing based on error…\n");
                    declaration =
                        fix_declaration(&openai, &all_docs_text, &declaration, &error_msg).await?;
                    println!("Fixed declaration:\n---\n{}\n---\n", declaration);
                }
            }
        }
    }

    println!("❌ Failed after 8 attempts.");
    Ok(())
}

async fn crawl_docs(http: &Client, openai: &OpenAiWrapper) -> Result<Vec<(String, String)>> {
    let mut visited = HashSet::new();
    let mut queue = vec!["index.md".to_string()];
    let mut result = Vec::new();

    while let Some(path) = queue.pop() {
        let clean = normalize_path(&path);
        if clean.is_empty() || visited.contains(&clean) {
            continue;
        }
        visited.insert(clean.clone());

        let local_path = docs_dir().join(&clean);
        let local_text_path = if has_image_ext(&clean) {
            docs_dir().join(format!("{}.vision.txt", clean))
        } else {
            local_path.clone()
        };

        if local_text_path.exists() {
            println!("  📂 Cached: {}", clean);
            let cached = fs::read_to_string(&local_text_path).await?;

            if !has_image_ext(&clean) {
                for link in extract_all_links(&cached) {
                    let resolved = resolve_link(&clean, &link);
                    if !visited.contains(&resolved) {
                        println!("    🔗 Link: {} → {}", link, resolved);
                        queue.push(resolved);
                    }
                }
            }

            result.push((clean, cached));
            continue;
        }

        let url = format!("{}{}", DOC_BASE, clean);
        println!("  📥 Downloading: {}", url);

        let resp = match http.get(&url).send().await {
            Ok(r) if r.status().is_success() => r,
            Ok(r) => {
                eprintln!("    ⚠️  HTTP {} — skipping", r.status());
                continue;
            }
            Err(e) => {
                eprintln!("    ⚠️  Network error: {} — skipping", e);
                continue;
            }
        };

        let ct = resp
            .headers()
            .get("content-type")
            .and_then(|v| v.to_str().ok())
            .unwrap_or("")
            .to_lowercase();

        let is_image = ct.contains("image") || has_image_ext(&clean);

        if is_image {
            let bytes = resp.bytes().await?;
            if let Some(parent) = local_path.parent() {
                fs::create_dir_all(parent).await?;
            }
            fs::write(&local_path, &bytes).await?;
            println!(
                "    💾 Saved image: {} ({} bytes)",
                local_path.display(),
                bytes.len()
            );

            println!("    🖼️  Analyzing with Vision…");
            let image_url = format!("{}{}", DOC_BASE, clean);
            let description = match analyse_image(openai, &image_url).await {
                Ok(desc) => desc,
                Err(e) => {
                    eprintln!("    ⚠️  URL vision failed: {:#}", e);
                    println!("    🔄 Retrying with base64…");
                    let b64 = base64_encode(&bytes);
                    let mime = if clean.ends_with(".png") {
                        "image/png"
                    } else {
                        "image/jpeg"
                    };
                    analyse_image_b64(openai, &b64, mime).await?
                }
            };

            let vision_text = format!("[IMAGE ANALYSIS: {}]\n{}", clean, description);
            fs::write(&local_text_path, &vision_text).await?;
            println!("    💾 Saved vision text: {}", local_text_path.display());

            result.push((clean, vision_text));
        } else {
            let text = resp.text().await.unwrap_or_default();

            if let Some(parent) = local_path.parent() {
                fs::create_dir_all(parent).await?;
            }
            fs::write(&local_path, &text).await?;
            println!(
                "    💾 Saved: {} ({} chars)",
                local_path.display(),
                text.len()
            );

            let links = extract_all_links(&text);
            for link in &links {
                let resolved = resolve_link(&clean, link);
                if !visited.contains(&resolved) {
                    println!("    🔗 Link: {} → {}", link, resolved);
                    queue.push(resolved);
                }
            }

            result.push((clean, text));
        }
    }

    Ok(result)
}

fn extract_all_links(text: &str) -> Vec<String> {
    let mut links = Vec::new();

    let bytes = text.as_bytes();
    let mut i = 0;
    while i < bytes.len() {
        if i + 1 < bytes.len() && bytes[i] == b']' && bytes[i + 1] == b'(' {
            let start = i + 2;
            let mut end = start;
            let mut depth = 1;
            while end < bytes.len() && depth > 0 {
                match bytes[end] {
                    b'(' => depth += 1,
                    b')' => depth -= 1,
                    _ => {}
                }
                if depth > 0 {
                    end += 1;
                }
            }
            if end > start {
                let raw = String::from_utf8_lossy(&bytes[start..end])
                    .trim()
                    .to_string();
                if !raw.starts_with("http")
                    && !raw.starts_with('#')
                    && !raw.starts_with("mailto:")
                    && !raw.is_empty()
                {
                    let without_fragment = raw.split('#').next().unwrap_or(&raw).to_string();
                    if !without_fragment.is_empty() {
                        links.push(without_fragment);
                    }
                }
            }
            i = end + 1;
        } else {
            i += 1;
        }
    }

    let include_prefix = "[include file=\"";
    let mut search_start = 0;
    while let Some(pos) = text[search_start..].find(include_prefix) {
        let abs_pos = search_start + pos + include_prefix.len();
        if let Some(end_quote) = text[abs_pos..].find('"') {
            let filename = &text[abs_pos..abs_pos + end_quote];
            let filename = filename.trim().to_string();
            if !filename.is_empty() {
                links.push(filename);
            }
            search_start = abs_pos + end_quote + 1;
        } else {
            break;
        }
    }

    links
}

fn normalize_path(path: &str) -> String {
    path.trim()
        .replace("./", "")
        .trim_start_matches('/')
        .to_string()
}

fn resolve_link(current_file: &str, link: &str) -> String {
    let link = normalize_path(link);
    if let Some(pos) = current_file.rfind('/') {
        let dir = &current_file[..pos];
        normalize_path(&format!("{}/{}", dir, link))
    } else {
        link
    }
}

fn has_image_ext(path: &str) -> bool {
    let lower = path.to_lowercase();
    lower.ends_with(".png")
        || lower.ends_with(".jpg")
        || lower.ends_with(".jpeg")
        || lower.ends_with(".gif")
        || lower.ends_with(".webp")
        || lower.ends_with(".bmp")
}

async fn analyse_image(openai: &OpenAiWrapper, image_url: &str) -> Result<String> {
    let payload = json!({
        "model": MODEL,
        "input": [
            {
                "role": "system",
                "content": [{
                    "type": "input_text",
                    "text": "Wyodrębnij CAŁY tekst, tabele, kody, numery tras i dane z tego obrazu. \
                             Bądź maksymalnie precyzyjny. Odtwórz tabele dokładnie w formacie tekstowym. \
                             Nie pomijaj żadnych detali — każda linia, każdy kod trasy, każda wartość."
                }]
            },
            {
                "role": "user",
                "content": [
                    { "type": "input_image", "image_url": image_url },
                    {
                        "type": "input_text",
                        "text": "Wyodrębnij cały tekst i dane z tego obrazu dokumentacji SPK. Dokładnie odtwórz tabele."
                    }
                ]
            }
        ]
    });

    let response = openai.responses(&payload).await?;
    extract_response_text(&response).context("No text output from Vision")
}

async fn analyse_image_b64(openai: &OpenAiWrapper, b64: &str, mime: &str) -> Result<String> {
    let data_url = format!("data:{};base64,{}", mime, b64);
    let payload = json!({
        "model": MODEL,
        "input": [
            {
                "role": "system",
                "content": [{
                    "type": "input_text",
                    "text": "Wyodrębnij CAŁY tekst, tabele, kody, numery tras i dane z tego obrazu. \
                             Bądź maksymalnie precyzyjny. Odtwórz tabele dokładnie."
                }]
            },
            {
                "role": "user",
                "content": [
                    { "type": "input_image", "image_url": data_url },
                    {
                        "type": "input_text",
                        "text": "Wyodrębnij cały tekst i dane z tego obrazu dokumentacji SPK."
                    }
                ]
            }
        ]
    });

    let response = openai.responses(&payload).await?;
    extract_response_text(&response).context("No text output from Vision (b64)")
}

async fn build_declaration(openai: &OpenAiWrapper, all_docs: &str) -> Result<String> {
    let system = r#"Jesteś ekspertem od wypełniania deklaracji transportowych w Systemie Przesyłek Konduktorskich (SPK).

Na podstawie dostarczonej dokumentacji wypełnij deklarację transportową DOKŁADNIE według wzoru z Załącznika E.

Dane przesyłki:
- Identyfikator nadawcy: 450202122
- Punkt nadawczy: Gdańsk
- Punkt docelowy: Żarnowiec
- Waga: 2800 kg (2.8 tony)
- Budżet: 0 PP — przesyłka musi być darmowa lub finansowana przez System
- Zawartość: kasety z paliwem do reaktora
- Uwagi specjalne: BRAK (nie wpisuj żadnych uwag specjalnych)

Zasady:
1. Użyj DOKŁADNIE wzoru z Załącznika E — każdy separator, pole, kolejność.
2. Ustal poprawny kod trasy z Gdańska do Żarnowca na podstawie sieci połączeń (sekcja 3) i tras wyłączonych (sekcja 8).
3. Kategoria A (strategiczna) — opłata 0 PP (finansowana przez System). Kasety do reaktora to podzespoły infrastruktury krytycznej.
4. Wypełnij WSZYSTKIE pola poprawnie.
5. NIE dodawaj uwag specjalnych.
6. Zwróć tekst deklaracji w polu declaration_text. Użyj \n dla nowych linii.
7. Zachowaj format wzoru DOKŁADNIE."#;

    let user = format!(
        "Oto cała pobrana dokumentacja SPK:\n\n{}\n\nWypełnij deklarację transportową.",
        all_docs
    );

    let payload = LlmPayloadService::build_responses_payload(
        MODEL,
        system,
        &user,
        JsonSchemaFormat {
            name: "declaration".to_owned(),
            schema: json!({
                "type": "object",
                "properties": {
                    "declaration_text": { "type": "string" }
                },
                "required": ["declaration_text"],
                "additionalProperties": false
            }),
            strict: true,
        },
    );

    let response = openai.responses(&payload).await?;
    let raw = extract_response_text(&response).context("No text in declaration response")?;
    let parsed: Value = serde_json::from_str(&raw)
        .with_context(|| format!("Failed to parse declaration JSON: {}", raw))?;

    parsed["declaration_text"]
        .as_str()
        .map(|s| s.trim().to_string())
        .context("Missing declaration_text field")
}

async fn fix_declaration(
    openai: &OpenAiWrapper,
    all_docs: &str,
    current: &str,
    error: &str,
) -> Result<String> {
    let system = r#"Jesteś ekspertem od deklaracji transportowych SPK.
Poprzednia deklaracja została odrzucona. Popraw ją na podstawie komunikatu błędu i pełnej dokumentacji.

Dane przesyłki:
- Identyfikator nadawcy: 450202122
- Punkt nadawczy: Gdańsk
- Punkt docelowy: Żarnowiec
- Waga: 2800 kg
- Budżet: 0 PP
- Zawartość: kasety z paliwem do reaktora
- Uwagi specjalne: BRAK

WAŻNE: Deklaracja musi DOKŁADNIE odpowiadać wzorowi z Załącznika E — zachowaj identyczny format, separatory, nagłówki.
Zwróć TYLKO poprawiony tekst deklaracji w polu declaration_text. Użyj \n dla nowych linii."#;

    let user = format!(
        "Pełna dokumentacja SPK:\n\n{}\n\n---\nOdrzucona deklaracja:\n{}\n\n---\nKomunikat błędu:\n{}\n\nPopraw deklarację.",
        all_docs, current, error
    );

    let payload = LlmPayloadService::build_responses_payload(
        MODEL,
        system,
        &user,
        JsonSchemaFormat {
            name: "declaration_fix".to_owned(),
            schema: json!({
                "type": "object",
                "properties": {
                    "declaration_text": { "type": "string" }
                },
                "required": ["declaration_text"],
                "additionalProperties": false
            }),
            strict: true,
        },
    );

    let response = openai.responses(&payload).await?;
    let raw = extract_response_text(&response).context("No text in fix response")?;
    let parsed: Value =
        serde_json::from_str(&raw).with_context(|| format!("Failed to parse fix JSON: {}", raw))?;

    parsed["declaration_text"]
        .as_str()
        .map(|s| s.trim().to_string())
        .context("Missing declaration_text in fix response")
}

fn base64_encode(data: &[u8]) -> String {
    const ALPHA: &[u8] = b"ABCDEFGHIJKLMNOPQRSTUVWXYZabcdefghijklmnopqrstuvwxyz0123456789+/";
    let mut out = String::with_capacity((data.len() + 2) / 3 * 4);

    for chunk in data.chunks(3) {
        let b0 = chunk[0] as u32;
        let b1 = chunk.get(1).copied().unwrap_or(0) as u32;
        let b2 = chunk.get(2).copied().unwrap_or(0) as u32;
        let triple = (b0 << 16) | (b1 << 8) | b2;

        out.push(ALPHA[((triple >> 18) & 0x3F) as usize] as char);
        out.push(ALPHA[((triple >> 12) & 0x3F) as usize] as char);
        if chunk.len() > 1 {
            out.push(ALPHA[((triple >> 6) & 0x3F) as usize] as char);
        } else {
            out.push('=');
        }
        if chunk.len() > 2 {
            out.push(ALPHA[(triple & 0x3F) as usize] as char);
        } else {
            out.push('=');
        }
    }

    out
}
