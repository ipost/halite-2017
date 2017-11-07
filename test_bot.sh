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


FILENAME=.bot_tests
[ -e $FILENAME ] && rm -f $FILENAME
touch $FILENAME
GAMES=20
GAMES=$((GAMES / 4))

START_TIME=$(date +%s)
for i in $(seq $GAMES $END);
do
  #run 4 games at a time
  for j in 1 2 3 4;
  do
    # largest board is 384 x 256, smallest is 240 x 160
    SIZE_Y=$(awk -v min=160 -v max=256 'BEGIN{srand(); print int(min+rand()*(max-min+1))}')
    SIZE_X=$((SIZE_Y * 3 / 2 ))
    ./halite_osx -d "$SIZE_X $SIZE_Y" "RUST_BACKTRACE=1 target/release/MyBot" "./bots/ipostv2" >> $FILENAME &
    #./halite_osx -d "$(random_dimension) $(random_dimension)" "RUST_BACKTRACE=1 target/release/MyBot" "./bots/ipostv2" "./bots/ipostv2" "./bots/ipostv2" >> $FILENAME
  done
  wait
done
END_TIME=$(date +%s)
echo "Test time: $((END_TIME - START_TIME))s"

mv replays/*.hlt .

echo "Player #0 won $(cat .bot_tests | grep "Player #0.\+came in rank #1" | wc -l) times out of $((GAMES * 4)) games"
