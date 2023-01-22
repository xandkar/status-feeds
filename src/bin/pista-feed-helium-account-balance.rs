use anyhow::{anyhow, Result};
use clap::Parser;

#[derive(Debug, Parser)]
struct Cli {
    account: String,

    #[clap(default_value_t = 60)]
    interval: u64,
}

fn helium_fetch_balance_hnt(account: &str) -> Result<f64> {
    let url = format!("https://api.helium.io/v1/accounts/{}", account);
    let body = reqwest::blocking::get(url)?;
    let payload = body.text()?;
    let parsed: serde_json::Value = serde_json::from_str(&payload)?;
    match &parsed["data"]["balance"] {
        serde_json::Value::Number(balance) => {
            let balance = balance.as_f64().ok_or_else(|| {
                anyhow!("Failed to convert to f64: {balance:?}")
            })?;
            let balance = balance / 100_000_000.0;
            tracing::debug!("HNT account balance: {:?}", balance);
            Ok(balance)
        }
        balance => Err(anyhow!("unexpected balance format: {:?}", balance)),
    }
}

fn binance_fetch_average_price(symbol: &str) -> Result<f64> {
    let mrkt: binance::market::Market =
        binance::api::Binance::new(None, None);
    // XXX binance's error doesn't implement Sync and
    //     isn't compatible with anyhow as is and thus cannot just be propagated
    //     with '?'.
    mrkt.get_average_price(symbol)
        .map_err(|e| anyhow!("{:?}", e))
        .map(|p| p.price)
}

fn main_loop(account: &str, interval: u64) {
    // TODO Independent intervals of update, but recompute when either updated,
    //      with the other pulled from cache.
    // TODO Put them in threads and read channels? spawn async jobs with tokio?
    // TODO Maybe add max age/TTL?
    let mut price_hnt_in_usdt: Option<f64> = None;
    let mut balance_hnt: Option<f64> = None;

    loop {
        match helium_fetch_balance_hnt(account) {
            Err(e) => tracing::error!("helium data fetch failure: {:?}", e),
            Ok(balance) => {
                tracing::info!(
                    "helium data fetch success. balance_hnt:{}.",
                    balance
                );
                balance_hnt = Some(balance);
            }
        };
        match binance_fetch_average_price("HNTUSDT") {
            Err(e) => tracing::error!("binance data fetch failure: {:?}", e),
            Ok(price) => {
                tracing::info!(
                    "binance data fetch success. price_hnt_in_usdt:{}.",
                    price
                );
                price_hnt_in_usdt = Some(price);
            }
        };
        match (price_hnt_in_usdt, balance_hnt) {
            (None, None) => {
                tracing::debug!("neither data is yet available.");
                println!("H __.__ $__.__ $__.__");
            }
            (None, Some(balance_hnt)) => {
                println!("H {:.2} $__:__ $__:__", balance_hnt);
            }
            (Some(price_hnt_in_usdt), None) => {
                println!("H __:__ ${:.2} $__:__", price_hnt_in_usdt);
            }
            (Some(price_hnt_in_usdt), Some(balance_hnt)) => {
                let balance_usdt = price_hnt_in_usdt * balance_hnt;
                println!(
                    "H {:.2} ${:.2} ${:.2}",
                    balance_hnt, price_hnt_in_usdt, balance_usdt
                );
            }
        };
        std::thread::sleep(std::time::Duration::from_secs(interval));
    }
}

fn main() -> Result<()> {
    let subscriber = tracing_subscriber::FmtSubscriber::builder()
        .with_env_filter(
            tracing_subscriber::EnvFilter::builder()
                .with_default_directive(
                    tracing_subscriber::filter::LevelFilter::INFO.into(),
                )
                .from_env()?,
        )
        .with_writer(std::io::stderr)
        .with_file(true)
        .with_line_number(true)
        .with_timer(tracing_subscriber::fmt::time::LocalTime::rfc_3339())
        .finish();
    tracing::subscriber::set_global_default(subscriber)?;
    let args = Cli::parse();
    tracing::info!("starting with args: {:?}", &args);
    main_loop(&args.account, args.interval);
    Ok(())
}
