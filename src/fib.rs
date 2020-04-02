use actix::prelude::*;
use tracing::*;

#[derive(Copy, Clone, PartialEq, PartialOrd, Eq, Ord, Debug, Hash)]
pub struct Fibonacci(pub u32);

impl Message for Fibonacci {
    type Result = Result<u64, ()>;
}

pub struct FibActor;

impl FibActor {
    pub fn new() -> Self { FibActor {} }
}

impl Actor for FibActor {
    type Context = Context<Self>;
}

impl Handler<Fibonacci> for FibActor {
    // type Result = ResponseActFuture<Self, u64, ()>;
    type Result = Result<u64, ()>;

    fn handle(&mut self, msg: Fibonacci, _: &mut Self::Context) -> Self::Result {
        let r = if msg.0 == 0 {
            Err(())
        } else if msg.0 == 1 {
            Ok(1)
        } else {
            let mut i = 0;
            let mut sum = 0;
            let mut last = 0;
            let mut curr = 1;
            while i < msg.0 - 1 {
                sum = last + curr;
                last = curr;
                curr = sum;
                i += 1;
            }
            event!(Level::INFO, %sum, ?msg, "fibonacci sum" );
            Ok(sum)
        };

        r
        // Box::new( r )
    }
}
