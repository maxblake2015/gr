use crate::{
    config,
    groups::Groups,
    middlewares::{
        ErrorHandlingMiddleware, EventVerifierMiddleware, LoggerMiddleware, Nip09Middleware,
        Nip29Middleware, Nip42Middleware, Nip70Middleware, ValidationMiddleware,
    },
    nostr_database::RelayDatabase,
    nostr_session_state::{NostrConnectionFactory, NostrConnectionState},
};
use anyhow::Result;
use nostr_sdk::prelude::*;
use std::sync::Arc;
use websocket_builder::WebSocketBuilder;
pub use websocket_builder::WebSocketHandler;

#[derive(Clone)]
pub struct NostrMessageConverter;

impl websocket_builder::MessageConverter<ClientMessage, RelayMessage> for NostrMessageConverter {
    fn outbound_to_string(&self, message: RelayMessage) -> Result<String> {
        Ok(message.as_json())
    }

    fn inbound_from_string(&self, message: String) -> Result<Option<ClientMessage>> {
        if let Ok(client_message) = ClientMessage::from_json(&message) {
            Ok(Some(client_message))
        } else {
            Ok(None)
        }
    }
}

pub fn build_websocket_handler(
    relay_url: String,
    auth_url: String,
    groups: Arc<Groups>,
    relay_keys: &config::Keys,
    database: Arc<RelayDatabase>,
    websocket_settings: &config::WebSocketSettings,
) -> Result<
    WebSocketHandler<
        NostrConnectionState,
        ClientMessage,
        RelayMessage,
        NostrMessageConverter,
        NostrConnectionFactory,
    >,
> {
    let logger = LoggerMiddleware;
    let event_verifier = EventVerifierMiddleware;
    let nip_42 = Nip42Middleware::new(auth_url);
    let nip_70 = Nip70Middleware;
    let nip_29 = Nip29Middleware::new(groups, relay_keys.public_key(), database.clone());
    let validation_middleware = ValidationMiddleware::new(relay_keys.public_key());
    let nip_09 = Nip09Middleware::new(database.clone());
    let error_handler = ErrorHandlingMiddleware;

    let connection_state_factory = NostrConnectionFactory::new(relay_url)?;

    let mut builder = WebSocketBuilder::new(connection_state_factory, NostrMessageConverter);

    // Apply WebSocket settings from configuration
    builder = builder.with_channel_size(websocket_settings.channel_size());
    if let Some(max_time) = websocket_settings.max_connection_time() {
        builder = builder.with_max_connection_time(max_time);
    }

    if let Some(max_conns) = websocket_settings.max_connections() {
        builder = builder.with_max_connections(max_conns);
    }

    // Add middlewares in order of execution
    // Error handler must be first to catch all errors
    Ok(builder
        .with_middleware(error_handler)
        .with_middleware(logger)
        .with_middleware(nip_42)
        .with_middleware(validation_middleware)
        .with_middleware(event_verifier)
        .with_middleware(nip_70)
        .with_middleware(nip_09)
        .with_middleware(nip_29)
        .build())
}
