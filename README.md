# Rust Inventory Telegram Bot

Simple Rust inventory bot. I wrote it as an excuse to write some Rust code
and to hopefully snag a few Raspberry Pi Pico W. The bot will periodically
poll websites and determine stock status and report via a Telegram bot.

To run, the `TELOXIDE_TOKEN` environement variable must be set to your Telegram
bot API Token retrieved via [BotFather](https://core.telegram.org/bots#6-botfather).

You will also need your `chat_id`, you get it by
[following these directions](https://www.alphr.com/find-chat-id-telegram/).

If using the Docker image, set the `TELEGRAM_CHAT_ID` environment variable.
