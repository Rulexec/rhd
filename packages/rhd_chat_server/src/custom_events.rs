//! Custom event broadcasting and acknowledgment helpers.

use chrono::Utc;
use uuid::Uuid;

use rhd_chat_api::common::PendingEvent;
use rhd_chat_api::events::{CustomEventAcknowledgedData, CustomEventData};
use rhd_chat_api::protocol::Event;

use rhd_db::ChatDb;

use crate::error::ServerError;

/// Create and store a custom event, returning the event and its ID.
pub fn create_custom_event(
    db: &ChatDb,
    event_name: &str,
    sender_plugin_id: Option<&str>,
    additional: Option<&str>,
) -> Result<(String, Event), ServerError> {
    let event_id = Uuid::new_v4().to_string();
    let created_at = Utc::now();

    // Store in database
    db.create_custom_event(&event_id, event_name, sender_plugin_id, additional)?;

    // Create event for broadcasting
    let data = CustomEventData {
        event_id: event_id.clone(),
        event_name: event_name.to_string(),
        sender_plugin_id: sender_plugin_id.map(|s| s.to_string()),
        additional: additional.map(|s| s.to_string()),
        created_at,
    };

    let event = Event::new("customEvent", serde_json::to_value(data)?);

    Ok((event_id, event))
}

/// Acknowledge a custom event and create the acknowledgment event.
pub fn ack_custom_event(
    db: &ChatDb,
    event_id: &str,
    plugin_id: &str,
) -> Result<Option<Event>, ServerError> {
    // Check if event exists
    let event_info = db.get_custom_event(event_id)?;
    let event_info = match event_info {
        Some(e) => e,
        None => return Ok(None),
    };

    // Store acknowledgment
    db.ack_custom_event(event_id, plugin_id)?;

    // Create acknowledgment event for the sender
    if let Some(_sender_plugin_id) = &event_info.sender_plugin_id {
        let data = CustomEventAcknowledgedData {
            event_id: event_id.to_string(),
            acknowledging_plugin_id: plugin_id.to_string(),
        };
        let event = Event::new("customEventAcknowledged", serde_json::to_value(data)?);
        Ok(Some(event))
    } else {
        Ok(None)
    }
}

/// Get pending events for a plugin and convert to API types.
pub fn get_pending_events(
    db: &ChatDb,
    plugin_id: &str,
) -> Result<Vec<PendingEvent>, ServerError> {
    let events = db.get_pending_events_for_plugin(plugin_id)?;
    let pending_events = events
        .into_iter()
        .map(|e| {
            let created_at = e.created_at.parse().unwrap_or_else(|_| Utc::now());
            PendingEvent {
                event_id: e.event_id,
                event_name: e.event_name,
                sender_plugin_id: e.sender_plugin_id,
                additional: e.additional,
                created_at,
            }
        })
        .collect();
    Ok(pending_events)
}
