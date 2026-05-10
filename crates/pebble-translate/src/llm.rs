use pebble_core::{PebbleError, Result};

use crate::deeplx::build_segments;
use crate::types::{LLMMode, TranslateResult};

pub async fn translate(
    client: &reqwest::Client,
    endpoint: &str,
    api_key: &str,
    model: &str,
    mode: &LLMMode,
    text: &str,
    from: &str,
    to: &str,
) -> Result<TranslateResult> {
    let system_prompt = format!(
        "You are a professional translator. Translate the following text from {from} to {to}. \
         Output ONLY the translation, nothing else. Preserve formatting and line breaks."
    );

    let url = match mode {
        LLMMode::Completions => {
            format!("{}/v1/chat/completions", endpoint.trim_end_matches('/'))
        }
        LLMMode::Responses => format!("{}/v1/responses", endpoint.trim_end_matches('/')),
    };

    let body = match mode {
        LLMMode::Completions => serde_json::json!({
            "model": model,
            "messages": [
                { "role": "system", "content": system_prompt },
                { "role": "user", "content": text }
            ],
            "temperature": 0.3,
        }),
        LLMMode::Responses => serde_json::json!({
            "model": model,
            "input": format!("{system_prompt}\n\n{text}"),
        }),
    };

    let resp = client
        .post(&url)
        .header("Authorization", format!("Bearer {api_key}"))
        .json(&body)
        .send()
        .await
        .map_err(|e| PebbleError::Translate(format!("LLM request failed: {e}")))?;

    if !resp.status().is_success() {
        let status = resp.status();
        let body_text = resp.text().await.unwrap_or_default();
        return Err(PebbleError::Translate(format!(
            "LLM error {status}: {body_text}"
        )));
    }

    let json: serde_json::Value = resp
        .json()
        .await
        .map_err(|e| PebbleError::Translate(format!("LLM parse failed: {e}")))?;

    let translated = match mode {
        LLMMode::Completions => json
            .get("choices")
            .and_then(|c| c.get(0))
            .and_then(|c| c.get("message"))
            .and_then(|m| m.get("content"))
            .and_then(|c| c.as_str())
            .unwrap_or("")
            .to_string(),
        LLMMode::Responses => json
            .get("output")
            .and_then(|o| o.get(0))
            .and_then(|o| o.get("content"))
            .and_then(|c| c.get(0))
            .and_then(|c| c.get("text"))
            .and_then(|t| t.as_str())
            .unwrap_or("")
            .to_string(),
    };

    Ok(TranslateResult {
        segments: build_segments(text, &translated),
        translated,
    })
}
