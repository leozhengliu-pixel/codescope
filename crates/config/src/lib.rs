use serde::{Deserialize, Serialize};
use std::env;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AppConfig {
    pub service_name: String,
    pub bind_addr: String,
    pub database_url: Option<String>,
}

impl Default for AppConfig {
    fn default() -> Self {
        Self {
            service_name: "sourcebot-api".to_string(),
            bind_addr: "127.0.0.1:3000".to_string(),
            database_url: None,
        }
    }
}

impl AppConfig {
    pub fn from_env() -> Self {
        Self {
            service_name: env::var("SOURCEBOT_SERVICE_NAME")
                .unwrap_or_else(|_| "sourcebot-api".to_string()),
            bind_addr: env::var("SOURCEBOT_BIND_ADDR")
                .unwrap_or_else(|_| "127.0.0.1:3000".to_string()),
            database_url: env::var("DATABASE_URL").ok(),
        }
    }

    pub fn public_view(&self) -> PublicAppConfig {
        PublicAppConfig {
            service_name: self.service_name.clone(),
            bind_addr: self.bind_addr.clone(),
            has_database_url: self.database_url.is_some(),
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PublicAppConfig {
    pub service_name: String,
    pub bind_addr: String,
    pub has_database_url: bool,
}
