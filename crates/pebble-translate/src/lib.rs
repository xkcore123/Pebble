pub mod deepl;
pub mod deeplx;
pub mod generic;
pub mod llm;
pub mod types;

use pebble_core::{HttpProxyConfig, PebbleError, Result};
use types::{TranslateProviderConfig, TranslateResult};

pub struct TranslateService;

impl TranslateService {
    pub fn http_client_with_proxy(proxy: Option<&HttpProxyConfig>) -> Result<reqwest::Client> {
        let mut builder = reqwest::ClientBuilder::new();
        if let Some(proxy) = proxy {
            let uri = proxy.socks5h_uri().map_err(PebbleError::Network)?;
            let reqwest_proxy = reqwest::Proxy::all(&uri)
                .map_err(|e| PebbleError::Network(format!("Invalid proxy: {e}")))?;
            builder = builder.proxy(reqwest_proxy);
        }
        builder
            .build()
            .map_err(|e| PebbleError::Network(format!("Failed to build HTTP client: {e}")))
    }

    pub async fn translate(
        config: &TranslateProviderConfig,
        text: &str,
        from: &str,
        to: &str,
    ) -> Result<TranslateResult> {
        Self::translate_with_proxy(config, None, text, from, to).await
    }

    pub async fn translate_with_proxy(
        config: &TranslateProviderConfig,
        proxy: Option<&HttpProxyConfig>,
        text: &str,
        from: &str,
        to: &str,
    ) -> Result<TranslateResult> {
        let client = Self::http_client_with_proxy(proxy)?;

        match config {
            TranslateProviderConfig::DeepLX { endpoint } => {
                deeplx::translate(&client, endpoint, text, from, to).await
            }
            TranslateProviderConfig::DeepL {
                api_key,
                use_free_api,
            } => deepl::translate(&client, api_key, *use_free_api, text, from, to).await,
            TranslateProviderConfig::GenericApi {
                endpoint,
                api_key,
                source_lang_param,
                target_lang_param,
                text_param,
                result_path,
            } => {
                generic::translate(
                    &client,
                    endpoint,
                    api_key.as_deref(),
                    source_lang_param,
                    target_lang_param,
                    text_param,
                    result_path,
                    text,
                    from,
                    to,
                )
                .await
            }
            TranslateProviderConfig::LLM {
                endpoint,
                api_key,
                model,
                mode,
            } => llm::translate(&client, endpoint, api_key, model, mode, text, from, to).await,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use pebble_core::HttpProxyConfig;

    #[test]
    fn translate_service_accepts_socks5_proxy_config() {
        let proxy = HttpProxyConfig {
            host: "127.0.0.1".to_string(),
            port: 7890,
        };

        let client = TranslateService::http_client_with_proxy(Some(&proxy));

        assert!(client.is_ok());
    }

    #[test]
    fn translate_service_rejects_invalid_proxy_config() {
        let proxy = HttpProxyConfig {
            host: " ".to_string(),
            port: 7890,
        };

        let err = TranslateService::http_client_with_proxy(Some(&proxy)).unwrap_err();

        assert!(err.to_string().contains("Proxy host"));
    }

    #[tokio::test]
    async fn translate_service_validates_proxy_before_translation_request() {
        let config = TranslateProviderConfig::DeepLX {
            endpoint: "http://localhost:1188/translate".to_string(),
        };
        let proxy = HttpProxyConfig {
            host: " ".to_string(),
            port: 7890,
        };

        let err =
            TranslateService::translate_with_proxy(&config, Some(&proxy), "Hello", "en", "zh")
                .await
                .unwrap_err();

        assert!(err.to_string().contains("Proxy host"));
    }
}
