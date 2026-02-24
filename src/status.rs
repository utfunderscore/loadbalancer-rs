use std::collections::HashMap;
use std::time::{Duration, Instant};
use tokio::sync::MutexGuard;
use crate::finder::ServerFinder;
use crate::protocol::packets::c2s_status_request::C2SStatusRequest;
use crate::protocol::packets::s2c_status_response::{Description, Players, ServerStatus, StatusResponse, Version};

pub struct StatusCache {
    count: u32,
    last_updated: Instant,
    cache: HashMap<(String, u32, u32), ServerStatus>,
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
    ) -> anyhow::Result<StatusResponse> {
        if self.last_updated.elapsed().as_secs() > 15 {
            self.count = server_finder.get_player_count().await;
            self.last_updated = Instant::now();
        }

        if let Some(cached) = self.cache.get(&(motd.clone(), protocol, self.count)) {
            return StatusResponse::new(cached.clone());
        }

        let response = ServerStatus {
            version: Some(Version {
                name: "Loadbalancer".to_string(),
                protocol,
            }),
            players: Some(Players {
                max: 1000,
                online: self.count,
                sample: Vec::new(),
            }),
            description: Description {
                text: motd.clone(),
            },
            favicon: None,
            enforces_secure_chat: false,
        };


        self.cache.insert((motd, protocol, self.count), response.clone());

        StatusResponse::new(response)
    }

}
