use futures::{Poll, Stream, Sink, Async, try_ready};
use std::collections::{BinaryHeap};
use tokio::sync::mpsc::{Sender, Receiver};
use tokio::sync::mpsc::error::RecvError;
use std::cmp::Ordering;

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

struct QueueItem<Event> {
    event: Option<Event>,
    order: u64,
    pipeline_index: usize,
}

impl<Event> Ord for  QueueItem<Event> {
    fn cmp(&self, other: &Self) -> Ordering {
        self.order.cmp(&other.order)
    }
}


impl<Event> PartialOrd for  QueueItem<Event> {
    fn partial_cmp(&self, other: &Self) -> Option<Ordering> {
        self.order.partial_cmp(&other.order)
    }
}

impl<Event> PartialEq for  QueueItem<Event> {
    fn eq(&self, other: &Self) -> bool {
        self.order == other.order
    }
}

impl<Event> Eq for  QueueItem<Event> { }

pub struct Join<Event, FOrd>
//where FOrd: Fn(&Event) -> u64
{
    calc_order: FOrd,
    pipelines:Vec<Receiver<Event>>,
    last_values:BinaryHeap<QueueItem<Event>>,
}

impl<Event, FOrd> Join<Event, FOrd>
    where FOrd: Fn(&Event) -> u64,
{
    pub fn new(calc_order: FOrd) -> Self
    {
        Join {
            calc_order,
            pipelines: Vec::new(),
            last_values: BinaryHeap::new(),
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

impl<Event, FOrd> Stream for Join<Event, FOrd>
    where //Ctx:Context<Event=Event, Result=R>,
        FOrd: Fn(&Event) -> u64,
{
    type Item = Event;
    type Error = RecvError;

    fn poll(&mut self) -> Poll<Option<Self::Item>, Self::Error> {
        match self.last_values.peek() {
            Some(q_item) => {
                let index = q_item.pipeline_index;
                let async_event = try_ready!(self.pipelines[index].poll());
                //TODO handle closed streams
                let result = match async_event {
                    Some(new_event)=> {
                        let old_event = self.last_values.pop().unwrap().event; //peek went ok already

                        let key = (self.calc_order)(&new_event);
                        self.last_values.push(QueueItem{
                            event: Some(new_event),
                            order: key,
                            pipeline_index: index
                        });

                        old_event
                    },
                    None => None
                };
                Ok(Async::Ready(result))
            },
            None => Ok(Async::Ready(None))
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