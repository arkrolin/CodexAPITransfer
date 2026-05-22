use async_stream::stream;
use axum::response::{
    sse::{Event, KeepAlive},
    Sse,
};
use eventsource_stream::Eventsource as EventsourceExt;
use futures_util::StreamExt;
use serde_json::{json, Value};
use std::collections::BTreeMap;
use std::sync::Arc;
use std::time::Duration;
use tokio::time::timeout;
use tracing::{error, warn};

use crate::{
    session::SessionStore,
    types::{ChatMessage, ChatRequest, ChatStreamChunk},
};

pub struct StreamArgs {
    pub client: reqwest::Client,
    pub url: String,
    pub api_key: Arc<String>,
    pub chat_req: ChatRequest,
    pub response_id: String,
    pub sessions: SessionStore,
    pub prior_messages: Vec<ChatMessage>,
    /// The fully translated request messages (including replayed history).
    /// Used to save correct session history so turn-level reasoning can be
    /// recovered when Codex replays the conversation without previous_response_id.
    pub request_messages: Vec<ChatMessage>,
    pub model: String,
    /// Per-chunk SSE read timeout in seconds (0 = no timeout).
    pub chunk_timeout_secs: u64,
}

struct ToolCallAccum {
    id: String,
    name: String,
    arguments: String,
}

/// Translate an upstream Chat Completions SSE stream into a Responses API SSE stream.
///
/// Text response event sequence:
///   response.created → response.output_item.added (message) → response.output_text.delta*
///   → response.output_item.done → response.completed
///
/// Tool call response event sequence:
///   response.created → [accumulate deltas] → response.output_item.added (function_call)
///   → response.function_call_arguments.delta → response.output_item.done → response.completed
pub fn translate_stream(
    args: StreamArgs,
) -> Sse<impl futures_util::Stream<Item = Result<Event, std::convert::Infallible>>> {
    let StreamArgs {
        client,
        url,
        api_key,
        chat_req,
        response_id,
        sessions,
        prior_messages,
        request_messages,
        model,
        chunk_timeout_secs,
    } = args;
    let msg_item_id = format!("msg_{}", uuid::Uuid::new_v4().simple());

    let event_stream = stream! {
        yield Ok(Event::default()
            .event("response.created")
            .data(json!({
                "type": "response.created",
                "response": { "id": &response_id, "status": "in_progress", "model": &model }
            }).to_string()));

        let mut builder = client.post(&url).header("Content-Type", "application/json");
        if !api_key.is_empty() {
            builder = builder.bearer_auth(api_key.as_str());
        }

        let upstream = match builder.json(&chat_req).send().await {
            Ok(r) if r.status().is_success() => r,
            Ok(r) => {
                let status = r.status();
                let body = r.text().await.unwrap_or_default();
                error!("upstream {status}: {body}");
                yield Ok(Event::default().event("response.failed").data(
                    json!({"type": "response.failed", "response": {"id": &response_id, "status": "failed", "error": {"code": status.as_u16().to_string(), "message": body}}}).to_string()
                ));
                return;
            }
            Err(e) => {
                error!("upstream request failed: {e}");
                yield Ok(Event::default().event("response.failed").data(
                    json!({"type": "response.failed", "response": {"id": &response_id, "status": "failed", "error": {"code": "connection_error", "message": e.to_string()}}}).to_string()
                ));
                return;
            }
        };

        let mut accumulated_text = String::new();
        let mut accumulated_reasoning = String::new();
        let mut tool_calls: BTreeMap<usize, ToolCallAccum> = BTreeMap::new();
        let mut emitted_message_item = false;
        let mut source = upstream.bytes_stream().eventsource();

        // ── SSE chunk read with 120s timeout ──────────────────────────
        // Per-chunk timeout prevents indefinite hangs when the upstream
        // connection stalls mid-stream (half-open TCP, proxy timeout, etc.).
        // 120s is generous enough for long reasoning/thinking phases
        // (kimi-k2.6, deepseek-v4-pro) while still catching real stalls.
        loop {
            let chunk_timeout = if chunk_timeout_secs > 0 { Duration::from_secs(chunk_timeout_secs) } else { Duration::from_secs(86400) };
            let chunk_result = timeout(chunk_timeout, source.next()).await;
            let ev = match chunk_result {
                Ok(Some(ev)) => ev,
                Ok(None) => break, // stream ended normally
                Err(_elapsed) => {
                    error!("SSE stream stalled for {}s (upstream may be overloaded or disconnected)", chunk_timeout.as_secs());
                    yield Ok(Event::default().event("response.failed").data(
                        json!({"type": "response.failed", "response": {"id": &response_id, "status": "failed", "error": {"code": "stream_timeout", "message": format!("Upstream stream stalled: no data for {} seconds", chunk_timeout.as_secs())}}}).to_string()
                    ));
                    return;
                }
            };
            match ev {
                Err(e) => {
                    warn!("SSE parse error: {e}");
                    break;
                }
                Ok(ev) if ev.data.trim() == "[DONE]" => break,
                Ok(ev) if ev.data.is_empty() => continue,
                Ok(ev) => {
                    match serde_json::from_str::<ChatStreamChunk>(&ev.data) {
                        Err(e) => warn!("chunk parse error: {e} — data: {}", ev.data),
                        Ok(chunk) => {
                            for choice in &chunk.choices {
                                // Reasoning/thinking content (kimi-k2.6 etc.)
                                if let Some(rc) = choice.delta.reasoning_content.as_deref() {
                                    if !rc.is_empty() {
                                        accumulated_reasoning.push_str(rc);
                                    }
                                }

                                // Text content
                                let content = choice.delta.content.as_deref().unwrap_or("");
                                if !content.is_empty() {
                                    if !emitted_message_item {
                                        yield Ok(Event::default()
                                            .event("response.output_item.added")
                                            .data(json!({
                                                "type": "response.output_item.added",
                                                "output_index": 0,
                                                "item": { "type": "message", "id": &msg_item_id, "role": "assistant", "content": [], "status": "in_progress" }
                                            }).to_string()));
                                        emitted_message_item = true;
                                    }
                                    accumulated_text.push_str(content);
                                    yield Ok(Event::default()
                                        .event("response.output_text.delta")
                                        .data(json!({
                                            "type": "response.output_text.delta",
                                            "item_id": &msg_item_id,
                                            "output_index": 0,
                                            "content_index": 0,
                                            "delta": content
                                        }).to_string()));
                                }

                                // Tool call deltas — accumulate by index
                                if let Some(delta_calls) = &choice.delta.tool_calls {
                                    for dc in delta_calls {
                                        let entry = tool_calls.entry(dc.index).or_insert(ToolCallAccum {
                                            id: String::new(),
                                            name: String::new(),
                                            arguments: String::new(),
                                        });
                                        if let Some(id) = &dc.id {
                                            if !id.is_empty() { entry.id.clone_from(id); }
                                        }
                                        if let Some(func) = &dc.function {
                                            if let Some(n) = &func.name {
                                                if !n.is_empty() { entry.name.push_str(n); }
                                            }
                                            if let Some(a) = &func.arguments {
                                                entry.arguments.push_str(a);
                                            }
                                        }
                                    }
                                }
                            }
                        }
                    }
                }
            }
        }

        // Close message item if one was opened
        if emitted_message_item {
            yield Ok(Event::default()
                .event("response.output_item.done")
                .data(json!({
                    "type": "response.output_item.done",
                    "output_index": 0,
                    "item": {
                        "type": "message",
                        "id": &msg_item_id,
                        "role": "assistant",
                        "status": "completed",
                        "content": [{"type": "output_text", "text": &accumulated_text}]
                    }
                }).to_string()));
        }

        // Emit function_call items for each accumulated tool call
        let base_index: usize = if emitted_message_item { 1 } else { 0 };
        let mut fc_items: Vec<Value> = Vec::new();

        for (rel_idx, (_, tc)) in tool_calls.iter().enumerate() {
            let fc_item_id = format!("fc_{}", uuid::Uuid::new_v4().simple());
            let output_index = base_index + rel_idx;

            yield Ok(Event::default()
                .event("response.output_item.added")
                .data(json!({
                    "type": "response.output_item.added",
                    "output_index": output_index,
                    "item": {
                        "type": "function_call",
                        "id": &fc_item_id,
                        "call_id": &tc.id,
                        "name": &tc.name,
                        "arguments": "",
                        "status": "in_progress"
                    }
                }).to_string()));

            if !tc.arguments.is_empty() {
                yield Ok(Event::default()
                    .event("response.function_call_arguments.delta")
                    .data(json!({
                        "type": "response.function_call_arguments.delta",
                        "item_id": &fc_item_id,
                        "output_index": output_index,
                        "delta": &tc.arguments
                    }).to_string()));
            }

            yield Ok(Event::default()
                .event("response.output_item.done")
                .data(json!({
                    "type": "response.output_item.done",
                    "output_index": output_index,
                    "item": {
                        "type": "function_call",
                        "id": &fc_item_id,
                        "call_id": &tc.id,
                        "name": &tc.name,
                        "arguments": &tc.arguments,
                        "status": "completed"
                    }
                }).to_string()));

            fc_items.push(json!({
                "type": "function_call",
                "id": fc_item_id,
                "call_id": &tc.id,
                "name": &tc.name,
                "arguments": &tc.arguments,
                "status": "completed"
            }));
        }

        // Persist turn to session store
        // Store reasoning_content per call_id so translate.rs can inject it
        // back when Codex replays function_call items in the next request.
        for tc in tool_calls.values() {
            if !tc.id.is_empty() {
                sessions.store_reasoning(tc.id.clone(), accumulated_reasoning.clone());
            }
        }

        let assistant_tool_calls: Option<Vec<Value>> = if tool_calls.is_empty() {
            None
        } else {
            Some(tool_calls.values().map(|tc| json!({
                "id": &tc.id,
                "type": "function",
                "function": { "name": &tc.name, "arguments": &tc.arguments }
            })).collect())
        };
        let assistant_msg = ChatMessage {
            role: "assistant".into(),
            content: if accumulated_text.is_empty() { None } else { Some(accumulated_text.clone()) },
            reasoning_content: if accumulated_reasoning.is_empty() { None } else { Some(accumulated_reasoning.clone()) },
            tool_calls: assistant_tool_calls,
            tool_call_id: None,
            name: None,
        };

        // Index reasoning by turn fingerprint so it can be recovered when
        // Codex replays the full conversation in input[] without previous_response_id.
        if !accumulated_reasoning.is_empty() {
            sessions.store_turn_reasoning(&request_messages, &assistant_msg, accumulated_reasoning.clone());
        }

        let mut messages = prior_messages;
        messages.push(assistant_msg);
        sessions.save_with_id(response_id.clone(), messages);

        // Build output array for response.completed
        let mut output_items: Vec<Value> = Vec::new();
        if emitted_message_item {
            output_items.push(json!({
                "type": "message",
                "id": &msg_item_id,
                "role": "assistant",
                "status": "completed",
                "content": [{"type": "output_text", "text": &accumulated_text}]
            }));
        }
        output_items.extend(fc_items);

        yield Ok(Event::default()
            .event("response.completed")
            .data(json!({
                "type": "response.completed",
                "response": {
                    "id": &response_id,
                    "status": "completed",
                    "model": &model,
                    "output": output_items
                }
            }).to_string()));
    };

    Sse::new(event_stream).keep_alive(KeepAlive::default())
}