//!
//! curl -X POST "https://api.cloudflare.com/client/v4/zones/89560cdd783e35f7a9d718755ea9c656/dns_records" \
//!      -H "Authorization: Bearer 9QjEiXC4B26tgshHZjuZ57kJcjaChSSsDfzUvfYQ" \
//!      -H "Content-Type: application/json" \
//!      --data '{"type":"A","name":"meow.n0des.xyz","content":"127.0.0.1","ttl":3600,"priority":10,"proxied":false,"comment":"Chain node record", "tags": ["owner": <guid>]}'
//!

use crate::auth::key_provider::KeyProvider;
use serde::Serialize;
use std::string::ToString;

pub type DnsResult<T> = Result<T, DnsError>;

#[derive(thiserror::Error, Debug)]
pub enum DnsError {
    #[error("Couldn't read env var: {0}")]
    EnvVar(#[from] std::env::VarError),
    #[error("Couldn't parse int val: {0}")]
    IntValue(#[from] std::num::ParseIntError),
    #[error("Couldn't read secret key: {0}")]
    SecretKey(#[from] crate::auth::key_provider::KeyProviderError),
    #[error("Error requesting DNS entry: {0}")]
    Http(#[from] reqwest::Error),
    #[error("Error handling JSON: {0}")]
    Json(#[from] serde_json::Error),
}

#[derive(Serialize)]
pub struct CloudflarePayload {
    pub r#type: String,
    pub name: String,
    pub content: String,
    pub ttl: i64,
    pub priority: i32,
    pub proxied: bool,
    pub tags: String,
}

impl CloudflarePayload {
    pub fn new(node_name: String, owner: uuid::Uuid) -> DnsResult<Self> {
        let name = format!("{node_name}.{}", std::env::var("CF_DNS_BASE")?);
        let ttl: i64 = std::env::var("CF_TTL")?.parse()?;

        Ok(Self {
            r#type: "A".to_string(),
            name,
            content: "127.0.0.1".to_string(),
            ttl,
            priority: 10,
            proxied: false,
            tags: format!("owner:{}", owner),
        })
    }
}

pub struct CloudflareApi {
    pub base_url: String,
    pub zone_id: String,
    pub token: String,
}

impl CloudflareApi {
    pub fn new() -> DnsResult<Self> {
        let zone_id = std::env::var("CF_ZONE")?;
        let base_url = std::env::var("CF_BASE_URL")?;
        let token = KeyProvider::get_var("CF_TOKEN")?.value;

        Ok(Self {
            base_url,
            zone_id,
            token,
        })
    }

    pub async fn create_node_dns(&self, node: crate::models::Node) -> DnsResult<bool> {
        let payload = CloudflarePayload::new(node.name, node.org_id)?;
        let endpoint = format!("zones/{}/dns_records", self.zone_id);

        self.post(payload, endpoint).await
    }

    async fn post(&self, payload: CloudflarePayload, endpoint: String) -> DnsResult<bool> {
        let url = format!("{}/{}", self.base_url, endpoint);
        let client = reqwest::Client::new();
        let res = client
            .post(url)
            .bearer_auth(&self.token)
            .json(&payload)
            .send()
            .await?;

        dbg!(res);

        Ok(false)
    }
}
