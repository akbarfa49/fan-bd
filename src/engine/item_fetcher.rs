use anyhow::{Ok, Result, anyhow};
use async_trait::async_trait;
use reqwest::Client;
use serde::{Deserialize, Serialize};
use std::fmt;

#[derive(Debug, Clone)]
pub struct ItemData {
    pub id: u64,
    pub name: String,
    pub vendor_buy_price: u64,
    pub vendor_sell_price: u64,
    pub market_buy_price: u64,
    pub market_sell_price: u64,
}

#[async_trait]
pub trait ItemFetcher: fmt::Debug + Send + Sync {
    async fn get_data_by_name(&self, item_name: &str) -> Result<ItemData>;
}

#[derive(Debug, Clone)]
pub struct DefaultFetcher {
    client: Client,
}

impl DefaultFetcher {
    pub fn new() -> Self {
        Self {
            client: Client::new(),
        }
    }
}

async fn search_id_by_name(client: &Client, name: &str) -> Result<u64> {
    let response = client
        .get(format!("https://bdolytics.com/en/SEA/db/search?q={}", name))
        .send()
        .await?;

    let search_result = response.json::<BdolyticsSearchResult>().await?;

    if !search_result.status.success {
        return Err(anyhow!("API request was not successful"));
    }

    let item = search_result
        .data
        .into_iter()
        .filter(|item| item.db_type == "item")
        .max_by_key(|item| item.id)
        .ok_or_else(|| anyhow!("No matching item found"))?;

    Ok(item.id)
}
async fn item_detail(client: &Client, region: &str, item_id: u64) -> Result<u64> {
    Ok(1)
}

#[async_trait]
impl ItemFetcher for DefaultFetcher {
    async fn get_data_by_name(&self, item_name: &str) -> Result<ItemData> {
        let id = search_id_by_name(&self.client, item_name).await?;

        Ok(ItemData {
            id,
            name: item_name.to_string(),
            vendor_buy_price: 0,
            vendor_sell_price: 0,
            market_buy_price: 0,
            market_sell_price: 0,
        })
    }
}

#[derive(Debug)]
pub enum Fetcher {
    Default(DefaultFetcher),
    // Add other fetcher variants here as needed
}

// impl Fetcher {
//     pub async fn get_data_by_name(&self, item_name: &str) -> Result<ItemData> {
//         match self {
//             Fetcher::Default(fetcher) => fetcher.get_data_by_name(item_name).await,
//         }
//     }
// }

#[async_trait]
impl ItemFetcher for Fetcher {
    async fn get_data_by_name(&self, item_name: &str) -> Result<ItemData> {
        match self {
            Fetcher::Default(fetcher) => fetcher.get_data_by_name(item_name).await,
        }
    }
}

// API Response Types
#[derive(Debug, Deserialize, Serialize)]
pub struct BdolyticsSearchResult {
    pub data: Vec<BdolyticsSearchResultData>,
    pub status: Status,
}

#[derive(Debug, Deserialize, Serialize)]
pub struct BdolyticsSearchResultData {
    pub id: u64,
    pub name: String,
    pub grade_type: i64,
    pub db_type: String,
}

#[derive(Debug, Deserialize, Serialize)]
pub struct Status {
    pub success: bool,
}
