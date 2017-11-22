#!/usr/bin/env bash
set -e

cargo rustc --release -q -- -Awarnings
cargo rustc --release -q -- -Awarnings -A dead_code

[ -e log_0.txt ] && rm -f log_0.txt
if ls *.hlt 1> /dev/null 2>&1; then
  rm -f *.hlt
fi
if ls *-*.log 1> /dev/null 2>&1; then
  rm -f *-*.log
fi

SIZE_Y=$(awk -v min=160 -v max=256 'BEGIN{srand(); print int(min+rand()*(max-min+1))}')
SIZE_X=$((SIZE_Y * 3 / 2 ))

#./halite_osx -s 542739609 -d "280 187" "RUST_BACKTRACE=1 target/release/MyBot" "bots/ipostv3"
#./halite_osx -s 2191309506 -d "384 256" "RUST_BACKTRACE=1 target/release/MyBot" "bots/ipostv3"
#./halite_osx -s 2191309507 -d "384 256" "RUST_BACKTRACE=1 target/release/MyBot" "bots/ipostv3"
#./halite_osx -s 1803222514 -d "355 237" "RUST_BACKTRACE=1 target/release/MyBot" "bots/ipostv3"

#oscillating between planets
#./halite_osx -s 2902545647 -d "351 234" "RUST_BACKTRACE=1 target/release/MyBot" "bots/cheesebot"

./halite_osx -s 476480283 -d "358 239"  "RUST_BACKTRACE=1 target/release/MyBot" "bots/ipostv4"

mv replays/*.hlt .
