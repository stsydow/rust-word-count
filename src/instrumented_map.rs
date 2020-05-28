use tokio::prelude::*;
use tokio;
use std::time::{ Instant };
use futures::try_ready;
use crate::LogHistogram;


pub struct InstrumentedMap<S, F>
{
    name: String,
    hist: LogHistogram,
    stream: S,
    function: F
}

pub fn new<S, F, U>(stream: S, function: F, name: String) -> InstrumentedMap<S, F>
    where S: Stream,
          F: FnMut(S::Item) -> U,
{
    InstrumentedMap {
        name,
        hist: LogHistogram::new(),
        stream,
        function,
    }
}

impl<S, F, U> Stream for InstrumentedMap<S, F>
    where S: Stream,
          F: FnMut(S::Item) -> U,
{
    type Item = U;
    type Error = S::Error;

    #[cfg(stream_profiling)]
    fn poll(&mut self) -> Poll<Option<U>, S::Error> {
        let option = try_ready!(self.stream.poll());
        let result = match option {
            None => {
                self.hist.print_stats(&self.name);
                None
            },
            Some(item) => {
                let start = Instant::now();
                let result = (self.function)(item);
                self.hist.sample_now(&start);
                Some(result)
            }
        };

        Ok(Async::Ready(result))
    }

    #[cfg(not(stream_profiling))]
    fn poll(&mut self) -> Poll<Option<U>, S::Error> {
        let option = try_ready!(self.stream.poll());
        Ok(Async::Ready(option.map(|item| (self.function)(item) )))
    }
}
