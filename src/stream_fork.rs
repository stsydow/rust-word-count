use futures::{Poll, Sink, Async, try_ready, StartSend};
use std::iter::FromIterator;

pub struct Fork<S:Sink, FSel>
{
    selector: FSel,
    pipelines:Vec<S>
}

impl<S:Sink, FSel> Fork<S, FSel>
where FSel: Fn(&S::SinkItem) -> usize
{
    pub fn new<I:IntoIterator<Item=S>>(selector: FSel, sinks: I) -> Self
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

impl<S: Sink, FSel> Sink for Fork<S, FSel>
where
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