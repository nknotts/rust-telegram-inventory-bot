FROM rust:buster
ADD src  /work/src
ADD Cargo.* /work/
RUN cd /work; cargo build

FROM rust:buster
WORKDIR /inventory/
COPY --from=0 /work/target/debug/inventory-bot /inventory/inventory-bot
ENV LOG_LEVEL=info
ENV UPDATE_PERIOD_S=60
CMD ./inventory-bot -u $UPDATE_PERIOD_S -l $LOG_LEVEL $TELEGRAM_CHAT_ID 
