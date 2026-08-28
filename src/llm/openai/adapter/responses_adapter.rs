use crate::llm::common::http;
use crate::llm::openai::model::responses_api::{ResponsesRequest, ResponsesResponse};
use anyhow::{anyhow, Result};
use reqwest::Client;

/// Adapter for the OpenAI Responses API
pub struct ResponsesAdapter;

impl ResponsesAdapter {
    /// Make a request to the Responses API
    pub async fn chat(request: &ResponsesRequest, api_key: &str) -> Result<ResponsesResponse> {
        let client = http::client()?;

        // Retry transport failures and provider back-pressure rather than
        // ending the turn on the first hiccup.
        let mut response_text = String::new();
        for attempt in 1..=http::MAX_ATTEMPTS {
            let response = match client
                .post("https://api.openai.com/v1/responses")
                .header("Content-Type", "application/json")
                .bearer_auth(api_key)
                .json(&request)
                .send()
                .await
            {
                Ok(response) => response,
                Err(error) => {
                    if attempt < http::MAX_ATTEMPTS && http::is_transient(&error) {
                        http::backoff("OpenAI request", attempt, &error.to_string()).await;
                        continue;
                    }
                    return Err(error.into());
                }
            };

            let status = response.status();
            if !status.is_success() {
                let error_text = response.text().await.unwrap_or_default();
                if attempt < http::MAX_ATTEMPTS && http::is_transient_status(status) {
                    http::backoff("OpenAI request", attempt, &status.to_string()).await;
                    continue;
                }
                return Err(anyhow!(
                    "OpenAI API request failed with status {}: {}",
                    status,
                    error_text
                ));
            }

            match response.text().await {
                Ok(body) => {
                    response_text = body;
                    break;
                }
                Err(error) => {
                    if attempt < http::MAX_ATTEMPTS {
                        http::backoff("OpenAI response", attempt, &error.to_string()).await;
                        continue;
                    }
                    return Err(error.into());
                }
            }
        }

        if response_text.is_empty() {
            return Err(anyhow!(
                "OpenAI API did not respond after {} attempts",
                http::MAX_ATTEMPTS
            ));
        }

        // Try to parse the JSON response
        match serde_json::from_str::<ResponsesResponse>(&response_text) {
            Ok(parsed_response) => Ok(parsed_response),
            Err(e) => {
                eprintln!("Failed to parse OpenAI response: {}", e);
                eprintln!("Response body: {}", response_text);
                Err(anyhow!("Error decoding response body: {}", e))
            }
        }
    }

    /// Make a streaming request to the Responses API
    #[allow(dead_code)]
    pub async fn chat_stream(
        request: &ResponsesRequest,
        api_key: &str,
    ) -> Result<reqwest::Response> {
        let mut streaming_request = request.clone();
        streaming_request.stream = Some(true);

        let client = Client::new();
        let response = client
            .post("https://api.openai.com/v1/responses")
            .header("Content-Type", "application/json")
            .bearer_auth(api_key)
            .json(&streaming_request)
            .send()
            .await?;

        Ok(response)
    }
}
