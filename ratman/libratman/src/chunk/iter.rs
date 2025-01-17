use crate::{chunk::Chunk, rt::size_commonbuf_t};
use std::{
    future::Future,
    pin::Pin,
    task::{Context, Poll},
};
use tokio::{
    io::{self, AsyncRead, ReadBuf},
    sync::mpsc::{channel, Receiver, Sender},
};

/// Reads chunks from a channel and garbage collect chunks that have already been read
pub struct ChunkIter<const L: usize> {
    /// A sender and receiver for new chunks
    source: Receiver<Chunk<L>>,
    /// The current chunk that is being read from
    current: Option<Chunk<L>>,
    /// Previous chunks that haven't been garbage collected yet
    _garbage: Vec<Chunk<L>>,
    /// Keep track of whether the iterator should just shut down
    _dead: bool,
}

impl<const L: usize> ChunkIter<L> {
    pub fn new() -> (Sender<Chunk<L>>, ChunkIter<L>) {
        let (tx, rx) = channel(size_commonbuf_t::<L>());

        (
            tx,
            Self {
                source: rx,
                current: None,
                _garbage: vec![],
                _dead: false,
            },
        )
    }

    async fn next_chunk(&mut self) {
        let current = self.source.recv().await;
        self.current = current;

        // If we didn't receive a new chunk, we are done!
        if self.current.is_none() {
            self._dead = true;
        }
    }

    async fn read_current_chunk(&mut self, buf: &mut ReadBuf<'_>) {
        let current = self.current.as_mut().unwrap();
        let _ = current.read_to_buf(buf).await;
    }
}

// Since this future operates on an in-memory buffer there's no reason
// not to re-schedule ourselves automatically.  This way the runtime
// can decide whether there are other tasks to schedule, or if this
// turns into a quasi busy-loop.
//
// A real wait point exists when fetching the next block from the
// channel.  After the next block was yielded, we can resume consuming
// it from memory.
impl<const L: usize> AsyncRead for ChunkIter<L> {
    fn poll_read(
        mut self: Pin<&mut Self>,
        ctx: &mut Context,
        buf: &mut ReadBuf<'_>,
    ) -> Poll<io::Result<()>> {
        // Chunk sizes are chosen as such that one chunk can ALWAYS
        // produce at least one full block of data.  The exception is
        // at the tail of a message, where the last chunk MAY contain
        // less.
        match self.current {
            // We are already reading from a chunk, which either needs
            // house-keeping or can be read as a buffer.
            Some(ref mut chunk) => {
                // We just read from the current chunk, as much as the
                // chunk can provide, and the reader demands.  A
                // Poll::Ready from this read means the chunk was
                // completed, but we don't want to signal completion
                // until the end of the iterator.
                if chunk.1 < chunk.0.len() {
                    let chunk_read = self.read_current_chunk(buf);
                    tokio::pin!(chunk_read);

                    // Even if this read was successful, unless we try
                    // to read and have reached the end of the chunk
                    // _stream_ there will always be more to read.
                    let _ = chunk_read.as_mut().as_mut().poll(ctx);
                    ctx.waker().wake_by_ref();
                    Poll::Pending
                }
                // If we read to the end of the chunk, we garbage it
                // to trigger a chunk reload in the next iteration
                else {
                    let old_chunk = self.current.take().unwrap();
                    self._garbage.push(old_chunk);

                    ctx.waker().wake_by_ref();
                    Poll::Pending
                }
            }
            // If we don't currently have a chunk we try to get one
            // from the channel via a local pinned future
            None => {
                // x.x
                if self._dead {
                    return Poll::Ready(Ok(()));
                }

                // Check the allowed garbage based on the chunk size
                let max_garbage = size_commonbuf_t::<L>();

                // Determine the start and end positions that we want
                // to delete (todo: what the fuck?)
                let (start, end) = match (self._garbage.len(), max_garbage) {
                    (len, max) if max - 32 > 0 => (max - 32, len),
                    (len, max) if max - 16 > 0 => (max - 16, len),
                    (len, max) if max - 4 > 0 => (max - 4, len),
                    (len, max) => (max, len),
                };

                // If it makes sense to run a cleanup do it
                if start > 0 && start < end {
                    for _ in start..end {
                        self._garbage.pop();
                    }
                }

                // Finally we try to get a new chunk!  If we reached
                // the end of the chunk stream next_chunk sets
                // dead=true, which stops the iterator
                let source_poll = self.next_chunk();
                tokio::pin!(source_poll);
                let _ = source_poll.as_mut().as_mut().poll(ctx);

                // Signal that we can be read again instantly!
                ctx.waker().wake_by_ref();
                Poll::Pending
            }
        }
    }
}
