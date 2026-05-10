use pebble_core::{PebbleError, Result};

use crate::types::{BilingualSegment, TranslateResult};

pub async fn translate(
    client: &reqwest::Client,
    endpoint: &str,
    text: &str,
    from: &str,
    to: &str,
) -> Result<TranslateResult> {
    let body = serde_json::json!({
        "text": text,
        "source_lang": from.to_uppercase(),
        "target_lang": to.to_uppercase(),
    });

    let resp = client
        .post(endpoint)
        .json(&body)
        .send()
        .await
        .map_err(|e| PebbleError::Translate(format!("DeepLX request failed: {e}")))?;

    let status = resp.status();
    let json: serde_json::Value = resp
        .json()
        .await
        .map_err(|e| PebbleError::Translate(format!("DeepLX response parse failed: {e}")))?;

    if !status.is_success() {
        return Err(PebbleError::Translate(format!(
            "DeepLX error {status}: {json}"
        )));
    }

    let translated = json
        .get("data")
        .and_then(|d| d.as_str())
        .unwrap_or("")
        .to_string();

    Ok(TranslateResult {
        segments: build_segments(text, &translated),
        translated,
    })
}

pub fn build_segments(source: &str, target: &str) -> Vec<BilingualSegment> {
    source
        .split('\n')
        .zip(target.split('\n'))
        .filter(|(s, _)| !s.trim().is_empty())
        .map(|(s, t)| BilingualSegment {
            source: s.to_string(),
            target: t.to_string(),
        })
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_build_segments() {
        let segments = build_segments("Hello\nWorld\n\nFoo", "你好\n世界\n\nFoo翻译");
        assert_eq!(segments.len(), 3);
        assert_eq!(segments[0].source, "Hello");
        assert_eq!(segments[0].target, "你好");
        assert_eq!(segments[1].source, "World");
        assert_eq!(segments[1].target, "世界");
    }

    #[test]
    fn test_build_segments_uneven() {
        let segments = build_segments("Line1\nLine2\nLine3", "译1\n译2");
        // zip stops at shorter
        assert_eq!(segments.len(), 2);
    }
}
