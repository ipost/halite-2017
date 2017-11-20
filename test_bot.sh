#!/usr/bin/env bash
set -e

cargo rustc --release -q -- -Awarnings
cargo rustc --release -q -- -Awarnings -A dead_code

if ls log_*.txt 1> /dev/null 2>&1; then
  rm -f log_*.txt
fi
if ls *.hlt 1> /dev/null 2>&1; then
  rm -f *.hlt
fi
if ls *-*.log 1> /dev/null 2>&1; then
  rm -f *-*.log
fi
if ls replays/*.hlt 1> /dev/null 2>&1; then
  rm -f replays/*.hlt
fi

# print config constants
cat src/hlt/constants.rs | grep -A500 'CONFIGURATIONS' | tail -n+2

FILENAME=.bot_tests
[ -e $FILENAME ] && rm -f $FILENAME
touch $FILENAME
BOT_1="target/release/MyBot"
BOT_2="bots/ipostv3"
GAMES=20
PARALLEL=1
GAMES=$((GAMES / PARALLEL))

#run $PARALLEL games at a time
START_TIME=$(date +%s)
for j in $(seq 1 $PARALLEL);
do
  {
    for i in $(seq 1 $GAMES);
    do
      # largest board is 384 x 256, smallest is 240 x 160
      SIZE_Y=$(awk -v min=160 -v max=256 'BEGIN{srand(); print int(min+rand()*(max-min+1))}')
      SIZE_X=$((SIZE_Y * 3 / 2 ))
      ./halite_osx -d "$SIZE_X $SIZE_Y" "RUST_BACKTRACE=1 $BOT_1" "$BOT_2" "$BOT_2" "$BOT_2" >> $FILENAME
      #./halite_osx -d "$SIZE_X $SIZE_Y" "RUST_BACKTRACE=1 $BOT_1" "$BOT_2" >> $FILENAME
      #./halite_osx -d "$SIZE_X $SIZE_Y" "target/release/MyBot" "bots/cheesebotv2" >> $FILENAME
      #./halite_osx -d "$SIZE_X $SIZE_Y" "RUST_BACKTRACE=1 target/release/MyBot" "./bots/ipostv3" "./bots/ipostv3" "./bots/ipostv3" >> $FILENAME
    done
  } &
done
wait
END_TIME=$(date +%s)
echo "Test time: $((END_TIME - START_TIME))s"

#mv replays/*.hlt .
#rm -f *.hlt

echo "Player #0 won $(cat .bot_tests | grep "Player #0.\+came in rank #1" | wc -l) times out of $((GAMES * PARALLEL)) games"
echo "Player #1 won $(cat .bot_tests | grep "Player #1.\+came in rank #1" | wc -l) times out of $((GAMES * PARALLEL)) games"
echo "$(ls *-*.log 2> /dev/null | wc -l) Failures found"

#ps | grep ipostv3 | awk -F" " '{if ($1) print $1}' | xargs kill
