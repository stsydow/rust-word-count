use tokio::prelude::*;
use tokio;
//use tracing::{trace, error, Span};
use std::time::{ SystemTime, Duration};
use futures::try_ready;

pub struct Tag<S>
    where S: Stream
{
    stream: S
}

impl<S> Tag<S>
    where S: Stream
{
    pub fn new(stream: S) -> Self {
        Tag {
            stream
        }
    }
}

impl<S, I>  Stream for Tag<S>
    where S: Stream<Item=I>
{
    type Item = (SystemTime, S::Item);
    type Error = S::Error;

    fn poll(&mut self) -> Poll<Option<Self::Item>, Self::Error> {
        let maybe_item = try_ready!(self.stream.poll());
        Ok(Async::Ready( maybe_item.map(|item| (SystemTime::now(), item))))
    }
}

pub struct LogHistogram
{
    min:u64,
    max:u64,
    hist: [u64;64],
}

impl LogHistogram {
    pub fn new() -> Self {
        LogHistogram {
            min: std::u64::MAX,
            max: 0,
            hist: [0;64],
        }
    }

    pub fn sample(&mut self, ref_time: &SystemTime) {
                let difference = ref_time.elapsed()
                    .expect("Clock may have gone backwards")
                    .as_nanos() as u64;
                let t_max_n = difference.max(self.max);
                self.max = t_max_n;
                let t_min_n = difference.min(self.min);
                self.min = t_min_n;
                self.hist[(64 - difference.leading_zeros()) as usize] += 1u64;
    }

    pub fn print_stats(&self, name: &str) {
        println!("[{}] median: {}ns min: {}ns max: {}ns",name, self.median(), self.min, self.max);
        // TODO rather use plotlib?
        for i in 0 .. 64 {
            let bin_time = 1<<i;
            if self.min > bin_time ||  self.max * 2  < bin_time {
                continue;
            }

            let count = self.hist[i];
            print!("{:?} \t" , Duration::from_nanos(bin_time));
            for _c in 0 .. (64 - count.leading_zeros()) {
                print!("#")
            }
            print!("\t{}\n", count);
        }
    }

    // TODO more accuracy! http://www.cs.uni.edu/~campbell/stat/histrev2.html
    pub fn median(&self) -> u64 {
        let mut n:u64 = 0;
        for i in 0 .. self.hist.len() {
            let f = self.hist[i];
            n += f;
        }

        let mut samples:u64 = 0;
        for i in 0 .. self.hist.len() {
            samples += self.hist[i];
            if samples > n/2 {
                return 1u64<<i;
            }
        }
        unreachable!()
    }
}

pub struct Probe<S>
    where S: Stream
{
    name: String,
    hist: LogHistogram,
    stream: S
}

impl<S, I> Probe<S>
    where S: Stream<Item=(SystemTime, I)>
{
    pub fn new(stream: S, name: String) -> Self {
        Probe {
            name,
            hist: LogHistogram::new(),
            stream
        }
    }

}


impl<S, I>  Stream for Probe<S>
    where S: Stream<Item=(SystemTime, I)>
{
    type Item = S::Item;
    type Error = S::Error;

    fn poll(&mut self) -> Poll<Option<Self::Item>, Self::Error> {
        let result = self.stream.poll();

        match result {
            Err(ref _err) => {
                //error!("stream err!");
            }
            Ok(Async::NotReady) => {

            },
            Ok(Async::Ready(None)) => {
                self.hist.print_stats(&self.name);
            },
            Ok(Async::Ready(Some((ref time, ref _item)))) => {
                self.hist.sample(time);
            }
        };

        result
    }
}


//TODO ProbeAndTag?
