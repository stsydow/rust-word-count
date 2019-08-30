//https://ptrace.fefe.de/wp/wpopt.rs

// gcc -o lines lines.c
// tar xzf llvm-8.0.0.src.tar.xz
// find llvm-8.0.0.src -type f | xargs cat | tr -sc 'a-zA-Z0-9_' '\n' | perl -ne 'print unless length($_) > 1000;' | ./lines > words.txt

use std::io::Result as StdResult;
use std::io;
use std::collections::HashMap;
use std::iter::FromIterator;

use tokio::prelude::*;
use tokio::runtime::Runtime;
use tokio::codec::{BytesCodec, FramedRead, FramedWrite};
use bytes::{Bytes, BytesMut};
use word_count::util::*;


#[inline(never)]
fn tokenize(frequency: &mut HashMap<Vec<u8>, u32>, text: &[u8])
{
    let mut i_start: usize = 0;
    for i in 0 .. text.len() {
        if text[i].is_ascii_whitespace() {
            let word = &text[i_start .. i];
            if !word.is_empty() {
                *frequency.entry(word.to_vec()).or_insert(0) += 1;
            }
            i_start = i + 1;
        }
    }
}

/*
//slower than copying to Vec<u8>
#[inline(never)]
fn tokenize_consume(frequency: &mut HashMap<BytesMut, u32>, mut text: BytesMut)
{
    loop {
        let mut white_space_idx = text.len();
        for i in 0 .. text.len() {
            if text[i].is_ascii_whitespace() {
                white_space_idx = i;
                break;
            }
        }

        if white_space_idx < text.len(){

            let mut word = text.split_to(white_space_idx + 1);
            let _space = word.split_off(word.len() -1);

            if !word.is_empty() {
                *frequency.entry(word).or_insert(0) += 1;
            }
        } else {
            break;
        }
    }
}
*/

fn main() -> StdResult<()> {

    let conf = parse_args("word count async buf");
    let mut runtime = Runtime::new()?;

    let (input, output) = open_io_async(&conf);

    let input_stream = FramedRead::new(input, WholeWordsCodec::new());
    let output_stream = FramedWrite::new(output, BytesCodec::new());

    let frequency: HashMap<Vec<u8>, u32> = HashMap::new();

    let dbg_future = input_stream
    .fold(frequency,
          |mut frequency, text|
          {
              tokenize(&mut frequency, &text);

              future::ok::<HashMap<Vec<u8>, u32>, io::Error>(frequency)
          }
    ).map(|frequency| {
        let mut frequency_vec = Vec::from_iter(frequency);
        frequency_vec.sort_by(|&(_, a), &(_, b)| b.cmp(&a));
        stream::iter_ok(frequency_vec)
    }).flatten_stream()
    .map(|(word_raw, count)| {
            let word = ::std::str::from_utf8(&word_raw).expect("UTF8 encoding error");
            Bytes::from(format!("{} {}\n", word, count))
    }).forward(output_stream);

    let (_, _output_stream) = runtime.block_on(dbg_future)?;
    runtime.shutdown_on_idle();

    Ok(())
}
