RUSTFLAGS="-C force-frame-pointers -C target-cpu=native"
RUST_BACKTRACE=1
#TEXT=~/dev/test_data/100M_rand_text.txt
TEXT=~/dev/test_data/rand_text.txt

RUNS=1
PERF="perf stat -r${RUNS} -e task-clock  -e cycles:u -e instructions:u -e cache-misses -e page-faults"


THREADS=4

cd rustwp
cargo build --release
cd ..

cargo build --release

gcc -march=native -O2 wp.c -o wc-seq-c

#$PERF -v ./target/release/wc-parallel-partition-shuffle -t$THREADS $TEXT > /dev/null
#exit

#$PERF ./wc-seq-c $TEXT>  /dev/null
#$PERF ./rustwp/target/release/rustwp $THREADS $TEXT > /dev/null

#$PERF ./target/release/wc-seq $TEXT > /dev/null
$PERF ./target/release/wc-seq-buf $TEXT > /dev/null
#$PERF ./target/release/wc-async $TEXT > /dev/null
#$PERF ./target/release/wc-async-buf $TEXT > /dev/null
#$PERF ./target/release/wc-parallel $TEXT > /dev/null
#$PERF ./target/release/wc-parallel-fine $TEXT > /dev/null
#$PERF ./target/release/wc-parallel-chunked $TEXT > /dev/null
#$PERF ./target/release/wc-parallel-buf $TEXT > /dev/null
#$PERF ./target/release/wc-parallel-partition -t$THREADS $TEXT > /dev/null
#$PERF ./target/release/wc-parallel-partition-chunked -t$THREADS $TEXT > /dev/null
$PERF ./target/release/wc-parallel-partition-buf -t$THREADS $TEXT > /dev/null
#$PERF ./target/release/wc-parallel-partition-buf -t8 $TEXT > /dev/null
$PERF ./target/release/wc-parallel-partition-shuffle -t$THREADS $TEXT > /dev/null
#$PERF ./target/release/wc-timely $TEXT > /dev/null

#$PERF ~/dev/pico/build/examples/word-count/seq_wc $TEXT  /dev/null

#export PARDEG=$THREADS
#$PERF ~/dev/pico/build/examples/word-count/pico_wc $TEXT  /dev/null

