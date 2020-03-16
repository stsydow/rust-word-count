//https://ptrace.fefe.de/wp/wpopt.rs

// gcc -o lines lines.c
// tar xzf llvm-8.0.0.src.tar.xz
// find llvm-8.0.0.src -type f | xargs cat | tr -sc 'a-zA-Z0-9_' '\n' | perl -ne 'print unless length($_) > 1000;' | ./lines > words.txt

use std::collections::HashMap;
use std::io::Error as StdError;
use std::io::Result as StdResult;
use std::iter::FromIterator;

use bytes::Bytes;
use futures::future::FutureResult;
use tokio::codec::{BytesCodec, FramedRead, FramedWrite};
use tokio::prelude::*;
use tokio::runtime::Runtime;
use word_count::util::*;
use std::time::SystemTime;
use word_count::probe_stream::*;

fn main() -> StdResult<()> {
    let conf = parse_args("word count async");
    let mut runtime = Runtime::new()?;

    let (start_usr_time, start_sys_time) =  get_cputime_usecs();
    let start_time = SystemTime::now();

    let (input, output) = open_io_async(&conf);

    let input_stream = FramedRead::new(input, BytesCodec::new());
    let output_stream = FramedWrite::new(output, BytesCodec::new());

    let frequency: HashMap<Vec<u8>, u32> = HashMap::new();

    let dbg_future = Probe::new(Tag::new(input_stream))
        /*
        .map(|fragment| {
            let start_time = SystemTime::now();
            (start_time, fragment)
        })*/
        .fold(
            (((std::f64::INFINITY, 0.0),frequency), Vec::<u8>::new()),
            |(((t_min, t_max), mut frequency), mut remainder), (start_time, buffer)| {
                word_count_buf_indexed(&mut frequency, &mut remainder, &buffer);



                let end_time = SystemTime::now();
                let difference = end_time.duration_since(start_time)
                .expect("Clock may have gone backwards");

                let d_t = difference.as_secs_f64();
                let t_max_n = d_t.max(t_max);
                let t_min_n = d_t.min(t_min);
                let result: FutureResult<_, StdError> = future::ok((((t_min_n, t_max_n), frequency), remainder));
                result
            },
        )
        .map(|(((t_min, t_max), mut frequency), remainder)| {
            if !remainder.is_empty() {
                *frequency.entry(remainder).or_insert(0) += 1;
            }
            println!("[count] min: {}s max: {}s", t_min, t_max);
            frequency
        })
        .map(|frequency| {
            let mut frequency_vec = Vec::from_iter(frequency);
            frequency_vec.sort_by(|&(_, a), &(_, b)| b.cmp(&a));
            stream::iter_ok(frequency_vec)
        })
        .flatten_stream()
        .map(|(word_raw, count)| {
            let word = ::std::str::from_utf8(&word_raw).expect("UTF8 encoding error");
            //Bytes::from(format!("{} {}\n", word, count))
            Bytes::from(format!(""))
        })
        .forward(output_stream);

    let (_, _output_stream) = runtime.block_on(dbg_future)?;

    let end_time = SystemTime::now();
    let (end_usr_time, end_sys_time) = get_cputime_usecs();
    let difference = end_time.duration_since(start_time)
        .expect("Clock may have gone backwards");
    let usr_time = (end_usr_time - start_usr_time) as f64 / 1000_000.0;
    let sys_time = (end_sys_time - start_sys_time) as f64 / 1000_000.0;
    println!("walltime: {:?} (usr: {:.3}s sys: {:.3}s)",
        difference, usr_time, sys_time);
    runtime.shutdown_on_idle();

    Ok(())
}
