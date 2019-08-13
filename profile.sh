#!/bin/bash
RUSTFLAGS="-C force-frame-pointers -C target-cpu=native"
TEXT=~/dev/test_data/100M_rand_text.txt
cargo build --release
#rustup run nightly perf record --call-graph lbr cargo wc-parallel
perf record --call-graph lbr target/release/wc-seq $TEXT > /dev/null
perf script | c++filt | ./gprof2dot.py -f perf | dot -Tsvg -o seq-output.svg

perf record --call-graph lbr target/release/wc-parallel-chunked $TEXT > /dev/null
perf script | c++filt | ./gprof2dot.py -f perf | dot -Tsvg -o para-chunked-output.svg
#perf script | ./FlameGraph/stackcollapse-perf.pl | c++filt | ./FlameGraph/flamegraph.pl --width 4000 > flame.svg


