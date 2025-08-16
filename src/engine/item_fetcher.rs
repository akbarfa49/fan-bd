use anyhow::{Context, Ok, Result, anyhow};
use async_trait::async_trait;
use chrono::Days;
use reqwest::Client;
use serde::{Deserialize, Serialize};
use serde_json::{Map, Value};
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
    pub region: String,
}

impl DefaultFetcher {
    pub fn new() -> Self {
        Self {
            client: Client::new(),
            region: "SEA".to_string(),
        }
    }
}

async fn search_id_by_name(client: &Client, name: &str) -> Result<u64> {
    let response = client
        .get(format!(
            "https://apiv2.bdolytics.com/en/SEA/db/query-extended?q={}",
            name
        ))
        .send()
        .await?;
    // println!("{}", response.text().await?);
    // return Ok(1);
    // let search_result: BdolyticsSearchResult;
    let search_result = response
        .json::<BdolyticsSearchResult>()
        .await
        .context("Failed to deserialize JSON into BdolyticsSearchResult")?;

    if !search_result.status.success {
        return Err(anyhow!("API request was not successful"));
    }
    let mut data = search_result.data;
    data = data
        .into_iter()
        .filter(|i| i.db_type == "item" && i.name == name)
        .collect();
    data.sort_by_key(|item| item.id);
    if data.len() == 0 {
        return Err(anyhow!("data doesnt exist"));
    }
    let mut item = data.first().unwrap();
    if item.grade_type.unwrap() != 0 {
        item = data.last().unwrap();
    }
    Ok(item.id)
}
async fn item_detail(
    client: &Client,
    region: &str,
    item_id: u64,
) -> Result<BdolyticsItemDetailResultData> {
    let response = client
        .get(format!("https://bdolytics.com/api/trpc/database.getEntity?input={{\"id\":{},\"dbType\":\"item\",\"region\":\"{}\",\"language\":\"en\"}}", item_id,region))
        .send()
        .await?;
    // println!("{}", response.text().await?);
    // return Ok(1);

    let item_detail = response.json::<BdolyticsItemDetailOut>().await?;

    if item_detail.error.is_some() {
        return Err(anyhow!("API request was not successful"));
    }

    Ok(item_detail.result.data)
}

async fn market_data(
    client: &Client,
    region: &str,
    item_id: u64,
    enhancement_level: u8,
) -> Result<Vec<Trade>> {
    let now = chrono::Utc::now();
    let start_date = now.checked_sub_days(Days::new(1)).unwrap();
    let response = client
        .get(format!("https://apiv2.bdolytics.com/market/analytics/{}?start_date={}&end_date={}&region={}&enhancement_level={}", item_id,start_date.timestamp_millis(),now.timestamp_millis(),region, enhancement_level))
        .send()
        .await?;

    let market_detail = response.json::<BdolyticsMarketAnalytics>().await?;

    if market_detail.error.is_some() {
        return Err(anyhow!("API request was not successful"));
    }

    Ok(market_detail.data)
}
#[async_trait]
impl ItemFetcher for DefaultFetcher {
    async fn get_data_by_name(&self, item_name: &str) -> Result<ItemData> {
        let id = search_id_by_name(&self.client, item_name)
            .await
            .context("failed to search id by name")?;
        let detail = item_detail(&self.client, &self.region, id)
            .await
            .context("failed to find item detail")?;

        let mut item_data = ItemData {
            id,
            name: item_name.to_string(),
            vendor_buy_price: detail.buy_price,
            vendor_sell_price: detail.sell_price,
            market_buy_price: 0,
            market_sell_price: 0,
        };
        if item_name == "Silver" {
            item_data.vendor_sell_price = 1
        }
        if detail.has_market_data {
            let item_market_data = market_data(&self.client, &self.region, id, 0)
                .await
                .context("failed to get item market data")?;
            item_data.market_buy_price = item_market_data[0].price;
            item_data.market_sell_price = item_market_data[0].price;
        }
        Ok(item_data)
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
    pub grade_type: Option<i64>,
    pub db_type: String,
}

#[derive(Debug, Deserialize, Serialize)]
pub struct Status {
    pub success: bool,
}

#[derive(Default, Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct BdolyticsItemDetailOut {
    pub result: BdolyticsItemDetailResult,
    pub error: Option<Map<String, Value>>,
}

#[derive(Default, Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct BdolyticsItemDetailResult {
    pub data: BdolyticsItemDetailResultData,
}

#[derive(Default, Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct BdolyticsItemDetailResultData {
    pub id: i64,
    pub sub_id: i64,
    pub name: String,
    pub description: String,
    pub icon_image: String,
    pub grade_type: i64,
    pub weight: f64,
    pub buy_price: u64,
    pub sell_price: u64,
    // pub repair_price: i64,
    pub has_market_data: bool,
    // pub expiration_period: i64,
    // pub main_category: String,
    // pub sub_category: String,
    pub db_type: String,
}

// API Response Types
#[derive(Debug, Deserialize)]
pub struct BdolyticsMarketAnalytics {
    pub data: Vec<Trade>,
    pub error: Option<Map<String, Value>>,
}
#[derive(Debug)]
pub struct Trade {
    id: u64,
    price: u64,
    volume: u64,
    stock: u64,
}

#[derive(Debug, Deserialize)]
struct TradeRaw(u64, u64, u64, u64);

impl From<TradeRaw> for Trade {
    fn from(raw: TradeRaw) -> Self {
        Trade {
            id: raw.0,
            price: raw.1,
            volume: raw.2,
            stock: raw.3,
        }
    }
}

impl<'de> Deserialize<'de> for Trade {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: serde::Deserializer<'de>,
    {
        TradeRaw::deserialize(deserializer).map(Trade::from)
    }
}

#[cfg(test)]
mod test_bdolytics {
    use reqwest::Client;

    use crate::engine::item_fetcher::{item_detail, market_data, search_id_by_name};

    #[tokio::test]
    async fn test_search_item_by_name() {
        let client = Client::new();
        let result = search_id_by_name(&client, "Narc Magic Mark").await.unwrap();
        assert_eq!(result, 59820);
    }
    #[tokio::test]
    async fn test_search_item_detail() {
        let client = Client::new();
        let result = search_id_by_name(&client, "Caphras Stone").await.unwrap();
        let detail = item_detail(&client, "SEA", result).await.unwrap();
        let market_data = market_data(&client, "SEA", result, 0).await.unwrap();
        assert_eq!(result, 721003);
        assert_eq!(detail.has_market_data, true);
        assert!(!market_data.is_empty());
    }
}
