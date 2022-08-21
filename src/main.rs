use log;
use regex::Regex;
use serde::{Deserialize, Serialize};
use std::{fmt::Write, io::Write as IoWrite, time::Duration};
use tokio::{task, time};

use clap::Parser;

use teloxide::{adaptors::DefaultParseMode, prelude::*, types::ParseMode};

#[derive(Parser)]
#[clap(about = "Inventory Alerts", long_about = None, version, about)]
struct Cli {
    #[clap(short, long, value_parser, default_value_t = log::LevelFilter::Info)]
    log_level: log::LevelFilter,

    #[clap(short, long, default_value = "matches.yml")]
    match_file: String,

    #[clap(short, long, default_value_t = 60.0)]
    update_period_s: f64,

    #[clap(required = true)]
    chat_id: i64,
}

#[derive(Debug, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
enum Match {
    #[serde(with = "serde_regex")]
    Regex(Regex),
    Contains(String),
    DoesNotContain(String),
}

#[derive(Debug, Serialize, Deserialize)]
struct InventoryState {
    vendor: String,
    product: String,
    url: String,
    matches: Vec<Match>,
    in_stock: bool,
}

async fn update_state(states: &mut Vec<InventoryState>) -> Result<bool, reqwest::Error> {
    log::debug!("Start Update State");
    let mut state_changed = false;
    for state in states.iter_mut() {
        let body = reqwest::get(state.url.clone()).await?.text().await?;

        let in_stock = state.matches.iter().all(|text_match| match text_match {
            Match::Regex(val) => val.is_match(&body),
            Match::Contains(val) => body.contains(val),
            Match::DoesNotContain(val) => !body.contains(val),
        });

        if state.in_stock != in_stock {
            state.in_stock = in_stock;
            state_changed = true;
        }
    }
    log::debug!("Finished Update State");
    return Ok(state_changed);
}

fn update_yml(fname: String, states: &Vec<InventoryState>) {
    let yaml_str = serde_yaml::to_string(&states).unwrap();
    std::fs::write(fname, yaml_str).unwrap();
}

async fn send_inventory_state(
    header: &str,
    bot: &DefaultParseMode<AutoSend<Bot>>,
    chat_id: ChatId,
    states: &Vec<InventoryState>,
) {
    let mut data: String;
    data = header.to_string();
    for state in states.iter() {
        write!(
            data,
            "\n{} \\- [{}]({}) \\- {}",
            state.product,
            state.vendor,
            state.url,
            match state.in_stock {
                true => "In Stock",
                false => "Out of Stock",
            }
        )
        .unwrap();
    }

    log::info!("Sending: {}", data);
    match bot.send_message(chat_id, data).await {
        Ok(_) => log::debug!("Successfully sent inventory to bot"),
        Err(err) => log::error!("Failed to send inventory to bot: {:?}", err),
    }
}

#[tokio::main]
async fn main() {
    let cli_args = Cli::parse();

    env_logger::Builder::new()
        .filter_level(cli_args.log_level)
        .format(|buf, record| {
            let ts = buf.timestamp_millis();
            let level_style = buf.default_styled_level(record.level());
            writeln!(
                buf,
                "[{}: {} {}] {}",
                ts,
                level_style,
                record.target(),
                record.args()
            )
        })
        .init();

    let yaml_str = std::fs::read_to_string(cli_args.match_file.clone()).unwrap();
    let mut inventory_state: Vec<InventoryState> = serde_yaml::from_str(&yaml_str).unwrap();

    let bot = Bot::from_env()
        .auto_send()
        .parse_mode(ParseMode::MarkdownV2);
    let chat_id = ChatId(cli_args.chat_id);

    send_inventory_state("Boot", &bot, chat_id.clone(), &inventory_state).await;

    let forever = task::spawn(async move {
        let mut interval = time::interval(Duration::from_secs_f64(cli_args.update_period_s));

        loop {
            interval.tick().await;
            match update_state(&mut inventory_state).await {
                Ok(changed) => {
                    if changed {
                        update_yml(cli_args.match_file.clone(), &inventory_state);
                        send_inventory_state(
                            "State Changed",
                            &bot,
                            chat_id.clone(),
                            &inventory_state,
                        )
                        .await;
                    }
                }
                Err(err) => log::error!("Failed to update state: {:?}", err),
            }
        }
    });

    forever.await.unwrap()
}
