use argparse::{ArgumentParser, Print, StoreOption};
use std::{io, str, usize};
use std::collections::HashMap;

use tokio::codec::{Decoder};
use bytes::{BytesMut};

pub struct RawWordCodec {
    _last_cursor: usize
}

impl RawWordCodec {
    pub fn new() -> Self {
        RawWordCodec{_last_cursor:0}
    }
}

impl Decoder for RawWordCodec {
    type Item = BytesMut;
    // TODO: in the next breaking change, this should be changed to a custom
    // error type that indicates the "max length exceeded" condition better.
    type Error = io::Error;

    fn decode(&mut self, buf: &mut BytesMut) -> Result<Option<BytesMut>, io::Error> {

        if buf.len() >= 1_000_000 {
            return Err(io::Error::new(io::ErrorKind::Other, format!("max word length exceeded {:#?}B", buf.len())));
        }

        let word_offset = buf//[self.last_cursor..] // TODO use last cursor
            .iter()
            .position(|b| b.is_ascii_whitespace());

        let some_word = if let Some(offset) = word_offset {
            // Found a line!
            let whitespace_index = offset;
            let mut word = buf.split_to(whitespace_index + 1);
            /*
            let word = &word[..word.len() - 1];
            let word = utf8(word)?;

            Some(word.to_string())
            */
            let _space = word.split_off(1);

            Some(word)
        } else {
            None
        };

        return Ok(some_word);
    }

    fn decode_eof(&mut self, buf: &mut BytesMut) -> Result<Option<BytesMut>, io::Error> {
        Ok(match self.decode(buf)? {
            Some(frame) => Some(frame),
            None => {
                // No terminating newline - return remaining data, if any
                if buf.is_empty() {
                    None
                } else {
                    let word = buf.take();
                    Some(word)
                }
            }
        })
    }
}

pub struct Config {
    pub output: Option<String>,
    pub input: Option<String>,
}

pub fn parse_args(description: &str)  -> Config {
    let mut conf : Config = Config {
        output: None,
        input: None,
    };

    {  // this block limits scope of borrows by ap.refer() method
        let mut ap = ArgumentParser::new();

        ap.set_description(description);
        ap.add_option(&["-V", "--version"],
                      Print(env!("CARGO_PKG_VERSION").to_string()), "Show version");


        ap.refer(&mut conf.input).add_argument(
            "input", StoreOption, "input file - default: stdin");

        ap.refer(&mut conf.output).add_argument(
            "output", StoreOption, "output file - default: stdout");

        ap.parse_args_or_exit();
    }

    return conf;
}

pub fn utf8(buf: &[u8]) -> Result<&str, io::Error> {
    str::from_utf8(buf).map_err(|_|
        io::Error::new(
            io::ErrorKind::InvalidData,
            "Unable to decode input as UTF8"))
}


pub fn count_word<T>(frequency: &mut HashMap<Vec<T>, u32>, word:&[T])
    where T : std::hash::Hash + std::cmp::Eq + std::clone::Clone
{
    if word.is_empty() {
        return;
    }

    *frequency.entry(word.to_vec()).or_insert(0) += 1;
    /*
    let value = frequency.get_mut(word);
    if let Some(count) = value {
        *count += 1;
    } else {
        frequency.insert(Vec::from(word), 1);
    }
    */
}

pub fn word_count_buf_split(frequency: &mut HashMap<Vec<u8>, u32>, remainder: &mut Vec<u8>, buffer: &[u8])
{
    let mut chunks:std::slice::Split<_,_> = buffer.split(|c| c.is_ascii_whitespace());
    let first_tail = chunks.next().unwrap();
    remainder.extend_from_slice( first_tail);
    let first_word =  remainder.clone();
    let mut last_word = first_word.as_slice();
    for word in chunks {
        count_word(frequency, last_word);
        last_word = word;
    }

    remainder.clear();
    remainder.extend_from_slice(last_word);
}

pub fn word_count_buf_by_foot(frequency: &mut HashMap<Vec<u8>, u32>, remainder: &mut Vec<u8>, buffer: &[u8])
{
    let current_word = remainder;
    current_word.reserve(20);

    for c in buffer {
        if c.is_ascii_whitespace() {
            count_word(frequency, current_word.as_ref());
            current_word.clear();
        } else {
            current_word.push(*c);
        }
    }
}


pub fn word_count_buf_indexed(frequency: &mut HashMap<Vec<u8>, u32>, remainder: &mut Vec<u8>, buffer: &[u8])
{
    let mut i_start: usize = 0;

    for i in 0 .. buffer.len() {
        if buffer[i].is_ascii_whitespace() {
            remainder.extend_from_slice(&buffer[i_start .. i]);
            count_word(frequency, remainder.as_ref());
            i_start = i + 1;
            remainder.clear();
            break;
        }
    }

    for i in i_start .. buffer.len() {
        if buffer[i].is_ascii_whitespace() {
            count_word(frequency, &buffer[i_start .. i]);
            i_start = i + 1;
        }
    }
    remainder.extend_from_slice(&buffer[i_start ..]);
}
