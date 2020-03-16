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

pub struct Probe<S>
    where S: Stream
{
    //TODO name
    t_min:u64,
    t_max:u64,
    hist: [u64;64],
    stream: S
}

impl<S, I> Probe<S>
    where S: Stream<Item=(SystemTime, I)>
{
    pub fn new(stream: S) -> Self {
        Probe {
            t_min: std::u64::MAX,
            t_max: 0,
            hist: [0;64],
            stream
        }
    }

    pub fn sample(&mut self, ref_time: &SystemTime) {
                let difference = ref_time.elapsed()
                    .expect("Clock may have gone backwards")
                    .as_nanos() as u64;
                let t_max_n = difference.max(self.t_max);
                self.t_max = t_max_n;
                let t_min_n = difference.min(self.t_min);
                self.t_min = t_min_n;
                self.hist[(64 - difference.leading_zeros()) as usize] += 1u64;
    }

    pub fn print_stats(&self) {
        println!("min: {}ns max: {}ns", self.t_min, self.t_max);
        // TODO rather use plotlib?
        for i in 0 .. 64 {
            let bin_time = 1<<i;
            if self.t_min > bin_time ||  self.t_max * 2  < bin_time {
                continue;
            }

            let count = self.hist[i];
            print!("{:?} \t" , Duration::from_nanos(bin_time));
            for _c in 0 .. (64 - count.leading_zeros()) {
                print!("#")
            }
            print!("\n");
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
                self.print_stats();
            },
            Ok(Async::Ready(Some((ref time, ref _item)))) => {
                self.sample(time);
            }
        };

        result
    }
}
