//! Chat commands — message send (non-streaming + streaming), stream cancel,
//! and network status.

use crate::chat;
use crate::context;
use crate::db::Database;
use crate::keyring;
use crate::network;
use crate::settings;
use crate::trial;

/// Send a message to the active AI provider and get a response (non-streaming)
#[tauri::command]
pub(crate) async fn send_chat_message(
    state: tauri::State<'_, Database>,
    messages: Vec<chat::ChatMessage>,
    system_prompt: Option<String>,
    conversation_id: Option<String>,
    employee_ids_used: Option<Vec<String>>,
) -> Result<chat::ChatResponse, chat::ChatError> {
    let active = chat::resolve_active_provider(&state.pool)
        .await
        .map_err(|e| chat::ChatError::RequestError(e.to_string()))?;

    // #112: the chat seam writes the audit row for every attempt.
    let audit = crate::audit::EgressAudit {
        source: crate::audit::EgressSource::Interactive,
        conversation_id,
        employee_ids: employee_ids_used.unwrap_or_default(),
        query_category: None,
    };

    chat::send_message(
        &state.pool,
        audit,
        messages,
        system_prompt,
        &active.provider_id,
        active.model_id.as_deref(),
    )
    .await
}

/// Send a message with streaming response
/// Emits "chat-stream" events as response chunks arrive
///
/// Dual-path routing: trial mode goes through proxy, paid mode uses BYOK key directly.
///
/// `stream_id` is a client-generated UUID. The frontend passes the same id
/// to `cancel_stream` when the user hits Stop, switches conversations, or
/// unmounts the chat view (issue #25). Unknown ids are a no-op, so it's safe
/// for the UI to always call cancel on navigation even if the stream ended.
///
/// Optional for backward compatibility while the frontend wire-up lands as
/// a follow-up. When absent we generate a UUID; the stream runs normally but
/// the UI can't cancel it (cancel_stream has no way to learn the generated
/// id). Once the frontend always passes an id, this can become required.
#[tauri::command]
pub(crate) async fn send_chat_message_streaming(
    app: tauri::AppHandle,
    state: tauri::State<'_, Database>,
    registry: tauri::State<'_, chat::StreamRegistry>,
    stream_id: Option<String>,
    messages: Vec<chat::ChatMessage>,
    system_prompt: Option<String>,
    aggregates: Option<context::OrgAggregates>,
    query_type: Option<context::QueryType>,
    conversation_id: Option<String>,
    employee_ids_used: Option<Vec<String>>,
) -> Result<(), chat::ChatError> {
    let stream_id = stream_id.unwrap_or_else(|| uuid::Uuid::new_v4().to_string());
    // #112: the chat seam writes the audit row for every attempt (success,
    // stream error, cancel). conversation_id/employee_ids come from the
    // frontend, which owns that context; absent values degrade to an
    // unattributed-but-present row, never a missing one.
    let audit = crate::audit::EgressAudit {
        source: crate::audit::EgressSource::Interactive,
        conversation_id,
        employee_ids: employee_ids_used.unwrap_or_default(),
        query_category: None,
    };
    let has_license = trial::has_license_key(&state.pool)
        .await
        .map_err(|e| chat::ChatError::TrialError(e.to_string()))?;

    // Read the user's active provider preference
    let provider_id = settings::get_setting(&state.pool, "active_provider")
        .await
        .map_err(|e| chat::ChatError::RequestError(e.to_string()))?
        .unwrap_or_else(|| "anthropic".to_string());

    let has_key = keyring::has_provider_api_key(&provider_id);

    if !has_license {
        // Get proxy config — trial always uses Anthropic via proxy
        let proxy_url = trial::get_proxy_url(&state.pool)
            .await
            .map_err(|e| chat::ChatError::TrialError(e.to_string()))?;
        let device_id = trial::get_device_id(&state.pool)
            .await
            .map_err(|e| chat::ChatError::TrialError(e.to_string()))?;
        let proxy_signing_secret = trial::get_proxy_signing_secret(&state.pool)
            .await
            .map_err(|e| chat::ChatError::TrialError(e.to_string()))?;

        // Route through proxy (trial doesn't support model selection)
        let trial_usage = chat::send_message_streaming_trial(
            app,
            &registry,
            stream_id,
            &state.pool,
            audit,
            messages,
            system_prompt,
            &proxy_url,
            &device_id,
            proxy_signing_secret.as_deref(),
            aggregates,
            query_type,
        )
        .await;

        let trial_usage = match trial_usage {
            Ok(usage) => usage,
            Err(chat::ChatError::TrialLimitReached { used, limit }) => {
                if let Some(server_used) = used {
                    let _ = trial::set_trial_messages_used(&state.pool, server_used).await;
                }
                return Err(chat::ChatError::TrialLimitReached { used, limit });
            }
            Err(other) => return Err(other),
        };

        // Sync local counter from proxy metadata when available.
        if let Some(used) = trial_usage.used {
            let _ = trial::set_trial_messages_used(&state.pool, used).await;
        } else {
            let _ = trial::increment_trial_messages(&state.pool).await;
        }

        Ok(())
    } else if !has_key {
        Err(chat::ChatError::NoApiKey)
    } else {
        // Paid mode: read user's selected model for this provider
        let model_key = format!("active_model_{}", provider_id);
        let model_id = settings::get_setting(&state.pool, &model_key)
            .await
            .map_err(|e| chat::ChatError::RequestError(e.to_string()))?;

        chat::send_message_streaming(
            app, &registry, stream_id, &state.pool, audit, messages, system_prompt,
            aggregates, query_type, &provider_id, model_id.as_deref(),
        ).await
    }
}

/// Cancel an in-flight streaming request by its client-generated stream_id.
///
/// Returns true if a matching stream was found and cancelled, false if the
/// id was unknown (e.g., the stream completed before the UI could call
/// cancel). Cancelling drops the upstream HTTP connection, so the provider
/// stops generating and billing for tokens the user won't see.
///
/// Calling this with an unknown id is intentionally safe — the frontend can
/// call it on conversation-switch / unmount without tracking whether a
/// stream is actually in flight.
#[tauri::command]
pub(crate) fn cancel_stream(registry: tauri::State<'_, chat::StreamRegistry>, stream_id: String) -> bool {
    registry.cancel(&stream_id)
}

/// Check if the network and Anthropic API are reachable
#[tauri::command]
pub(crate) async fn check_network_status(
    state: tauri::State<'_, Database>,
) -> Result<network::NetworkStatus, ()> {
    Ok(network::check_network(&state.pool).await)
}

/// Quick check if online (returns just a boolean)
#[tauri::command]
pub(crate) async fn is_online(state: tauri::State<'_, Database>) -> Result<bool, ()> {
    Ok(network::is_online(&state.pool).await)
}
