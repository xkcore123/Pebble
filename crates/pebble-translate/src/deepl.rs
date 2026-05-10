use pebble_core::{PebbleError, Result};

use crate::deeplx::build_segments;
use crate::types::TranslateResult;

pub async fn translate(
    client: &reqwest::Client,
    api_key: &str,
    use_free_api: bool,
    text: &str,
    from: &str,
    to: &str,
) -> Result<TranslateResult> {
    let base = if use_free_api {
        "https://api-free.deepl.com/v2/translate"
    } else {
        "https://api.deepl.com/v2/translate"
    };

    let from_upper = from.to_uppercase();
    let to_upper = to.to_uppercase();

    let resp = client
        .post(base)
        .header("Authorization", format!("DeepL-Auth-Key {api_key}"))
        .form(&[
            ("text", text),
            ("source_lang", from_upper.as_str()),
            ("target_lang", to_upper.as_str()),
        ])
        .send()
        .await
        .map_err(|e| PebbleError::Translate(format!("DeepL request failed: {e}")))?;

    if !resp.status().is_success() {
        let status = resp.status();
        let body = resp.text().await.unwrap_or_default();
        return Err(PebbleError::Translate(format!(
            "DeepL error {status}: {body}"
        )));
    }

    let json: serde_json::Value = resp
        .json()
        .await
        .map_err(|e| PebbleError::Translate(format!("DeepL parse failed: {e}")))?;

    let translated = json
        .get("translations")
        .and_then(|t| t.get(0))
        .and_then(|t| t.get("text"))
        .and_then(|t| t.as_str())
        .unwrap_or("")
        .to_string();

    Ok(TranslateResult {
        segments: build_segments(text, &translated),
        translated,
    })
}
