#!/bin/bash
RUSTFLAGS="-C force-frame-pointers -C target-cpu=native"
TEXT=~/dev/test_data/100M_rand_text.txt

GProf2dot="./gprof2dot.py -f perf  --show-samples  --color-nodes-by-selftime"
PERF="perf record --call-graph lbr"
cargo build --release
#rustup run nightly perf record --call-graph lbr cargo wc-parallel
$PERF target/release/wc-seq $TEXT > /dev/null
perf script | c++filt | $GProf2dot  | dot -Tsvg -o seq-output.svg

$PERF target/release/wc-parallel-partition -t4 $TEXT > /dev/null
perf script | c++filt | $GProf2dot | dot -Tsvg -o para-output.svg

$PERF target/release/wc-parallel-partition-chunked -t4 $TEXT > /dev/null
perf script | c++filt | $GProf2dot | dot -Tsvg -o para-chunked-output.svg
#perf script | ./FlameGraph/stackcollapse-perf.pl | c++filt | ./FlameGraph/flamegraph.pl --width 4000 > flame.svg


