#!/bin/sh
sudo pgrep miniftp|xargs sudo kill -9;
cargo build --release;
sudo ./target/release/miniftp -c config.yaml;
sudo tail -f /var/log/miniftp.log;
