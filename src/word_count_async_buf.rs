//https://ptrace.fefe.de/wp/wpopt.rs

// gcc -o lines lines.c
// tar xzf llvm-8.0.0.src.tar.xz
// find llvm-8.0.0.src -type f | xargs cat | tr -sc 'a-zA-Z0-9_' '\n' | perl -ne 'print unless length($_) > 1000;' | ./lines > words.txt

use std::io::Result as StdResult;
use std::iter::FromIterator;

use bytes::Bytes;
use tokio_util::codec::{BytesCodec, FramedRead, FramedWrite};
use tokio::runtime::Runtime;
use word_count::util::*;
use std::time::Instant;
use futures::StreamExt as FutStreamExt;
use futures::FutureExt;
use parallel_stream::{StreamExt};

fn main() -> StdResult<()> {
    let conf = parse_args("word count async buf");
    let mut runtime = Runtime::new()?;
    let (start_usr_time, start_sys_time) =  get_cputime_usecs();
    let start_time = Instant::now();

    let (input, output) = open_io_async(&conf);

    let input_stream = FramedRead::new(input, WholeWordsCodec::new());
    let output_stream = FramedWrite::new(output, BytesCodec::new());

    let dbg_future = input_stream
        .instrumented_fold(FreqTable::new(), |mut frequency, text| async move {
            count_bytes(&mut frequency, &text.expect("error reading input stream."));

            frequency
        },"split_and_count".to_owned())
        .map(|frequency| {
            let sort_time = Instant::now();
            let mut frequency_vec = Vec::from_iter(frequency);
            //frequency_vec.sort_unstable_by_key(|&(_, f)| f);
            frequency_vec.sort_unstable_by(|(ref w_a, ref f_a), (ref w_b, ref f_b)| f_b.cmp(&f_a).then(w_b.cmp(&w_a)));
            eprintln!("sorttime:{:?}", sort_time.elapsed());
            futures::stream::iter(frequency_vec)
        })
        .flatten_stream()
        .instrumented_map(|(word_raw, count)| {
            let word = ::std::str::from_utf8(&word_raw).expect("UTF8 encoding error");
            Bytes::from(format!("{} {}\n", word, count))
        },"format".to_owned())
        .map(|i| Ok(i))
        .forward(output_stream);

    runtime.block_on(dbg_future)?;
    let difference = start_time.elapsed();
    let (end_usr_time, end_sys_time) = get_cputime_usecs();
    let usr_time = (end_usr_time - start_usr_time) as f64 / 1000_000.0;
    let sys_time = (end_sys_time - start_sys_time) as f64 / 1000_000.0;
    eprintln!("walltime: {:?} (usr: {:.3}s sys: {:.3}s)",
        difference, usr_time, sys_time);

    Ok(())
}
