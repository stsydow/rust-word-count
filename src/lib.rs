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

use tokio::prelude::*;

impl<T: ?Sized> StreamExt for T where T: Stream {}

pub trait StreamExt: Stream {
    fn instrumented_map<U, F>(self, f: F, name: String) -> instrumented_map::InstrumentedMap<Self, F>
        where F: FnMut(Self::Item) -> U,
              Self: Sized
    {
        instrumented_map::new(self, f, name)
    }

    fn instrumented_fold<Fut, T, F>(self, init:T, f:F, name: String) -> instrumented_fold::InstrumentedFold<Self, F, Fut, T>
        where F: FnMut(T, Self::Item) -> Fut,
              Fut: IntoFuture<Item = T>,
              Self::Error: From<Fut::Error>,
              Self: Sized,

    {
        instrumented_fold::new(self, f, init, name)
    }
}
