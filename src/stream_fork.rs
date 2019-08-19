use futures::{Poll, Stream, Sink, Async, try_ready};
use std::collections::{BinaryHeap};
use tokio::sync::mpsc::{Sender, Receiver};
use tokio::sync::mpsc::error::RecvError;
use std::cmp::Ordering;
use crate::stream_join::Join;

pub struct Fork<Event, InStream, FSel>
{
    selector: FSel,
    pipelines:Vec<Sender<Event>>,
    input:InStream
}

impl<Event, InStream, FSel> Fork<Event, InStream, FSel>
where InStream:Stream<Item=Event>,
      FSel: Fn(&Event) -> usize,
{
    pub fn new(input:InStream, selector: FSel) -> Self
    {
        Fork {
            selector,
            pipelines: Vec::new(),
            input
        }
    }
}

pub struct ParallelPipeline<InEvent, InStream, OutEvent, FSel, FOrd, FBuildPipeline>
{
    fork: Fork<InEvent, InStream, FSel>,
    join: Join<OutEvent, FOrd>,
    build_pipeline: FBuildPipeline
}

impl<InEvent, InStream, OutEvent, FSel, FOrd, FBuildPipeline> ParallelPipeline<InEvent, InStream, OutEvent, FSel, FOrd, FBuildPipeline>
    where InStream:Stream<Item=InEvent>,
          FSel: Fn(&InEvent) -> usize,
          FOrd: Fn(&OutEvent) -> u64,
          FBuildPipeline: Fn(Receiver<InEvent>, Sender<OutEvent>)
{
    pub fn new(input: InStream, selector: FSel, cmp: FOrd, build_pipeline: FBuildPipeline) -> Self
    {
        let fork = Fork::new(input, selector);
        let join = Join::new(cmp);
        ParallelPipeline {
            fork,
            join,
            build_pipeline
        }
    }
}


/*
pub fn selective_context<Event, R, Key, Ctx, InStream, CtxInit, FSel, FWork> (input:InStream, ctx_builder: CtxInit, selector: FSel, work: FWork) -> SelectiveContext<Key, Ctx, InStream, CtxInit, FSel, FWork>
    where //Ctx:Context<Event=Event, Result=R>,
          InStream:Stream<Item=Event>,
          Key: Ord,
          CtxInit:Fn(&Key) -> Ctx,
          FSel: Fn(&Event) -> Key,
          FWork:Fn(&mut Ctx, &Event) -> R
{
    SelectiveContext {
        ctx_init: ctx_builder,
        selector,
        work,
        context_map: BTreeMap::new(),
        input
    }
}
*/

/*
impl<Event, Error, Ctx, InStream, FSel> Sink for Fork<Event, InStream, FSel>
    where //Ctx:Context<Event=Event, Result=R>,
          InStream:Stream<Item=Event, Error=Error>,
          FSel: Fn(&Event) -> usize
{
    type SinkItem = Event;
    type SinkError = Error;

    fn start_send(
        &mut self,
        item: Self::SinkItem
    ) -> StartSend<Self::SinkItem, Self::SinkError> {
        // Attempt to complete processing any outstanding requests.
        self.left.keep_flushing()?;
        self.right.keep_flushing()?;
        // Only if both downstream sinks are ready, start sending the next item.
        if self.left.is_ready() && self.right.is_ready() {
            self.left.state = self.left.sink.start_send(item.clone())?;
            self.right.state = self.right.sink.start_send(item)?;
            Ok(AsyncSink::Ready)
        } else {
            Ok(AsyncSink::NotReady(item))
        }
    }

    fn poll_complete(&mut self) -> Poll<(), Self::SinkError> {
        let left_async = self.left.poll_complete()?;
        let right_async = self.right.poll_complete()?;
        // Only if both downstream sinks are ready, signal readiness.
        if left_async.is_ready() && right_async.is_ready() {
            Ok(Async::Ready(()))
        } else {
            Ok(Async::NotReady)
        }
    }

    fn close(&mut self) -> Poll<(), Self::SinkError> {

        let left_async = self.left.close()?;
        let right_async = self.right.close()?;
        // Only if both downstream sinks are ready, signal readiness.
        if left_async.is_ready() && right_async.is_ready() {
            Ok(Async::Ready(()))
        } else {
            Ok(Async::NotReady)
        }
    }
    /*
    fn poll(&mut self) -> Poll<Option<Self::Item>, Self::Error> {
        let async_event = try_ready!(self.input.poll());
        let result = match async_event {
            Some(event)=> {
                let index = (self.selector)(&event) % self.pipelines.len();
                let pipeline = self.pipelines[index];
                pipeline.send(event)
            },
            None => None
        };

        Ok(Async::Ready(()))
    }
    */
}
*/