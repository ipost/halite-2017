#!/usr/bin/env bash

cargo rustc --release -q -- -Awarnings
./halite_osx -d "240 160" "target/release/MyBot" "target/release/MyBot"
