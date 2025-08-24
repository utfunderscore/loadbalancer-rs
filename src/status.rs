use crate::finder::ServerFinder;
use pumpkin_protocol::java::client::status::CStatusResponse;
use pumpkin_protocol::{Players, StatusResponse, Version};
use std::collections::HashMap;
use std::time::{Duration, Instant};
use tokio::sync::MutexGuard;

pub struct StatusCache {
    count: u32,
    last_updated: Instant,
    cache: HashMap<(String, u32, u32), String>,
}

impl Default for StatusCache {
    fn default() -> Self {
        Self::new()
    }
}

impl StatusCache {
    pub fn new() -> Self {
        StatusCache {
            count: 0,
            last_updated: Instant::now() - Duration::from_secs(60),
            cache: HashMap::new(),
        }
    }

    pub async fn get_status_response(
        &mut self,
        motd: String,
        protocol: u32,
        server_finder: MutexGuard<'_, Box<dyn ServerFinder>>,
    ) -> CStatusResponse {
        if self.last_updated.elapsed().as_secs() > 15 {
            self.count = server_finder.get_player_count().await;
            self.last_updated = Instant::now();
        }

        if let Some(cached) = self.cache.get(&(motd.clone(), protocol, self.count)) {
            return CStatusResponse::new(cached.clone());
        }

        let response = self.build_status_response(motd.clone(), protocol, self.count);
        self.cache
            .insert((motd, protocol, self.count), response.clone());

        CStatusResponse::new(response)
    }

    fn build_status_response(&self, motd: String, protocol: u32, player_count: u32) -> String {
        let response = StatusResponse {
            version: Some(Version {
                name: "Loadbalancer".to_string(),
                protocol,
            }),
            players: Some(Players {
                max: 1000,
                online: player_count,
                sample: Vec::new(),
            }),
            description: motd,
            favicon: None,
            enforce_secure_chat: false,
        };

        serde_json::to_string(&response).unwrap_or_default()
    }
}
