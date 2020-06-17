
use std::fmt::Write;
use std::io;
use std::iter::FromIterator;

use bytes::{BufMut, BytesMut};
use futures::future;
use futures::stream;
use futures::Stream;
use tokio::codec::{BytesCodec, FramedRead, FramedWrite};
use tokio::prelude::*;
use tokio::runtime::Runtime;

use word_count::util::*;

use std::time::Instant;
use parallel_stream::{StreamExt, StreamChunkedExt};

const CHUNKS_CAPACITY: usize = 256;

fn main() {
    let conf = parse_args("word count parallel buf");
    let mut runtime = Runtime::new().expect("can't create runtime");
    let mut exec = runtime.executor();

    let (start_usr_time, start_sys_time) =  get_cputime_usecs();
    let start_time = Instant::now();
    let (input, output) = open_io_async(&conf);

    let input_stream = FramedRead::new(input, WholeWordsCodec::new());
    let output_stream = FramedWrite::new(output, BytesCodec::new());


    //use tokio_timer::clock::Clock;
    /*
    use tokio::runtime::Builder;
    let mut runtime = Builder::new()
        //.blocking_threads(pipe_threads/4+1)
        //.blocking_threads(2)
        //.clock(Clock::system())
        .core_threads(pipe_threads)
        .keep_alive(Some(Duration::from_secs(1)))
        //.stack_size(16 * 1024 * 1024)
        .build().expect("can't create runtime");
    */
    let task = input_stream.fork(conf.threads, 4, &mut exec)

        .instrumented_fold(|| FreqTable::new(), |mut frequency, text| {
            count_bytes(&mut frequency, &text);

            future::ok(frequency)
        }, "split_and_count".to_owned())
        .merge(FreqTable::new(), |mut frequency, sub_table| {
            for (word, count) in sub_table {
                *frequency.entry(word).or_insert(0) += count;
            }
            future::ok(frequency)
        }, &mut exec)
        .map(|frequency| {
            let mut frequency_vec = Vec::from_iter(frequency.into_iter());
            frequency_vec.sort_unstable_by(|(ref w_a, ref f_a), (ref w_b, ref f_b)| f_b.cmp(&f_a).then(w_b.cmp(&w_a)));
            stream::iter_ok(frequency_vec).chunks(CHUNKS_CAPACITY) // <- TODO performance?
        })
        .flatten_stream()
        .decouple(2,&mut exec)
        .instrumented_map(|chunk| {
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
        }, "format".to_owned())
        .map_err(|e| io::Error::new(io::ErrorKind::Other, format!("recv error: {:#?}", e)))
        .forward(output_stream)
        .map_err(|e| {panic!("processing error: {:#?}", e)} );

    let (_word_stream, _out_file) = runtime.block_on(task).expect("error running main task");
    let difference = start_time.elapsed();
    let (end_usr_time, end_sys_time) = get_cputime_usecs();
    let usr_time = (end_usr_time - start_usr_time) as f64 / 1000_000.0;
    let sys_time = (end_sys_time - start_sys_time) as f64 / 1000_000.0;
    eprintln!("walltime: {:?} (usr: {:.3}s sys: {:.3}s)",
        difference, usr_time, sys_time);
    runtime.shutdown_on_idle();

}
