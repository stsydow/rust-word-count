use futures::{try_ready, Async, Poll, Stream};
use std::collections::btree_map::Entry;
use std::collections::BTreeMap;

pub struct SelectiveContext<Key, Ctx, InStream, FInit, FSel, FWork> {
    ctx_init: FInit,
    selector: FSel,
    work: FWork,
    context_map: BTreeMap<Key, Ctx>,
    input: InStream,
}

impl<R, Key, Ctx, InStream, FInit, FSel, FWork>
    SelectiveContext<Key, Ctx, InStream, FInit, FSel, FWork>
where
    Key: Ord,
    InStream: Stream,
    FInit: Fn(&Key) -> Ctx,
    FSel: Fn(&InStream::Item) -> Key,
    FWork: Fn(&mut Ctx, &InStream::Item) -> R,
{
    pub fn new(input: InStream, ctx_builder: FInit, selector: FSel, work: FWork) -> Self {
        SelectiveContext {
            ctx_init: ctx_builder,
            selector,
            work,
            context_map: BTreeMap::new(),
            input,
        }
    }

    fn apply(&mut self, event: &InStream::Item) -> R {
        let key = (self.selector)(event);

        let work_fn = &self.work;
        let context = match self.context_map.entry(key) {
            Entry::Occupied(entry) => entry.into_mut(),
            Entry::Vacant(entry) => {
                let inital_ctx = (&self.ctx_init)(entry.key());
                entry.insert(inital_ctx)
            }
        };
        work_fn(context, &event)

        //TODO decide / implement context termination (via work()'s Return Type? An extra function? Timeout registration? )
    }
}

pub fn selective_context<R, Key, Ctx, InStream, CtxInit, FSel, FWork>(
    input: InStream,
    ctx_builder: CtxInit,
    selector: FSel,
    work: FWork,
) -> SelectiveContext<Key, Ctx, InStream, CtxInit, FSel, FWork>
where
    InStream: Stream,
    Key: Ord,
    CtxInit: Fn(&Key) -> Ctx,
    FSel: Fn(&InStream::Item) -> Key,
    FWork: Fn(&mut Ctx, &InStream::Item) -> R,
{
    SelectiveContext {
        ctx_init: ctx_builder,
        selector,
        work,
        context_map: BTreeMap::new(),
        input,
    }
}

impl<R, Key, Ctx, InStream, CtxInit, FSel, FWork> Stream
    for SelectiveContext<Key, Ctx, InStream, CtxInit, FSel, FWork>
where
    InStream: Stream,
    Key: Ord,
    CtxInit: Fn(&Key) -> Ctx,
    FSel: Fn(&InStream::Item) -> Key,
    FWork: Fn(&mut Ctx, &InStream::Item) -> R,
{
    type Item = R;
    type Error = InStream::Error;

    fn poll(&mut self) -> Poll<Option<Self::Item>, Self::Error> {
        let async_event = try_ready!(self.input.poll());
        let result = match async_event {
            Some(event) => Some(self.apply(&event)),
            None => None,
        };

        Ok(Async::Ready(result))
    }
}
