#!/bin/sh
set -x
sudo pgrep miniftp|xargs sudo kill -9;
cargo build --release;
mkdir -p bin;
cp ./target/release/miniftp bin/
sudo RUST_BACKTRACE=full ./bin/miniftp -c config.yaml;
sudo tail -f /var/log/miniftp.log;
