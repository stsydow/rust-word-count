//https://ptrace.fefe.de/wp/wpopt.rs

// gcc -o lines lines.c
// tar xzf llvm-8.0.0.src.tar.xz
// find llvm-8.0.0.src -type f | xargs cat | tr -sc 'a-zA-Z0-9_' '\n' | perl -ne 'print unless length($_) > 1000;' | ./lines > words.txt

use std::io;
use std::io::Result as StdResult;
use std::iter::FromIterator;

use bytes::Bytes;
use tokio::codec::{BytesCodec, FramedRead, FramedWrite};
use tokio::prelude::*;
use tokio::runtime::Runtime;
use word_count::util::*;

fn main() -> StdResult<()> {
    let conf = parse_args("word count async buf");
    let mut runtime = Runtime::new()?;

    let (input, output) = open_io_async(&conf);

    let input_stream = FramedRead::new(input, WholeWordsCodec::new());
    let output_stream = FramedWrite::new(output, BytesCodec::new());

    let dbg_future = input_stream
        .fold(FreqTable::new(), |mut frequency, text| {
            count_bytes(&mut frequency, &text);

            future::ok::<FreqTable, io::Error>(frequency)
        })
        .map(|frequency| {
            let mut frequency_vec = Vec::from_iter(frequency);
            frequency_vec.sort_by(|&(_, a), &(_, b)| b.cmp(&a));
            stream::iter_ok(frequency_vec)
        })
        .flatten_stream()
        .map(|(word_raw, count)| {
            let word = ::std::str::from_utf8(&word_raw).expect("UTF8 encoding error");
            Bytes::from(format!("{} {}\n", word, count))
        })
        .forward(output_stream);

    let (_, _output_stream) = runtime.block_on(dbg_future)?;
    runtime.shutdown_on_idle();

    Ok(())
}
