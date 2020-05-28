#export RUSTFLAGS="-C force-frame-pointers -C target-cpu=native --cfg stream_profiling"
#export RUSTFLAGS="-C target-cpu=native --cfg stream_profiling"
export RUSTFLAGS="-C target-cpu=native"
#RUST_BACKTRACE=1
#TEXT=~/dev/test_data/100M_rand_text.txt
#TEXT=./test_data/rand_text.txt
TEXT=./test_data/big_text.txt

RUNS=1
PERF="perf stat -r${RUNS} -e duration_time,task-clock,cycles,instructions,cache-misses,page-faults,context-switches,cpu-migrations"

function run {
	CPUS=$1
	CPU_RANGE="0-$(($CPUS - 1))"
	BINARY=$2
	taskset -c $CPU_RANGE $PERF $BINARY -t$1 $TEXT > /dev/null
}

THREADS=20

cd rustwp
cargo build --release
cd ..

cargo build --release -v

gcc -march=native -O3 wp.c -o wc-seq-c

#$PERF -v ./target/release/wc-parallel-partition-shuffle -t$THREADS $TEXT > /dev/null
#exit

run 20 ./target/release/wc-parallel-partition-buf
run 16 ./target/release/wc-parallel-partition-buf
run 8 ./target/release/wc-parallel-partition-buf
run 4 ./target/release/wc-parallel-partition-buf
run 2 ./target/release/wc-parallel-partition-buf

$PERF ./rustwp/target/release/rustwp 16 $TEXT > /dev/null
$PERF ./rustwp/target/release/rustwp 8 $TEXT > /dev/null
$PERF ./rustwp/target/release/rustwp 4 $TEXT > /dev/null

run 1 ./target/release/wc-async 
$PERF ./wc-seq-c $TEXT >  /dev/null


export PARDEG=$THREADS
echo "PARDEG=$PARDEG"
$PERF ~/pico/build/examples/word-count/pico_wc $TEXT  /dev/null

export PARDEG=16
echo "PARDEG=$PARDEG"
$PERF ~/pico/build/examples/word-count/pico_wc $TEXT  /dev/null

export PARDEG=8
echo "PARDEG=$PARDEG"
$PERF ~/pico/build/examples/word-count/pico_wc $TEXT  /dev/null

export PARDEG=4
echo "PARDEG=$PARDEG"
$PERF ~/pico/build/examples/word-count/pico_wc $TEXT  /dev/null

exit

#$PERF ./wc-seq-c $TEXT>  /dev/null
#$PERF ./rustwp/target/release/rustwp $THREADS $TEXT > /dev/null

#$PERF ./target/release/wc-seq $TEXT > /dev/null
#$PERF ./target/release/wc-seq-buf $TEXT > /dev/null
$PERF ./target/release/wc-async $TEXT /dev/null
#$PERF ./target/release/wc-async-buf $TEXT > /dev/null
#$PERF ./target/release/wc-parallel $TEXT > /dev/null
#$PERF ./target/release/wc-parallel-fine $TEXT > /dev/null
#$PERF ./target/release/wc-parallel-chunked $TEXT > /dev/null
#$PERF ./target/release/wc-parallel-buf $TEXT > /dev/null
#$PERF ./target/release/wc-parallel-partition -t$THREADS $TEXT > /dev/null
#$PERF ./target/release/wc-parallel-partition-chunked -t$THREADS $TEXT #> /dev/null
$PERF ./target/release/wc-parallel-partition-buf -t$THREADS $TEXT #> /dev/null
#$PERF ./target/release/wc-parallel-partition-buf -t8 $TEXT > /dev/null
$PERF ./target/release/wc-parallel-partition-shuffle-chunked -t$THREADS $TEXT > /dev/null
#$PERF ./target/release/wc-timely $TEXT > /dev/null

#$PERF ~/dev/pico/build/examples/word-count/seq_wc $TEXT  /dev/null

export PARDEG=$THREADS
$PERF ~/pico/build/examples/word-count/pico_wc $TEXT  /dev/null

