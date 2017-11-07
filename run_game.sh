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

random_dimension()
{
  awk -v min=120 -v max=400 'BEGIN{srand(); print int(min+rand()*(max-min+1))}'
}

#./halite_osx -d "180 180" "RUST_BACKTRACE=1 target/release/MyBot" "./ipostv1"
#./halite_osx -d "180 180" "RUST_BACKTRACE=1 target/release/MyBot" "./VanillaSettler"

./halite_osx -d "150 150" "RUST_BACKTRACE=1 target/release/MyBot" "./bots/ipostv2"

#./halite_osx -d "$(random_dimension) $(random_dimension)" "RUST_BACKTRACE=1 target/release/MyBot" "./bots/cheesebot"

mv replays/*.hlt .
