//! Adapter for the OpenAI Codex backend API
//!
//! This adapter handles requests to the Codex API endpoint
//! (chatgpt.com/backend-api/codex/responses) using OAuth tokens.

use crate::llm::openai::model::codex_api::{
    CodexContentItem, CodexOutput, CodexRequest, CodexResponse,
};
use crate::llm::common::http;
use anyhow::{anyhow, Result};

/// The Codex API endpoint
const CODEX_API_ENDPOINT: &str = "https://chatgpt.com/backend-api/codex/responses";

/// Adapter for the OpenAI Codex API
pub struct CodexAdapter;

impl CodexAdapter {
    /// Make a streaming request to the Codex API and collect the full response
    ///
    /// Uses OAuth access token for authentication instead of API key.
    /// The Codex API requires streaming, so we parse SSE events and extract the final response.
    pub async fn chat(request: &CodexRequest, access_token: &str) -> Result<CodexResponse> {
        let client = http::client()?;

        let response = client
            .post(CODEX_API_ENDPOINT)
            .header("Content-Type", "application/json")
            .header("Accept", "text/event-stream")
            .bearer_auth(access_token)
            .json(&request)
            .send()
            .await?;

        // Check if the response was successful
        if !response.status().is_success() {
            let status = response.status();
            let error_text = response.text().await.unwrap_or_default();

            // Check for specific error cases
            if status.as_u16() == 401 {
                return Err(anyhow!(
                    "Authentication failed. Your Codex session may have expired. Run 'termai auth login codex' to re-authenticate."
                ));
            }

            return Err(anyhow!(
                "Codex API request failed with status {}: {}",
                status,
                error_text
            ));
        }

        // Read the stream incrementally. `.text()` buffers the whole body and
        // fails as a unit, so a reset near the end of a long answer used to
        // throw away every byte that had already arrived.
        let mut response = response;
        let mut body = String::new();
        let mut stream_error: Option<reqwest::Error> = None;
        loop {
            match response.chunk().await {
                Ok(Some(chunk)) => body.push_str(&String::from_utf8_lossy(&chunk)),
                Ok(None) => break,
                Err(error) => {
                    stream_error = Some(error);
                    break;
                }
            }
        }

        match Self::parse_sse_response(&body) {
            Ok(parsed) => Ok(parsed),
            Err(parse_error) => match stream_error {
                Some(error) => Err(anyhow!(
                    "The connection dropped mid-response and nothing usable had arrived: {}",
                    error
                )),
                None => Err(parse_error),
            },
        }
    }

    /// Parse SSE (Server-Sent Events) response and extract the final CodexResponse
    ///
    /// Handles multiple SSE event types:
    /// - `response.output_text.delta` - accumulates text deltas
    /// - `response.output_text.done` - final text for a segment
    /// - `response.completed` - full response object
    /// - `response.failed` - error handling
    fn parse_sse_response(sse_text: &str) -> Result<CodexResponse> {
        let mut final_response: Option<CodexResponse> = None;
        let mut accumulated_text = String::new();
        let mut output_text_done: Option<String> = None;
        let mut last_message_meta: Option<(String, String, String)> = None;

        for line in sse_text.lines() {
            // SSE format: "data: {...json...}" or "event: ..." etc.
            if let Some(data) = line.strip_prefix("data: ") {
                // Skip empty data or [DONE] marker
                if data.is_empty() || data == "[DONE]" {
                    continue;
                }

                // Try to parse as JSON
                if let Ok(event) = serde_json::from_str::<serde_json::Value>(data) {
                    let event_type = event.get("type").and_then(|t| t.as_str());

                    match event_type {
                        Some("response.output_item.added") => {
                            if let Some(item) = event.get("item") {
                                if item.get("type").and_then(|t| t.as_str()) == Some("message") {
                                    let id = item
                                        .get("id")
                                        .and_then(|v| v.as_str())
                                        .unwrap_or("msg_synthetic")
                                        .to_string();
                                    let status = item
                                        .get("status")
                                        .and_then(|v| v.as_str())
                                        .unwrap_or("completed")
                                        .to_string();
                                    let role = item
                                        .get("role")
                                        .and_then(|v| v.as_str())
                                        .unwrap_or("assistant")
                                        .to_string();
                                    last_message_meta = Some((id, status, role));
                                }
                            }
                        }

                        // Accumulate text deltas
                        Some("response.output_text.delta") => {
                            if let Some(delta) = event.get("delta").and_then(|d| d.as_str()) {
                                accumulated_text.push_str(delta);
                            }
                        }

                        // Final text for a segment (can use this or accumulated text)
                        Some("response.output_text.done") => {
                            // The accumulated text should match this, but we can verify
                            if let Some(text) = event.get("text").and_then(|t| t.as_str()) {
                                if accumulated_text.is_empty() {
                                    accumulated_text = text.to_string();
                                }
                                output_text_done = Some(text.to_string());
                            }
                        }

                        // Full response completed - this is the authoritative response
                        Some("response.completed") => {
                            if let Some(response_obj) = event.get("response") {
                                if let Ok(parsed) =
                                    serde_json::from_value::<CodexResponse>(response_obj.clone())
                                {
                                    final_response = Some(parsed);
                                }
                            }
                        }

                        // Also check for response.done (alternative event name)
                        Some("response.done") => {
                            if let Some(response_obj) = event.get("response") {
                                if let Ok(parsed) =
                                    serde_json::from_value::<CodexResponse>(response_obj.clone())
                                {
                                    final_response = Some(parsed);
                                }
                            }
                        }

                        // Handle failed responses
                        Some("response.failed") => {
                            if let Some(response_obj) = event.get("response") {
                                if let Ok(parsed) =
                                    serde_json::from_value::<CodexResponse>(response_obj.clone())
                                {
                                    return Ok(parsed); // Return immediately on failure
                                }
                            }
                            // Extract error message if available
                            if let Some(error) = event.get("error") {
                                let error_msg = error
                                    .get("message")
                                    .and_then(|m| m.as_str())
                                    .unwrap_or("Unknown error");
                                return Err(anyhow!("Codex API failed: {}", error_msg));
                            }
                        }

                        // Try parsing the event itself as a CodexResponse (some APIs return it directly)
                        _ => {
                            if event.get("id").is_some() && event.get("output").is_some() {
                                if let Ok(parsed) = serde_json::from_value::<CodexResponse>(event) {
                                    final_response = Some(parsed);
                                }
                            }
                        }
                    }
                }
            }
        }

        // A stream cut short never carries `response.completed`. If any answer
        // text arrived, keep it rather than discarding the whole turn.
        let mut response = match final_response {
            Some(response) => response,
            None => {
                let salvaged = output_text_done
                    .clone()
                    .filter(|text| !text.is_empty())
                    .or_else(|| (!accumulated_text.is_empty()).then(|| accumulated_text.clone()));
                match salvaged {
                    Some(_) => CodexResponse {
                        id: "resp_truncated".to_string(),
                        object: "response".to_string(),
                        model: String::new(),
                        status: "incomplete".to_string(),
                        error: None,
                        output: Vec::new(),
                        usage: None,
                    },
                    None => {
                        return Err(anyhow!(
                            "No valid response found in SSE stream. Raw response:\n{}",
                            sse_text.chars().take(1000).collect::<String>()
                        ))
                    }
                }
            }
        };

        if response.output.is_empty() {
            let synthesized_text = output_text_done
                .filter(|text| !text.is_empty())
                .or_else(|| (!accumulated_text.is_empty()).then(|| accumulated_text.clone()));

            if let Some(text) = synthesized_text {
                let (id, status, role) = last_message_meta.unwrap_or_else(|| {
                    (
                        "msg_synthetic".to_string(),
                        "completed".to_string(),
                        "assistant".to_string(),
                    )
                });

                response.output.push(CodexOutput::Message {
                    id,
                    status,
                    role,
                    content: vec![CodexContentItem::OutputText {
                        text,
                        annotations: vec![],
                    }],
                });
            }
        }

        Ok(response)
    }
}

#[cfg(test)]
mod tests {
    use super::CodexAdapter;

    #[test]
    fn test_parse_sse_response_recovers_text_when_completed_output_is_empty() {
        let sse = r#"event: response.output_item.added
data: {"type":"response.output_item.added","item":{"id":"msg_123","type":"message","status":"in_progress","content":[],"phase":"final_answer","role":"assistant"},"output_index":0,"sequence_number":2}

event: response.output_text.delta
data: {"type":"response.output_text.delta","content_index":0,"delta":"Hey!","item_id":"msg_123","output_index":0,"sequence_number":3}

event: response.output_text.delta
data: {"type":"response.output_text.delta","content_index":0,"delta":" How can I help?","item_id":"msg_123","output_index":0,"sequence_number":4}

event: response.output_text.done
data: {"type":"response.output_text.done","content_index":0,"text":"Hey! How can I help?","item_id":"msg_123","output_index":0,"sequence_number":5}

event: response.completed
data: {"type":"response.completed","response":{"id":"resp_123","object":"response","model":"gpt-5.4","status":"completed","error":null,"output":[],"usage":null},"sequence_number":6}
"#;

        let response = CodexAdapter::parse_sse_response(sse).unwrap();

        assert_eq!(
            response.extract_text().as_deref(),
            Some("Hey! How can I help?")
        );
        assert_eq!(response.output.len(), 1);
    }
}
