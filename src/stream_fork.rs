use futures::{Poll, Sink, Async, try_ready, StartSend};
use std::iter::FromIterator;

pub struct Fork<EventSink, FSel>
{
    selector: FSel,
    pipelines:Vec<EventSink>
}

impl<FSel, EventSink> Fork<EventSink, FSel>
where EventSink: Sink,
      FSel: Fn(&EventSink::SinkItem) -> usize
{
    pub fn new<I:IntoIterator<Item=EventSink>>(selector: FSel, sinks: I) -> Self
    {
        let pipelines = Vec::from_iter(sinks);
        assert!(!pipelines.is_empty());

        Fork {
            selector,
            pipelines
        }
    }

    /*
    pub fn add(&mut self, sink: EventSink) {
        self.pipelines.push(sink);
    }
    */
}

impl<FSel, S> Sink for Fork<S, FSel>
where
    S: Sink,
    FSel: Fn(&S::SinkItem) -> usize

{
    type SinkItem = S::SinkItem;
    type SinkError = S::SinkError;

    fn start_send(&mut self, item: Self::SinkItem) -> StartSend<Self::SinkItem, Self::SinkError> {
        let index = (self.selector)(&item) % self.pipelines.len();
        let sink = &mut self.pipelines[index];

        sink.start_send(item)
    }

    fn poll_complete(&mut self) -> Poll<(), Self::SinkError> {

        for sink in self.pipelines.iter_mut() {
            try_ready!(sink.poll_complete());
        }

        Ok(Async::Ready(()))
    }

    fn close(&mut self) -> Poll<(), Self::SinkError> {

        for i in 0 .. self.pipelines.len() {
            let sink = &mut self.pipelines[i];
            try_ready!(sink.close());
            self.pipelines.remove(i);
        }

        Ok(Async::Ready(()))
    }
}