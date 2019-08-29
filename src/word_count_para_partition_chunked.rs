//https://ptrace.fefe.de/wp/wpopt.rs

// gcc -o lines lines.c
// tar xzf llvm-8.0.0.src.tar.xz
// find llvm-8.0.0.src -type f | xargs cat | tr -sc 'a-zA-Z0-9_' '\n' | perl -ne 'print unless length($_) > 1000;' | ./lines > words.txt
//#![feature(impl_trait_in_bindings)]

use std::io;
use std::fmt::Write;
use std::collections::HashMap;
use std::iter::FromIterator;

use futures::stream;
use futures::future;
use futures::{Stream};
use tokio::prelude::*;
use tokio::runtime::Runtime;
use tokio::fs::{File, OpenOptions};
use tokio::io::{stdin, stdout};
use tokio::codec::{BytesCodec, FramedRead, FramedWrite};
use bytes::{BytesMut, BufMut};

use word_count::util::*;

use tokio::sync::mpsc::{channel, Receiver, Sender};

use word_count::stream_fork::{ForkRR};
use std::cmp::{max};

const BUFFER_SIZE:usize = 4;

//use futures::sync::mpsc::channel;

const CHUNKS_CAPACITY:usize = 256;

fn reduce_task<InItem, OutItem, FBuildPipeline, OutFuture, E>(src: Receiver<InItem>, sink:Sender<OutItem>, builder: FBuildPipeline)
                                                                 -> impl Future<Item=(), Error=()>
    where E:std::error::Error,
          OutFuture: Future<Item=OutItem, Error=E>,
          FBuildPipeline: FnOnce(Receiver<InItem>) -> OutFuture
{
    future::lazy(move || {
        let task = builder(src)
            .and_then(|result| {
                sink
                    .sink_map_err(|e| panic!("join send error: {}", e))
                    .send(result)
            })
            .map(|_sink| ())
            .map_err(|e| { panic!("pipe_err:{:?}", e) });
        task
    })
}

#[inline(never)]
fn task_fn(stream: Receiver<Vec<BytesMut>>) -> impl Future<Item=HashMap<Vec<u8>, u64>, Error=io::Error>{
    let frequency: HashMap<Vec<u8>, u64> = HashMap::new();
    let table_future = stream
        .map_err(|e| io::Error::new(io::ErrorKind::Other, format!("recv error: {:#?}", e)))
        .fold(frequency,
        |mut frequency, chunk|
            {
                for word in chunk {
                    if !word.is_empty() {
                    *frequency.entry(word.to_vec()).or_insert(0) += 1;
                    }
                }
                future::ok::<HashMap<Vec<u8>, u64>, io::Error>(frequency)
            });
    table_future
}

#[inline(never)]
fn open_io_async(conf: &Config) -> (Box<dyn AsyncRead + Send>, Box<dyn AsyncWrite + Send>)
{
    let mut runtime = Runtime::new().expect("can't start async runtime");
    let input: Box<dyn AsyncRead + Send> = match &conf.input {
        None => Box::new(stdin()),
        Some(filename) => {
            let file_future =  File::open(filename.clone());
            let byte_stream = runtime.block_on(file_future).expect("Can't open input file.");
            Box::new(byte_stream)
        }
    };

    let output: Box<dyn AsyncWrite + Send> = match &conf.output {
        None => Box::new(stdout()),
        Some(filename) => {

            let file_future = OpenOptions::new().write(true).create(true).open(filename.clone());
            let byte_stream = runtime.block_on(file_future).expect("Can't open output file.");
            Box::new(byte_stream)
        }
    };

    (input, output)
}

fn main() -> io::Result<()> {

    let conf = parse_args("word count parallel chunked");
    let mut runtime = Runtime::new()?;

    let (input, output) = open_io_async(&conf);

    let input_stream = FramedRead::new(input, RawWordCodec::new());
    let output_stream = FramedWrite::new(output, BytesCodec::new());

    let (fork, join) = {

        let mut senders = Vec::new();
        //let mut join = Join::new(|(_word, count)| { *count});

        let pipe_theards = max(1,  conf.threads -1); // discount I/O Thread
        let (out_tx, out_rx) = channel::<HashMap<Vec<u8>, u64>>(pipe_theards);
        for _i in 0 .. pipe_theards {
            let (in_tx, in_rx) = channel::<Vec<BytesMut>>(BUFFER_SIZE);

            senders.push(in_tx);
            let pipe = reduce_task(in_rx, out_tx.clone(), task_fn);
            runtime.spawn(pipe);
            //join.add(out_rx);
        }

        let fork = ForkRR::new(senders);

        (fork, out_rx)
    };

    let file_reader = input_stream.chunks(CHUNKS_CAPACITY)
        .forward(fork
            .sink_map_err(|e| io::Error::new(io::ErrorKind::Other, format!("fork send error: {}", e))))
        .map(|(_in, _out)| ())
        .map_err(|e| { eprintln!("error: {}", e); panic!()});
    runtime.spawn(file_reader);

    let sub_table_stream /*: impl Stream<Item=HashMap<Vec<u8>, u64>, Error=io::Error> + Send*/ = join
        .map_err(|e| io::Error::new(io::ErrorKind::Other, format!("recv error: {:#?}", e)));

    let file_writer = sub_table_stream
        .fold(HashMap::<Vec<u8>, u64>::new(), |mut frequency, mut sub_table| {
                  for (word, count)  in sub_table.drain() {
                      *frequency.entry(word).or_insert(0) += count;
                  }
                future::ok::<HashMap<Vec<u8>, u64>, io::Error>(frequency)
              })
        .map(|mut frequency| {
                let mut frequency_vec = Vec::from_iter(frequency.drain());
                frequency_vec.sort_by(|&(_, a), &(_, b)| b.cmp(&a));
                stream::iter_ok(frequency_vec).chunks(CHUNKS_CAPACITY) // <- TODO performance?
            })
        .flatten_stream()
        .map(|chunk| {
            let mut buffer = BytesMut::with_capacity(CHUNKS_CAPACITY * 15);
            for (word_raw, count) in chunk{
                let word = utf8(&word_raw).expect("UTF8 encoding error");
                let max_len = word_raw.len() + 15;
                if buffer.remaining_mut() < max_len {
                    buffer.reserve(10*max_len);
                }
                buffer.write_fmt(format_args!("{} {}\n", word, count)).expect("Formating error");
            }
            buffer.freeze()
        })
        .forward(output_stream);

    let (_word_stream, _out_file) = runtime.block_on(file_writer)?;

    runtime.shutdown_on_idle();

    Ok(())
}
