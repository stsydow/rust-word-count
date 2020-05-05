//#![feature(impl_trait_in_bindings)]
//extern crate num_derive;

//pub mod error;

pub mod logging;
//pub mod context;
pub mod util;
//pub mod stream_group_by;
pub mod global_context;
pub mod stream_fork;
pub mod stream_join;
pub mod stream_shuffle;
pub mod stream_shuffle_buffered;
//pub mod parallel_pipeline;
pub mod selective_context;
pub mod probe_stream;
mod log_histogram;
pub use crate::log_histogram::LogHistogram;
pub mod instrumented_map;
pub mod instrumented_fold;

use std::time::{ Instant };
use tokio::prelude::*;

impl<T: ?Sized> StreamExt for T where T: Stream {}

pub trait StreamExt: Stream {
    // TODO rename to map
    fn instrumented_map<U, F>(self, f: F, name: String) -> instrumented_map::InstrumentedMap<Self, F>
        where F: FnMut(Self::Item) -> U,
              Self: Sized
    {
        instrumented_map::new(self, f, name)
    }

    // TODO rename to fold
    fn instrumented_fold<Fut, T, F>(self, init:T, f:F, name: String) -> instrumented_fold::InstrumentedFold<Self, F, Fut, T>
        where F: FnMut(T, Self::Item) -> Fut,
              Fut: IntoFuture<Item = T>,
              Self::Error: From<Fut::Error>,
              Self: Sized,

    {
        instrumented_fold::new(self, f, init, name)
    }


    //TODO instrument ; add name
    fn selective_context<R, Key, Ctx, CtxInit, FSel, FWork>(
        self,
        ctx_builder: CtxInit,
        selector: FSel,
        work: FWork,
    ) -> selective_context::SelectiveContext<Key, Ctx, Self, CtxInit, FSel, FWork>
    where
        //Ctx:Context<Event=Event, Result=R>,
        Key: Ord,
        CtxInit: Fn(&Key) -> Ctx,
        FSel: Fn(&Self::Item) -> Key,
        FWork: Fn(&mut Ctx, &Self::Item) -> R,
        Self: Sized,
    {
        selective_context::selective_context(self, ctx_builder, selector, work)
    }

    fn fork(self, degree: usize) -> Vec<tokio::sync::mpsc::Receiver<Self::Item>>
        where
            Self::Error: std::fmt::Display,
            Self::Item: Send,
        Self: Sized + Send + 'static,
    {
        stream_fork::fork_stream(self, degree)
    }

    fn meter(self, name: String) -> probe_stream::Meter<Self>
        where Self: Sized
    {
        probe_stream::Meter::new(self, name)
    }

    fn tag(self) -> probe_stream::Tag<Self>
        where Self: Sized
    {
        probe_stream::Tag::new(self)
    }

    fn probe<I>(self, name: String) -> probe_stream::Probe<Self>
        where Self: Stream<Item=(Instant, I)>,
            Self: Sized
    {
        probe_stream::Probe::new(self, name)
    }
}

//TODO ????
/*pub trait ParallelStream
{
    type Stream;


}
*/
pub struct ParallelStream<S,Sel>
{
    tagged: bool,
    partitioing:Option<Sel>,
    streams: Vec<S>,


}

/* TODO
// see https://github.com/async-rs/parallel-stream/
  fork Stream -> ParallelStream

  pub trait ParallelStream {

        map()
        context()
        ?fold()

        join() -> Stream
        reorder()
  }


  pub trait TaggedStream {

  }
  fork(Stream) -> TaggedStream
  join!([TaggedStream]) -> Stream

 */

