use tokio::sync::broadcast;

use crate::analysis::opportunity::Opportunity;

#[derive(Debug, Clone)]
pub enum Event {
    TickerUpdate {
        instrument_name: String,
        data: TickerData,
    },
    OrderbookUpdate {
        instrument_name: String,
        bids: Vec<(f64, f64)>,
        asks: Vec<(f64, f64)>,
    },
    InstrumentsLoaded {
        count: usize,
    },
    OpportunityFound(Opportunity),
}

#[derive(Debug, Clone)]
pub struct TickerData {
    pub mark_price: f64,
    pub mark_iv: f64,
    pub best_bid_price: Option<f64>,
    pub best_ask_price: Option<f64>,
    pub best_bid_amount: f64,
    pub best_ask_amount: f64,
    pub open_interest: f64,
    pub underlying_price: f64,
    pub delta: f64,
    pub gamma: f64,
    pub vega: f64,
    pub theta: f64,
    pub timestamp: i64,
}

#[derive(Clone)]
pub struct EventBus {
    sender: broadcast::Sender<Event>,
}

impl EventBus {
    pub fn new(capacity: usize) -> Self {
        let (sender, _) = broadcast::channel(capacity);
        EventBus { sender }
    }

    pub fn publish(&self, event: Event) {
        // Ignore error when no receivers
        let _ = self.sender.send(event);
    }

    pub fn subscribe(&self) -> broadcast::Receiver<Event> {
        self.sender.subscribe()
    }
}
