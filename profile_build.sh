RUSTFLAGS="-C force-frame-pointers -C target-cpu=native"
#TEXT=~/dev/test_data/100M_rand_text.txt
TEXT=~/dev/test_data/rand_text.txt

RUNS=2
PERF="perf stat -r${RUNS} -e task-clock  -e cycles:u -e instructions:u"

THREADS=4

cd rustwp
cargo build --release
cd ..

cargo build --release

gcc -march=native -O2 wp.c -o wc-seq-c

$PERF ./wc-seq-c $TEXT>  /dev/null
$PERF ./rustwp/target/release/rustwp $THREADS $TEXT > /dev/null

$PERF ./target/release/wc-seq $TEXT > /dev/null
$PERF ./target/release/wc-async $TEXT > /dev/null
$PERF ./target/release/wc-parallel $TEXT > /dev/null
$PERF ./target/release/wc-parallel-fine $TEXT > /dev/null
$PERF ./target/release/wc-parallel-chunked $TEXT > /dev/null
#$PERF ./target/release/wc-timely $TEXT > /dev/null

#$PERF ~/dev/pico/build/examples/word-count/seq_wc $TEXT  /dev/null

export PARDEG=4
$PERF ~/dev/pico/build/examples/word-count/pico_wc $TEXT  /dev/null

