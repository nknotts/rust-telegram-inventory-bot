use clap::Parser;
use log;
use regex::Regex;
use serde::{Deserialize, Serialize};
use std::{fmt::Write, io::Write as IoWrite, time::Duration};
use teloxide::{adaptors::DefaultParseMode, prelude::*, types::ParseMode};
use tokio::{task, time};

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
    #[serde(with = "serde_regex")]
    NotRegex(Regex),
    Contains(String),
    DoesNotContain(String),
}

#[derive(Debug, Serialize, Deserialize)]
struct InventoryState {
    product: String,
    vendor: String,
    url: String,
    in_stock: bool,
    matches: Vec<Match>,
}

enum MatchUpdate {
    NoChange,
    Updated(Vec<InventoryState>),
}

#[derive(Debug)]
enum Error {
    IO(std::io::Error),
    SerdeYaml(serde_yaml::Error),
    Reqwest(reqwest::Error),
}

type Result<T> = std::result::Result<T, Error>;

impl From<std::io::Error> for Error {
    fn from(err: std::io::Error) -> Error {
        Error::IO(err)
    }
}

impl From<serde_yaml::Error> for Error {
    fn from(err: serde_yaml::Error) -> Error {
        Error::SerdeYaml(err)
    }
}

impl From<reqwest::Error> for Error {
    fn from(err: reqwest::Error) -> Error {
        Error::Reqwest(err)
    }
}

async fn update_state(fname: &str) -> Result<MatchUpdate> {
    log::debug!("Start Update State");

    let yaml_str = std::fs::read_to_string(fname.clone())?;
    let mut states: Vec<InventoryState> = serde_yaml::from_str(&yaml_str)?;

    let mut state_changed = false;
    for state in states.iter_mut() {
        let body = reqwest::Client::builder()
            .user_agent("curl/7.79.1")
            .build()?
            .get(state.url.clone())
            .send()
            .await?
            .text()
            .await?;

        let in_stock = state.matches.iter().all(|text_match| match text_match {
            Match::Regex(val) => val.is_match(&body),
            Match::NotRegex(val) => !val.is_match(&body),
            Match::Contains(val) => body.contains(val),
            Match::DoesNotContain(val) => !body.contains(val),
        });

        if state.in_stock != in_stock {
            state.in_stock = in_stock;
            state_changed = true;
        }
    }

    if state_changed {
        let yaml_str = serde_yaml::to_string(&states)?;
        std::fs::write(fname.to_string(), yaml_str)?;
        Ok(MatchUpdate::Updated(states))
    } else {
        Ok(MatchUpdate::NoChange)
    }
}

async fn send_inventory_state(
    header: &str,
    bot: &DefaultParseMode<AutoSend<Bot>>,
    chat_id: ChatId,
    states: Vec<InventoryState>,
) {
    let mut data: String;
    data = header.to_string();
    for state in states.iter() {
        write!(
            data,
            "\n{} - [{}]({}) - {}",
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
    data = data.replace("-", "\\-");

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

    let bot = Bot::from_env()
        .auto_send()
        .parse_mode(ParseMode::MarkdownV2);
    let chat_id = ChatId(cli_args.chat_id);

    {
        let yaml_str = std::fs::read_to_string(cli_args.match_file.clone()).unwrap();
        let inventory_state: Vec<InventoryState> = serde_yaml::from_str(&yaml_str).unwrap();
        send_inventory_state("Boot", &bot, chat_id.clone(), inventory_state).await;
    }

    let forever = task::spawn(async move {
        let mut interval = time::interval(Duration::from_secs_f64(cli_args.update_period_s));

        loop {
            interval.tick().await;
            match update_state(&cli_args.match_file).await {
                Ok(update) => match update {
                    MatchUpdate::NoChange => log::debug!("State did not change"),
                    MatchUpdate::Updated(inventory_state) => {
                        send_inventory_state(
                            "State Changed",
                            &bot,
                            chat_id.clone(),
                            inventory_state,
                        )
                        .await
                    }
                },
                Err(err) => log::error!("Failed to update state: {:?}", err),
            }
        }
    });

    forever.await.unwrap()
}
