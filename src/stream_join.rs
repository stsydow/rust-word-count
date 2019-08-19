use futures::{Poll, Stream, Async, try_ready};
use std::collections::{BinaryHeap};
use tokio::sync::mpsc::{Sender, Receiver};
use tokio::sync::mpsc::error::RecvError;
use std::cmp::Ordering;

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

    pub fn add(&mut self, stream: Receiver<Event>) {
        self.pipelines.push(stream);
        self.last_values.push(QueueItem{event:None,
            order: 0,
            pipeline_index: self.pipelines.len() -1})
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