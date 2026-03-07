mod alert;
mod analysis;
mod config;
mod events;
mod market;
mod storage;
mod ws;

use anyhow::Result;
use serde_json::json;
use tokio::time::{interval, Duration};
use tracing::{error, info, warn};

use crate::analysis::box_spread::BoxSpreadAnalyzer;
use crate::analysis::calendar_arb::CalendarArbAnalyzer;
use crate::analysis::calendar_spread::CalendarSpreadAnalyzer;
use crate::analysis::conversion::ConversionAnalyzer;
use crate::analysis::put_call_parity::PutCallParityAnalyzer;
use crate::analysis::vertical_arb::VerticalArbAnalyzer;
use crate::analysis::vol_surface::VolSurfaceAnalyzer;
use crate::config::Config;
use crate::events::bus::{Event, EventBus};
use crate::market::instruments::InstrumentRegistry;
use crate::market::orderbook::OrderBookManager;
use crate::market::subscriber::Subscriber;
use crate::market::ticker::TickerCache;
use crate::storage::sqlite::Storage;
use crate::ws::client::WsManager;

#[tokio::main]
async fn main() -> Result<()> {
    tracing_subscriber::fmt()
        .with_env_filter(
            tracing_subscriber::EnvFilter::try_from_default_env()
                .unwrap_or_else(|_| tracing_subscriber::EnvFilter::new("info")),
        )
        .init();

    info!("Deribit BTC Options Trading System starting...");

    let config = Config::from_env()?;
    info!(
        env = if config.ws_url.contains("test") { "test" } else { "prod" },
        "Configuration loaded"
    );

    let event_bus = EventBus::new(4096);
    let registry = InstrumentRegistry::new();
    let ticker_cache = TickerCache::new();
    let orderbook_manager = OrderBookManager::new();
    let storage = Storage::new(&config.db_path)?;

    let ws_manager = WsManager::new(config.clone(), event_bus.clone());
    let ws_client = ws_manager.client();

    // WS connection loop
    let _ws_handle = tokio::spawn(async move {
        if let Err(e) = ws_manager.run().await {
            error!(error = %e, "WebSocket manager fatal error");
        }
    });

    // Ticker event processor
    let ticker_cache_writer = ticker_cache.clone();
    let storage_ticker = storage.clone();
    let event_bus_ticker = event_bus.clone();
    tokio::spawn(async move {
        let mut rx = event_bus_ticker.subscribe();
        let mut save_counter: u64 = 0;
        loop {
            match rx.recv().await {
                Ok(Event::TickerUpdate {
                    instrument_name,
                    data,
                }) => {
                    ticker_cache_writer
                        .update(&instrument_name, data.clone())
                        .await;
                    save_counter += 1;
                    if save_counter % 100 == 0 {
                        if let Err(e) =
                            storage_ticker.save_ticker(&instrument_name, &data).await
                        {
                            warn!(error = %e, "Failed to save ticker");
                        }
                    }
                }
                Err(tokio::sync::broadcast::error::RecvError::Lagged(n)) => {
                    warn!(skipped = n, "Ticker processor lagged");
                }
                Err(tokio::sync::broadcast::error::RecvError::Closed) => break,
                _ => {}
            }
        }
    });

    // Orderbook event processor
    let ob_manager = orderbook_manager.clone();
    let event_bus_ob = event_bus.clone();
    tokio::spawn(async move {
        let mut rx = event_bus_ob.subscribe();
        loop {
            match rx.recv().await {
                Ok(Event::OrderbookUpdate {
                    instrument_name,
                    bids,
                    asks,
                }) => {
                    ob_manager.update(&instrument_name, &bids, &asks).await;
                }
                Err(tokio::sync::broadcast::error::RecvError::Lagged(n)) => {
                    warn!(skipped = n, "Orderbook processor lagged");
                }
                Err(tokio::sync::broadcast::error::RecvError::Closed) => break,
                _ => {}
            }
        }
    });

    // Analysis task — all 7 analyzers
    let analysis_registry = registry.clone();
    let analysis_ticker = ticker_cache.clone();
    let analysis_event_bus = event_bus.clone();
    let alert_threshold = config.alert_threshold;
    tokio::spawn(async move {
        // === Risk-free arbitrage (exact pricing violations) ===
        let pcp = PutCallParityAnalyzer::new(alert_threshold);
        let box_spread = BoxSpreadAnalyzer::new(10.0); // min $10 profit
        let conversion = ConversionAnalyzer::new(10.0);
        let vertical = VerticalArbAnalyzer::new(5.0);
        let calendar_arb = CalendarArbAnalyzer::new(5.0);

        // === Soft signals (IV-based) ===
        let vol_surface = VolSurfaceAnalyzer::new(15.0);
        let calendar_spread = CalendarSpreadAnalyzer::new(10.0);

        // Wait for data
        tokio::time::sleep(Duration::from_secs(30)).await;
        info!("Starting arbitrage scanning...");

        let mut scan_interval = interval(Duration::from_secs(10));
        loop {
            scan_interval.tick().await;

            let reg = &analysis_registry;
            let tc = &analysis_ticker;

            // Risk-free scans
            let mut arb_count = 0;
            for opp in pcp.scan_all(reg, tc).await {
                analysis_event_bus.publish(Event::OpportunityFound(opp));
                arb_count += 1;
            }
            for opp in box_spread.scan(reg, tc).await {
                analysis_event_bus.publish(Event::OpportunityFound(opp));
                arb_count += 1;
            }
            for opp in conversion.scan(reg, tc).await {
                analysis_event_bus.publish(Event::OpportunityFound(opp));
                arb_count += 1;
            }
            for opp in vertical.scan(reg, tc).await {
                analysis_event_bus.publish(Event::OpportunityFound(opp));
                arb_count += 1;
            }
            for opp in calendar_arb.scan(reg, tc).await {
                analysis_event_bus.publish(Event::OpportunityFound(opp));
                arb_count += 1;
            }

            // Soft signal scans
            let mut signal_count = 0;
            for opp in vol_surface.scan(reg, tc).await {
                analysis_event_bus.publish(Event::OpportunityFound(opp));
                signal_count += 1;
            }
            for opp in calendar_spread.scan(reg, tc).await {
                analysis_event_bus.publish(Event::OpportunityFound(opp));
                signal_count += 1;
            }

            if arb_count > 0 || signal_count > 0 {
                info!(
                    arbitrage = arb_count,
                    signals = signal_count,
                    "Scan complete"
                );
            }
        }
    });

    // Notifier
    let notifier = alert::notifier::Notifier::new(storage.clone());
    let notifier_bus = event_bus.clone();
    tokio::spawn(async move {
        notifier.run(&notifier_bus).await;
    });

    // Wait for WS connection
    tokio::time::sleep(Duration::from_secs(3)).await;

    // Load instruments + subscribe loop
    loop {
        match load_and_subscribe(&ws_client, &registry, &storage, &event_bus).await {
            Ok(()) => {
                info!("Instruments loaded and subscribed successfully");
            }
            Err(e) => {
                error!(error = %e, "Failed to load instruments, retrying in 10s...");
                tokio::time::sleep(Duration::from_secs(10)).await;
                continue;
            }
        }

        // Refresh every hour
        tokio::time::sleep(Duration::from_secs(3600)).await;
        info!("Refreshing instrument list...");
    }
}

async fn load_and_subscribe(
    client: &ws::client::WsClient,
    registry: &InstrumentRegistry,
    storage: &Storage,
    event_bus: &EventBus,
) -> Result<()> {
    info!("Loading BTC option instruments...");
    let instruments_result = client
        .send_request(
            "public/get_instruments",
            json!({
                "currency": "BTC",
                "kind": "option",
                "expired": false
            }),
        )
        .await?;

    let count = registry.load_from_response(&instruments_result).await?;

    let all_instruments = registry.get_all().await;
    for inst in &all_instruments {
        if let Err(e) = storage.save_instrument(inst).await {
            warn!(error = %e, instrument = %inst.instrument_name, "Failed to save instrument");
        }
    }

    event_bus.publish(Event::InstrumentsLoaded { count });

    let names = registry.get_all_names().await;
    info!(count = names.len(), "Subscribing to ticker channels...");
    Subscriber::subscribe_tickers(client, &names).await?;

    info!("All subscriptions active. Monitoring for opportunities...");
    Ok(())
}
