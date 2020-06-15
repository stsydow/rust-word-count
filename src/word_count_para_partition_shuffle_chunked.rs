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

fn main() {
    let conf = parse_args("word count parallel buf");
    let (input, output) = open_io_async(&conf);

    let input_stream = FramedRead::new(input, WholeWordsCodec::new());
    let output_stream = FramedWrite::new(output, BytesCodec::new());
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

    let sub_table_streams = input_stream.fork(pipe_threads, &mut exec)
        .instrumented_fold(|| FreqTable::new(), |mut frequency, text| {
            count_bytes(&mut frequency, &text);

            future::ok::<FreqTable, _>(frequency)
        }, "split_and_count".to_owned())
    .map(|frequency|{
        Vec::from_iter(frequency)
    });

    let result_stream = sub_table_streams
        .instrumented_map_chunked(|e| e, "test".to_owned())
        .fork_sel_chunked( |(word, _count)| word.len() , pipe_threads, &mut exec)
        .instrumented_fold(|| FreqTable::new(), |mut frequency, chunk| {

            for (word, count) in chunk {
                *frequency.entry(word).or_insert(0) += count;
            }

            future::ok::<FreqTable, _>(frequency)
        }, "merge_table".to_owned())
        .map(|sub_table| Vec::from_iter(sub_table.into_iter()));

    let file_writer = result_stream
        .map_err(|e| io::Error::new(io::ErrorKind::Other, format!("recv error: {:#?}", e)))
        .fold(Vec::new(), |mut frequency, mut part| {
            frequency.append(&mut part);
            future::ok::<Vec<(Bytes, u64)>, io::Error>(frequency)
        })
        .map(|mut frequency| {
            let sort_time = Instant::now();
            //frequency.sort_by(|&(_, a), &(_, b)| b.cmp(&a));
            //frequency.sort_by_key(|&(_, a)| a);
            frequency.sort_unstable_by_key(|&(_, a)| a);
            //frequency.chunks(CHUNKS_CAPACITY) // <- TODO performance?
            eprintln!("sorttime:{:?}", sort_time.elapsed());
            stream::iter_ok(frequency).chunks(CHUNKS_CAPACITY)
        }).flatten_stream()
        .instrumented_map(
        //.map(
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

    let (_word_stream, _out_file) = runtime.block_on(file_writer).expect("error whil running task");
    runtime.shutdown_on_idle();

}
