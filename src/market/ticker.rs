use std::collections::HashMap;
use std::sync::Arc;
use tokio::sync::RwLock;

use crate::events::bus::TickerData;

/// In-memory ticker cache for latest ticker data per instrument
#[derive(Clone)]
pub struct TickerCache {
    tickers: Arc<RwLock<HashMap<String, TickerData>>>,
}

impl TickerCache {
    pub fn new() -> Self {
        TickerCache {
            tickers: Arc::new(RwLock::new(HashMap::new())),
        }
    }

    pub async fn update(&self, instrument_name: &str, data: TickerData) {
        self.tickers
            .write()
            .await
            .insert(instrument_name.to_string(), data);
    }

    pub async fn get(&self, instrument_name: &str) -> Option<TickerData> {
        self.tickers.read().await.get(instrument_name).cloned()
    }

    pub async fn get_all(&self) -> HashMap<String, TickerData> {
        self.tickers.read().await.clone()
    }

    pub async fn len(&self) -> usize {
        self.tickers.read().await.len()
    }
}
