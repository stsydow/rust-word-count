//https://ptrace.fefe.de/wp/wpopt.rs

use std::fmt::Write;
use std::iter::FromIterator;

use bytes::{BufMut, Bytes, BytesMut};
use futures::stream;
use futures::FutureExt;
use futures::TryFutureExt;
use futures::StreamExt as FutStreamExt;
use tokio_util::codec::{BytesCodec, FramedRead, FramedWrite};
//use tokio::runtime::Runtime;

use std::time::Instant;
use word_count::util::*;
use parallel_stream::{StreamExt};

const CHUNKS_CAPACITY: usize = 256;
const BUFFER_SIZE: usize = 32; //4;

fn main() {
    let conf = parse_args("word count parallel buf");
    //let mut runtime = Runtime::new().expect("can't create runtime");

    //use tokio_timer::clock::Clock;
    use tokio::runtime::Builder;
    let mut runtime = Builder::new()
        //.blocking_threads(pipe_threads/4+1)
        //.blocking_threads(2)
        //.clock(Clock::system())
        .core_threads(conf.threads +2)
        //.keep_alive(Some(Duration::from_secs(1)))
        //.stack_size(16 * 1024 * 1024)
        .build().expect("can't create runtime");
    let mut exec = runtime.handle();

    let (start_usr_time, start_sys_time) =  get_cputime_usecs();
    let start_time = Instant::now();

    let (input, output) = open_io_async(&conf);
    let input_stream = FramedRead::new(input, WholeWordsCodec::new());
    /*
    let input_stream = FramedRead {
                    inner: framed_read2_with_buffer(
                    Fuse(input, WholeWordsCodec::new()),
                    BytesMut::with_capacity(8129*8)
                    ),
                    };
    */
    let output_stream = FramedWrite::new(output, BytesCodec::new());

    let task = input_stream.fork(conf.threads, BUFFER_SIZE, &mut exec)
        .instrumented_fold(|| FreqTable::new(), |mut frequency, text| async move{
            count_bytes(&mut frequency, &text.expect("io error"));

            frequency
        }, "split_and_count".to_owned())
        .map_result(|frequency| stream::iter(frequency).chunks(CHUNKS_CAPACITY) )
        .flatten_stream()
        .shuffle_unordered_chunked( |(word, _count)| 
            ((word[0] as usize) << 8) + word.len(), 
            std::cmp::min(16, conf.threads), 
            BUFFER_SIZE, &mut exec)
        .instrumented_fold(|| FreqTable::new(), |mut frequency, sub_table| async move {

            for (word, count) in sub_table {
                *frequency.entry(word).or_insert(0) += count;
            }
            frequency
        }, "merge_table".to_owned())
        .map_result(|sub_table| Vec::from_iter(sub_table))
        .merge(Vec::new(), |mut frequency, mut part| async move {
                frequency.append(&mut part);
                frequency
            },
            &mut exec)
        //.map_err(|e| io::Error::new(io::ErrorKind::Other, format!("recv error: {:#?}", e)))
        .map(|mut frequency| {
            frequency.sort_unstable_by(|(ref w_a, ref f_a), (ref w_b, ref f_b)| f_b.cmp(&f_a).then(w_b.cmp(&w_a)));
            stream::iter(frequency).chunks(CHUNKS_CAPACITY) // <- TODO performance?
        })
        .flatten_stream()
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
        .forward(output_stream)
        .map_err(|e| {panic!("processing error: {:#?}", e)} );

    runtime.block_on(task).expect("error running main task");
    let difference = start_time.elapsed();
    let (end_usr_time, end_sys_time) = get_cputime_usecs();
    let usr_time = (end_usr_time - start_usr_time) as f64 / 1000_000.0;
    let sys_time = (end_sys_time - start_sys_time) as f64 / 1000_000.0;
    eprintln!("walltime: {:?} (usr: {:.3}s sys: {:.3}s)",
        difference, usr_time, sys_time);
}
