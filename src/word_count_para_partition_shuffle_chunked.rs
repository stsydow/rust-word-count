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

use tokio::sync::mpsc::{channel, Receiver};

use std::cmp::max;
use word_count::stream_fork::ForkRR;
use word_count::stream_shuffle_buffered::ShuffleBuffered;
use tokio::sync::mpsc::error::SendError;

use std::time::Instant;
use word_count::util::*;
use word_count::{StreamExt};

const BUFFER_SIZE: usize = 4;

//use futures::sync::mpsc::channel;

const CHUNKS_CAPACITY: usize = 256;

fn reduce_task<InItem, OutItem, S, FBuildPipeline, OutFuture, E>(
    src: Receiver<InItem>,
    sink: S,
    builder: FBuildPipeline,
) -> impl Future<Item = (), Error = ()>
where
    E: std::error::Error,
    S: Sink<SinkItem=OutItem, SinkError=SendError>,
    OutFuture: Future<Item = OutItem, Error = E>,
    FBuildPipeline: FnOnce(Receiver<InItem>) -> OutFuture,
{
    future::lazy(move || {
        let task = builder(src)
            .and_then(|result| {
                sink.sink_map_err(|e| panic!("join send error: {}", e))
                    .send(result)
            })
            .map(|_sink| ())
            .map_err(|e| panic!("pipe_err:{:?}", e));
        task
    })
}

fn forward_task<InItem, OutItem, S, FBuildPipeline, OutStream, E>(
    src: Receiver<InItem>,
    sink: S,
    builder: FBuildPipeline,
) -> impl Future<Item = (), Error = ()>
    where
        E: std::error::Error,
        S: Sink<SinkItem=OutItem, SinkError=SendError>,
        OutStream: Stream<Item = OutItem, Error = E>,
        FBuildPipeline: FnOnce(Receiver<InItem>) -> OutStream,
{
    future::lazy(move || {
        let task = builder(src)
            .forward(sink.sink_map_err(|e| panic!("join send error: {}", e)))
            .map(|(_src, _sink)| ())
            .map_err(|e| panic!("pipe_err:{:?}", e));
        task
    })
}


//#[inline(never)]
fn count_fn(stream: Receiver<Bytes>) -> impl Future<Item=Vec<(Bytes, u64)>, Error = io::Error> {
    let item_stream = stream
        .map_err(|e| io::Error::new(io::ErrorKind::Other, format!("recv error: {:#?}", e)))
        .instrumented_fold(FreqTable::new(), |mut frequency, text| {
            count_bytes(&mut frequency, &text);

            future::ok::<FreqTable, io::Error>(frequency)
        }, "split_and_count".to_owned())
        .map(move |mut frequency|{
            Vec::from_iter(frequency.drain())
        });
    item_stream
}

//#[inline(never)]
fn acc_fn(stream: Receiver<Vec<(Bytes, u64)>>) -> impl Future<Item=Vec<(Bytes, u64)>, Error = io::Error> {
    let part = stream
        .map_err(|e| io::Error::new(io::ErrorKind::Other, format!("recv error: {:#?}", e)))
        .instrumented_fold(FreqTable::new(), |mut frequency, chunk| {

            for (word, count) in chunk {
                *frequency.entry(word).or_insert(0) += count;
            }

            future::ok::<FreqTable, io::Error>(frequency)
        }, "merge_table".to_owned()).map(move |mut sub_table| {
            let freq = Vec::from_iter(sub_table.drain());
            //freq.sort_unstable_by_key(|&(_, a)| a); //presort seems slower
            freq
        } );
    part
}

fn main() -> io::Result<()> {
    let conf = parse_args("word count parallel buf");
    let (input, output) = open_io_async(&conf);

    let input_stream = FramedRead::new(input, WholeWordsCodec::new());
    let output_stream = FramedWrite::new(output, BytesCodec::new());
    let pipe_threads = max(1, conf.threads);
    //let mut runtime = Runtime::new()?;
    

    use std::time::Duration;

    //use tokio_timer::clock::Clock;
    use tokio::runtime::Builder;
    let mut runtime = Builder::new()
        //.blocking_threads(pipe_threads/4+1)
        //.blocking_threads(2)
        //.clock(Clock::system())
        .core_threads(pipe_threads)
        .keep_alive(Some(Duration::from_secs(1)))
        //.name_prefix("my-custom-name-")
        //.stack_size(16 * 1024 * 1024)
        .build()?;


    let (fork, join) = {
        let mut senders = Vec::new();
        //let mut join = Join::new(|(_word, count)| { *count});

        let mut sort_inputs = Vec::with_capacity(pipe_threads);
        let (out_tx, out_rx) = channel::<Vec<(Bytes, u64)>>(pipe_threads);

        for _i in 0..pipe_threads {
            let (tx, rx) = channel::<Vec<(Bytes, u64)>>(pipe_threads);
            sort_inputs.push(tx);
            let sort_task = reduce_task(rx, out_tx.clone(), acc_fn);
            runtime.spawn(sort_task);
        }

        let mut shuffle = ShuffleBuffered::new( |(word, _count)| word.len() , sort_inputs);

        //let (out_tx, out_rx) = channel::<FreqTable>(pipe_threads);
        for _i in 0..pipe_threads {
            let (in_tx, in_rx) = channel::<Bytes>(BUFFER_SIZE);

            senders.push(in_tx);
            let sort_tx = shuffle.create_input();
            let pipe = reduce_task(in_rx, sort_tx, count_fn);
            runtime.spawn(pipe);
            //join.add(out_rx);
        }

        let fork = ForkRR::new(senders);

        (fork, out_rx)
    };

    let file_reader = input_stream
        .forward(fork.sink_map_err(|e| {
            io::Error::new(io::ErrorKind::Other, format!("fork send error: {}", e))
        }))
        .map(|(_in, _out)| ())
        .map_err(|e| {
            eprintln!("error: {}", e);
            panic!()
        });
    runtime.spawn(file_reader);

    let sub_table_stream /*: impl Stream<Item=HashMap<Vec<u8>, u64>, Error=io::Error> + Send*/ = join
        .map_err(|e| io::Error::new(io::ErrorKind::Other, format!("recv error: {:#?}", e)));

    let file_writer = sub_table_stream
        .fold(Vec::new(), |mut frequency, mut part| {
            frequency.append(&mut part);
            future::ok::<Vec<(Bytes, u64)>, io::Error>(frequency)
        })
        .map(|mut frequency| {
            let sort_time = Instant::now();
            //frequency.sort_by(|&(_, a), &(_, b)| b.cmp(&a));
            //frequency.sort_by_cached_key(|&(_, a)| a);
            //frequency.sort_by_key(|&(_, a)| a);
            //frequency.sort_unstable_by(|&(_, a), &(_, b)| b.cmp(&a));
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
        .forward(output_stream);

    let (_word_stream, _out_file) = runtime.block_on(file_writer)?;

    runtime.shutdown_on_idle();

    Ok(())
}
