use crate::llm::claude::model::chat_completion_request::ChatCompletionRequest;
use crate::llm::claude::model::chat_completion_response::ChatCompletionResponse;
use crate::llm::common::http;
use anyhow::Result;
use reqwest::StatusCode;

pub async fn chat(
    request: &ChatCompletionRequest,
    api_key: &str,
) -> Result<(StatusCode, ChatCompletionResponse)> {
    let client = http::client()?;

    // Log request info for debugging
    let input_size: usize = request.messages.iter().map(|msg| msg.content.len()).sum();

    if input_size > 10000 {
        eprintln!(
            "Claude Request: Large input detected ({} characters)",
            input_size
        );
    }

    // A reset connection or a 503 used to end the turn on the first try and
    // take the user's prompt with it.
    for attempt in 1..=http::MAX_ATTEMPTS {
        let response = match client
            .post("https://api.anthropic.com/v1/messages")
            .header("Content-Type", "application/json")
            .header("x-api-key", api_key)
            .header("anthropic-version", "2023-06-01")
            .json(&request)
            .send()
            .await
        {
            Ok(response) => response,
            Err(error) => {
                if attempt < http::MAX_ATTEMPTS && http::is_transient(&error) {
                    http::backoff("Claude request", attempt, &error.to_string()).await;
                    continue;
                }
                return Err(error.into());
            }
        };

        let status = response.status();
        if !status.is_success() {
            let error_text = response.text().await.unwrap_or_default();
            if attempt < http::MAX_ATTEMPTS && http::is_transient_status(status) {
                http::backoff("Claude request", attempt, &status.to_string()).await;
                continue;
            }
            eprintln!("Claude API Error: {}", error_text);
            anyhow::bail!("Claude API error ({}): {}", status, error_text);
        }

        let body = match response.text().await {
            Ok(body) => body,
            Err(error) => {
                if attempt < http::MAX_ATTEMPTS {
                    http::backoff("Claude response", attempt, &error.to_string()).await;
                    continue;
                }
                return Err(error.into());
            }
        };

        let parsed_response = serde_json::from_str::<ChatCompletionResponse>(&body)
            .map_err(|e| anyhow::anyhow!("Could not decode the Claude response: {}", e))?;
        return Ok((status, parsed_response));
    }

    anyhow::bail!("Claude API did not respond after {} attempts", http::MAX_ATTEMPTS)
}
