#!/usr/bin/env bash
set -e

cargo rustc --release -q -- -Awarnings

#rm -f log_*

#settler crashes into itself here
#./halite_osx -d "80 80" -s 3288636875 "target/release/MyBot" "./VanillaSettler"
./halite_osx -d "180 180" -s 3288636877 "target/release/MyBot" "./VanillaSettler"

#./halite_osx -d "160 160" "target/release/MyBot" "./VanillaSettler"
