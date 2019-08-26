//https://ptrace.fefe.de/wp/wpopt.rs

// gcc -o lines lines.c
// tar xzf llvm-8.0.0.src.tar.xz
// find llvm-8.0.0.src -type f | xargs cat | tr -sc 'a-zA-Z0-9_' '\n' | perl -ne 'print unless length($_) > 1000;' | ./lines > words.txt

use std::io;
use std::fmt::Write;
use std::collections::HashMap;
use std::iter::FromIterator;

use tokio::prelude::*;
use tokio::runtime::Runtime;
use tokio::fs::{File, OpenOptions};
use tokio::io::{stdin, stdout};
use tokio::codec::{BytesCodec, FramedRead, FramedWrite};
use bytes::{BytesMut};
use futures::future::FutureResult;
use word_count::util::*;

use tokio::sync::mpsc::{channel, Receiver, Sender};

use word_count::stream_join::Join;
use word_count::stream_fork::Fork;
use std::sync::Arc;

const BUFFER_SIZE:usize = 4;

//use futures::sync::mpsc::channel;

const CHUNKS_CAPACITY:usize = 256;

fn pipeline_task<InItem, OutItem, FBuildPipeline, OutStream, E>(src: Receiver<InItem>, sink:Sender<OutItem>, builder: FBuildPipeline)
                                                                 -> impl Future<Item=(), Error=()>
    where E:std::error::Error,
          OutStream: Stream<Item=OutItem, Error=E>,
          FBuildPipeline: FnOnce(Receiver<InItem>) -> OutStream
{
    future::lazy(move || {
        let stream = builder(src);
        stream.forward(sink.sink_map_err(|e| { panic!("send_err:{}", e) }))
            .map(|(_stream, sink)| ())
            .map_err(|e| { panic!("pipe_err:{:?}", e) })
    })
}

fn main() -> io::Result<()> {

    let conf = parse_args("word count parallel chunked");
    let mut runtime = Runtime::new()?;

    let input: Box<dyn AsyncRead + Send> = match conf.input {
        None => Box::new(stdin()),
        Some(filename) => {
            let file_future =  File::open(filename);
            let byte_stream = runtime.block_on(file_future).expect("Can't open input file.");
            Box::new(byte_stream)
        }
    };

    let output: Box<dyn AsyncWrite + Send> = match conf.output {
        None => Box::new(stdout()),
        Some(filename) => {

            let file_future = OpenOptions::new().write(true).create(true).open(filename);
            let byte_stream = runtime.block_on(file_future).expect("Can't open output file.");
            Box::new(byte_stream)
        }
    };

    let input_stream = FramedRead::new(input, RawWordCodec::new());
    let output_stream = FramedWrite::new(output, BytesCodec::new());

    let (fork, join) = {

        let task_fn = |stream: Receiver<BytesMut>| {
            let frequency: HashMap<Vec<u8>, u64> = HashMap::new();
            let table_future = stream
                .map_err(|e| io::Error::new(io::ErrorKind::Other, format!("recv error: {:#?}", e)))
                .fold(frequency,
                      |mut frequency, word|
                          {
                              if !word.is_empty() {
                                  *frequency.entry(word.to_vec()).or_insert(0) += 1;
                              }

                              let result: FutureResult<_, io::Error> = future::ok(frequency);
                              result
                          }
                );

            table_future.map(|frequency|
                {
                    let mut frequency_vec = Vec::from_iter(frequency);
                    frequency_vec.sort_by(|&(_, a), &(_, b)| b.cmp(&a));
                    stream::iter_ok(frequency_vec)
                }).flatten_stream()//.chunks(CHUNKS_CAPACITY)
        };


        const pipe_count: usize = 4;
        let mut senders = Vec::new();
        let mut join = Join::new(|(_word, count)| { *count});

        for _i in 0 .. pipe_count {
            let (in_tx, in_rx) = channel::<BytesMut>(BUFFER_SIZE);
            let (out_tx, out_rx) = channel::<(Vec<u8>, u64)>(BUFFER_SIZE);
            senders.push(in_tx);
            let pipe = pipeline_task(in_rx, out_tx, task_fn);
            runtime.spawn(pipe);
            join.add(out_rx);
        }

        let fork = Fork::new(|word| word.len(), senders);

        (fork, join)
    };


    let file_reader = input_stream//.chunks(CHUNKS_CAPACITY)
        .forward(fork
            .sink_map_err(|e| io::Error::new(io::ErrorKind::Other, format!("send error: {}", e))))
        .map(|(_in, _out)| ())
        .map_err(|e| { eprintln!("error: {}", e); panic!()});
    runtime.spawn(file_reader);

    let file_writer = join
        .map_err(|e| io::Error::new(io::ErrorKind::Other, format!("recv error: {:#?}", e)))
        .map(|(word_raw, count) | {
            let mut buffer = BytesMut::with_capacity(CHUNKS_CAPACITY * 15);
            let word = utf8(&word_raw).expect("UTF8 encoding error");
            buffer.write_fmt(format_args!("{} {}\n", word, count)).expect("Formating error");
            buffer.freeze()
        })
        .forward(output_stream);

    let (_word_stream, _out_file) = runtime.block_on(file_writer)?;

    runtime.shutdown_on_idle();

    Ok(())
}