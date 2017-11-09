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

#./halite_osx -d "180 180" "RUST_BACKTRACE=1 target/release/MyBot" "./ipostv1"
#./halite_osx -d "180 180" "RUST_BACKTRACE=1 target/release/MyBot" "./VanillaSettler"

#./halite_osx -d "345 230" -s 1137349230 "RUST_BACKTRACE=1 target/release/MyBot" "./bots/ipostv2"

./halite_osx -d "306 204" -s 361320654 "./bots/ipostv2" "target/release/MyBot"
#inconsistent nav P1 vs P2 ^^

#./halite_osx -d "$SIZE_X $SIZE_Y" "RUST_BACKTRACE=1 target/release/MyBot" "./bots/ipostv2"

#./halite_osx -d "$(random_dimension) $(random_dimension)" "RUST_BACKTRACE=1 target/release/MyBot" "./bots/cheesebot"

mv replays/*.hlt .
