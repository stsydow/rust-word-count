use tokio::prelude::*;
use tokio;
use std::time::{ Instant };
use futures::try_ready;
use crate::LogHistogram;

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
