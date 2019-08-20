use futures::{Poll, Sink, stream::Stream, Async, try_ready, Future, StartSend};
use tokio::sync::mpsc::{channel, Sender, Receiver};
use crate::stream_join::Join;
use crate::stream_fork::Fork;

const BUFFER_SIZE:usize = 4;

pub struct ParallelPipeline<PipelineStart, PipelineEnd, FSel, FOrd, Task, FBuildPipeline>
    where PipelineStart: Sink, PipelineEnd: Stream
{
    fork: Fork<PipelineStart, FSel>,
    join: Join<PipelineEnd, FOrd>,
    tasks: Vec<Task>,
    build_pipeline: FBuildPipeline
}


impl<Item, OutItem, OutStream, FSel, FOrd, FBuildPipeline> ParallelPipeline<Sender<Item>, Receiver<OutItem>,FSel, FOrd, Box<dyn Future<Item=(), Error=()>>, FBuildPipeline>
    where OutStream: Stream<Item=OutItem, Error=()> /*+'static*/, OutItem: /*'static*/,
          FSel: Fn(&Item) -> usize,
          FOrd: Fn(&OutItem) -> u64,
          //Task: Future<Item=(), Error=()>, // + Sized,
          FBuildPipeline: Fn(Receiver<Item>) -> OutStream
{
    pub fn new(selector: FSel, cmp: FOrd, build_pipeline: FBuildPipeline, pipe_count: usize) -> Self
    {
        let mut senders = Vec::new();
        let mut tasks = Vec::new();
        let mut join = Join::new(cmp);

        for i in 0 .. pipe_count {
            let (in_tx, in_rx) = channel::<Item>(BUFFER_SIZE);
            let (out_tx, out_rx) = channel::<OutItem>(BUFFER_SIZE);
            senders.push(in_tx);

            let out_stream = (build_pipeline)(in_rx);

            let task:Box<dyn Future<Item=(), Error=()>> =
                Box::new(
                    out_stream
                .forward(out_tx.sink_map_err(|e| {panic!("send_err:{}", e)}))
                .map(|(_pipe,_tx)| { () })
                );
            /*
                .map(|(_pipe,_tx)| { () })
                .map_err(|_e| ());
                */
            tasks.push(task);
            join.add(out_rx);
        }

        let fork = Fork::new(selector, senders);

        ParallelPipeline {
            fork,
            join,
            tasks,
            build_pipeline
        }
    }
}

impl<PipelineStart, PipelineEnd, FSel, FOrd, Task, FBuildPipeline> Stream for ParallelPipeline<PipelineStart, PipelineEnd, FSel, FOrd, Task, FBuildPipeline>
    where PipelineEnd: Stream, PipelineStart:Sink,
          FOrd: Fn(&PipelineEnd::Item) -> u64,
{
    type Item = PipelineEnd::Item;
    type Error = PipelineEnd::Error;

    fn poll(&mut self) -> Poll<Option<Self::Item>, Self::Error> {
        self.join.poll()
    }
}

impl<PipelineStart, PipelineEnd, FSel, FOrd, Task, FBuildPipeline> Sink for ParallelPipeline<PipelineStart, PipelineEnd, FSel, FOrd, Task, FBuildPipeline>
    where PipelineEnd: Stream, PipelineStart:Sink,
          FSel: Fn(&PipelineStart::SinkItem) -> usize,
{
    type SinkItem = PipelineStart::SinkItem;
    type SinkError = PipelineStart::SinkError;

    fn start_send(&mut self, item: Self::SinkItem) -> StartSend<Self::SinkItem, Self::SinkError> {
        self.fork.start_send(item)
    }

    fn poll_complete(&mut self) -> Poll<(), Self::SinkError> {
        self.fork.poll_complete()
    }

    fn close(&mut self) -> Poll<(), Self::SinkError> {
        self.fork.close()
    }
}