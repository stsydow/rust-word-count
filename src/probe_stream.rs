use tokio::prelude::*;
use tokio;
//use tracing::{trace, error, Span};
use std::time::{ Instant, Duration};
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
    type Item = (Instant, S::Item);
    type Error = S::Error;

    fn poll(&mut self) -> Poll<Option<Self::Item>, Self::Error> {
        let maybe_item = try_ready!(self.stream.poll());
        Ok(Async::Ready( maybe_item.map(|item| (Instant::now(), item))))
    }
}

pub struct LogHistogram
{
    min:u64,
    max:u64,
    sum:u64,
    hist: [u64;64],
}

const BARS: &'static [char;9] = &['_','▁','▂','▃','▄','▅','▆','▇','█'];
const BARS_MAX:usize = 8;

impl LogHistogram {
    pub fn new() -> Self {
        LogHistogram {
            min: std::u64::MAX,
            max: 0,
            sum: 0,
            hist: [0;64], // use 128 bins maybe: 10^(log(1<<64 -1 ) / 128) = 1.41421356 or estimate with first two non zero bits
        }
    }

    pub fn sample(&mut self, ref_time: &Instant) {
                let difference = ref_time.elapsed()
                    .as_nanos() as u64;
                // TODO use TSC for lower overhead:
                // https://crates.io/crates/tsc-timer
                // http://gz.github.io/rust-perfcnt/x86/time/fn.rdtsc.html
                self.sum += difference;
                let t_max_n = difference.max(self.max);
                self.max = t_max_n;
                let t_min_n = difference.min(self.min);
                self.min = t_min_n;
                self.hist[(64 - difference.leading_zeros()) as usize] += 1u64;
    }

    fn print_sparkline(& self){

        let f_max = self.hist.iter().max().unwrap();
        let log_f_max = 64 - f_max.leading_zeros();

        let line: String = self.hist.iter().map(|f| {
            let log_f = 64 - f.leading_zeros();

            let i = if log_f_max > BARS_MAX as u32 {
                log_f.saturating_sub(log_f_max - BARS_MAX as u32)
            } else {
                log_f
            } as usize;
            BARS[i]
        }).collect();
        println!("{:?}", line);
    }

    pub fn print_stats(&self, name: &str) {
        let mut spark_line:Vec<char> = Vec::with_capacity(64);
        {
            let f_max = self.hist.iter().max().unwrap();
            let log_f_max = 64 - f_max.leading_zeros() as i32;
            for i in 0 .. 64 {
                let bin_time = 1<<i;
                if self.min > bin_time ||  self.max * 2  < bin_time {
                    continue;
                }

                let f = self.hist[i];
                let log_f = 64 - f.leading_zeros() as i32;
                let b = if log_f_max > BARS_MAX as i32 {
                    log_f - (log_f_max - BARS_MAX as i32)
                } else {
                    log_f
                };
                if b < 0 {
                    if f > 0 {
                        spark_line.push('.');
                    } else {
                        spark_line.push(' ');
                    }
                } else {
                    spark_line.push(BARS[b as usize].clone());
                }
            }
        }
        println!("[{}] ops: {} acc_time:{:0.3}ms\n 5%:{:0.3}ms med:_{:0.3}ms_ 95%:{:0.3}ms\n min: {:0.3}ms |{}| max: {:0.3}ms",
        name, self.size(), self.sum as f32/1000_000.0,
        self.percentile(0.05)/1000_000.0, self.percentile(0.5)/1000_000.0, self.percentile(0.95)/1000_000.0,
        self.min as f32/1000_000.0, spark_line.iter().collect::<String>() ,self.max as f32/1000_000.0
        );

        // TODO rather use plotlib?
        /*
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
        */
    }

    fn size(&self) -> u64 {
        let mut n:u64 = 0;
        for i in 0 .. self.hist.len() {
            let f = self.hist[i];
            n += f;
        }
        n
    }

    // TODO log transformations for narrow distributions is inaccurate
    pub fn percentile(&self, p: f32) -> f32 {
        assert!(p >= 0.0 && p <= 1.0);

        let p_count = (self.size() as f32) * p;
        let mut samples:u64 = 0;
        for i in 0 .. self.hist.len() {
            let c_bin = self.hist[i];
            let samples_incl = samples + c_bin;
            if samples_incl > p_count as u64 {
                let d_bin = (p_count - samples as f32) / c_bin as f32;
                let log_val = (i -1) as f32 + d_bin;
                return log_val.exp2();
            }
            samples = samples_incl;
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
    where S: Stream<Item=(Instant, I)>
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
    where S: Stream<Item=(Instant, I)>
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
