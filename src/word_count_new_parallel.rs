
use std::fmt::Write;
use std::iter::FromIterator;

use bytes::{BufMut, Bytes, BytesMut};
use futures::{stream};
use futures::StreamExt as FutStreamExt;
use futures::FutureExt;
use parallel_stream::StreamExt;
use tokio_util::codec::{BytesCodec, FramedRead, FramedWrite};
//use tokio::runtime::Runtime;

use std::time::Instant;
use word_count::util::*;

const CHUNKS_CAPACITY: usize = 256;
const CHANNEL_CAPACITY: usize = 64;

fn main() {
    let conf = parse_args("word count parallel buf");
    //let mut runtime = Runtime::new().expect("can't create runtime");

    //use tokio_timer::clock::Clock;
    use tokio::runtime::Builder;
    let mut runtime = Builder::new()
        .threaded_scheduler()
        //.blocking_threads(pipe_threads/4+1)
        //.blocking_threads(2)
        .core_threads(conf.threads + 2)
        //.stack_size(16 * 1024 * 1024)
        .build().expect("can't create runtime");

    let mut exec = runtime.handle();

    let (start_usr_time, start_sys_time) =  get_cputime_usecs();
    let start_time = Instant::now();

    let (input, output) = open_io_async(&conf);
    let input_stream = FramedRead::new(input, WholeWordsCodec::new());
    let output_stream = FramedWrite::new(output, BytesCodec::new());

    let task = input_stream.fork(conf.threads, CHANNEL_CAPACITY, &mut exec)
        .instrumented_fold(|| FreqTable::new(), |mut frequency, text| async {
            count_bytes(&mut frequency, &text.expect("io_error"));
            frequency
        }, "split_and_count".to_owned())
        .merge(FreqTable::new(), |mut frequency, sub_table| async {
            for (word, count) in sub_table {
                *frequency.entry(word).or_insert(0) += count;
            }
            println!("merge");
            frequency
        }, &mut exec)
        .map(|frequency_map| {
            println!("sort");
            let mut frequency = Vec::from_iter(frequency_map);
            frequency.sort_unstable_by(|(ref w_a, ref f_a), (ref w_b, ref f_b)| f_b.cmp(&f_a).then(w_b.cmp(&w_a)));
            stream::iter(frequency).chunks(CHUNKS_CAPACITY) // <- TODO performance?
        })
        .flatten_stream()
        //.decouple(CHANNEL_CAPACITY, &mut exec)
        .instrumented_map(|chunk: Vec<(Bytes, u64)>| {
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
        .map(|i| Ok(i))
        .forward(output_stream);

    runtime.block_on(task).expect("error running main task");
    let difference = start_time.elapsed();
    let (end_usr_time, end_sys_time) = get_cputime_usecs();
    let usr_time = (end_usr_time - start_usr_time) as f64 / 1000_000.0;
    let sys_time = (end_sys_time - start_sys_time) as f64 / 1000_000.0;
    eprintln!("walltime: {:?} (usr: {:.3}s sys: {:.3}s)",
        difference, usr_time, sys_time);
}
