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

#settler crashes into itself here
#./halite_osx -d "80 80" -s 3288636875 "target/release/MyBot" "./VanillaSettler"
./halite_osx -d "180 180" -s 3288636877 "target/release/MyBot" "./VanillaSettler"

#./halite_osx -d "160 160" "target/release/MyBot" "./VanillaSettler"
