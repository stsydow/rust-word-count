//https://ptrace.fefe.de/wp/wpopt.rs

// gcc -o lines lines.c
// tar xzf llvm-8.0.0.src.tar.xz
// find llvm-8.0.0.src -type f | xargs cat | tr -sc 'a-zA-Z0-9_' '\n' | perl -ne 'print unless length($_) > 1000;' | ./lines > words.txt

use std::io::{self, Read};
use std::collections::HashMap;
use std::iter::FromIterator;
use std::fs::File;

use word_count::util::*;

fn main() -> io::Result<()> {
    let conf = parse_args("word count simple");

    let mut buffer = String::new();
    if let Some(filename) = conf.input {
        File::open(filename)?.read_to_string(&mut buffer)?;
    } else {
        io::stdin().read_to_string(&mut buffer)?;
    }
    // Primitive Tokenize
    let mut frequency: HashMap<&str, u32> = HashMap::new();
    for word in buffer.split_ascii_whitespace(){
        *frequency.entry(&*word).or_insert(0) += 1;
    }

    // Sort by value
    let mut frequency = Vec::from_iter(frequency);
    frequency.sort_by(|&(_, a), &(_, b)| b.cmp(&a));

    let stdout = io::stdout();
    let mut stdout = stdout.lock();

    // Push to stdout
    for (word, count) in frequency{
        // Could also do println!("{} {}", word count) here,
        // but this is faster
        let out = &*format!("{} {}\n", word, count);
        io::copy(&mut out.as_bytes(), &mut stdout)?;
    }
    Ok(())
}