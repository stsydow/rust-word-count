//https://ptrace.fefe.de/wp/wpopt.rs

// gcc -o lines lines.c
// tar xzf llvm-8.0.0.src.tar.xz
// find llvm-8.0.0.src -type f | xargs cat | tr -sc 'a-zA-Z0-9_' '\n' | perl -ne 'print unless length($_) > 1000;' | ./lines > words.txt
//#![feature(impl_trait_in_bindings)]

use std::fmt::Write;
use std::io;
use std::iter::FromIterator;

use bytes::{BufMut, Bytes, BytesMut};
use futures::future;
use futures::stream;
//use futures::Stream;
use tokio::codec::{BytesCodec, FramedRead, FramedWrite};
use tokio::prelude::*;
use tokio::runtime::Runtime;

use std::cmp::max;

use std::time::Instant;
use word_count::util::*;
use parallel_stream::{StreamExt, StreamChunkedExt};

const CHUNKS_CAPACITY: usize = 256;
const BUFFER_SIZE: usize = 4;

fn main() {
    let conf = parse_args("word count parallel buf");
    let pipe_threads = max(1, conf.threads);

    let mut runtime = Runtime::new().expect("can't create runtime");
    let mut exec = runtime.executor();

    //use tokio_timer::clock::Clock;
    /*
    use tokio::runtime::Builder;
    let mut runtime = Builder::new()
        //.blocking_threads(pipe_threads/4+1)
        //.blocking_threads(2)
        //.clock(Clock::system())
        .core_threads(pipe_threads)
        .keep_alive(Some(Duration::from_secs(1)))
        //.name_prefix("my-custom-name-")
        //.stack_size(16 * 1024 * 1024)
        .build().expect("can't create runtime");
    */

    let (start_usr_time, start_sys_time) =  get_cputime_usecs();
    let start_time = Instant::now();

    let (input, output) = open_io_async(&conf);
    let input_stream = FramedRead::new(input, WholeWordsCodec::new());
    let output_stream = FramedWrite::new(output, BytesCodec::new());

    let task = input_stream.fork(pipe_threads, BUFFER_SIZE, &mut exec)
        .instrumented_fold(|| FreqTable::new(), |mut frequency, text| {
            count_bytes(&mut frequency, &text);

            future::ok::<FreqTable, _>(frequency)
        }, "split_and_count".to_owned())
        .map_result(|frequency| stream::iter_ok(frequency).chunks(CHUNKS_CAPACITY) )
        .flatten_stream()
        .shuffle_unordered_chunked( |(word, _count)| word.len() , pipe_threads, BUFFER_SIZE, &mut exec)
        .instrumented_fold(|| FreqTable::new(), |mut frequency, chunk| {

            for (word, count) in chunk {
                *frequency.entry(word).or_insert(0) += count;
            }

            future::ok::<FreqTable, _>(frequency)
        }, "merge_table".to_owned())
        .map_result(|sub_table| Vec::from_iter(sub_table))
        .merge(Vec::new(), |mut frequency, mut part| {
                frequency.append(&mut part);
                future::ok::<Vec<(Bytes, u64)>, _>(frequency)
            },
            &mut exec)
        .map_err(|e| io::Error::new(io::ErrorKind::Other, format!("recv error: {:#?}", e)))
        .map(|mut frequency| {
            let sort_time = Instant::now();
            frequency.sort_unstable_by(|(ref w_a, ref f_a), (ref w_b, ref f_b)| f_b.cmp(&f_a).then(w_b.cmp(&w_a)));
            eprintln!("sorttime:{:?}", sort_time.elapsed());
            stream::iter_ok(frequency).chunks(CHUNKS_CAPACITY)
        })
        .flatten_stream()
        .instrumented_map(
            |chunk: Vec<(Bytes, u64)>| {
            let mut buffer = BytesMut::with_capacity(CHUNKS_CAPACITY * 15);
            for (word_raw, count) in chunk {
                let word = utf8(&word_raw).expect("UTF8 encoding error");
                let max_len = word_raw.len() + 15;
                if buffer.remaining_mut() < max_len {
                    buffer.reserve(10 * max_len);
                }
                buffer
                    .write_fmt(format_args!("{} {}\n", word, count))
                    .expect("Formating error");
            }
            buffer.freeze()
        }, "format_chunk".to_owned())
        .forward(output_stream)
        .map_err(|e| {panic!("processing error: {:#?}", e)} );

    let (_word_stream, _out_file) = runtime.block_on(task).expect("error whil running task");

    let difference = start_time.elapsed();
    let (end_usr_time, end_sys_time) = get_cputime_usecs();
    let usr_time = (end_usr_time - start_usr_time) as f64 / 1000_000.0;
    let sys_time = (end_sys_time - start_sys_time) as f64 / 1000_000.0;
    eprintln!("walltime: {:?} (usr: {:.3}s sys: {:.3}s)",
        difference, usr_time, sys_time);

    runtime.shutdown_on_idle();
}
