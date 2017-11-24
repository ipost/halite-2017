#!/usr/bin/env bash
set -e

#cargo rustc --release -q -- -Awarnings
cargo rustc --release -q -- -Awarnings -g -A dead_code

[ -e log_0.txt ] && rm -f log_0.txt
if ls *.hlt 1> /dev/null 2>&1; then
  rm -f *.hlt
fi
if ls *-*.log 1> /dev/null 2>&1; then
  rm -f *-*.log
fi
if ls callgrind.out* 1> /dev/null 2>&1; then
  rm -f callgrind.out*
fi

SIZE_Y=$(awk -v min=160 -v max=256 'BEGIN{srand(); print int(min+rand()*(max-min+1))}')
SIZE_X=$((SIZE_Y * 3 / 2 ))

#./halite_osx -t -s 476480283 -d "150 100"  "valgrind --tool=callgrind --log-file=lmao target/debug/MyBot" "bots/ipostv5"
#./halite_osx -t -s 476480283 -d "210 140"  "valgrind --tool=callgrind --log-file=lmao target/debug/MyBot" "bots/ipostv5"
./halite_osx -t -s 476480285 -d "358 239"  "target/release/MyBot" "bots/ipostv6"

rg "PT" log_0.txt | sort -r | head -n 15 >> turn_timings
echo "" >> turn_timings

mv replays/*.hlt .

echo -n "Total turn time: "
ruby -e 'puts File.read("log_0.txt").scan(/(?<=time: PT).+/).map(&:to_f).reduce(:+)'
echo "Code timings:"
ruby -e 'puts File.read("log_0.txt").scan(/time at line.+/).map{|l| [l[/\d+/], l[/(?<=PT).\.\d+/].to_f]}.group_by(&:first).map{|k, v| "  " + k + ": " + v.map(&:last).reduce(:+).to_s }'

echo -ne "\0007"
