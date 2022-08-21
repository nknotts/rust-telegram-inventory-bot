FROM rust:buster
ADD src  /work/src
ADD Cargo.* /work
RUN cd /work; cargo build

FROM rust:buster
WORKDIR /inventory/
COPY --from=0 /work/target/debug/inventory-bot /inventory/inventory-bot
CMD exec ./inventory-bot -u 60 $TELEGRAM_CHAT_ID 
