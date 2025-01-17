//! Shared sync & async runtime utilities
//!
//! ## A WORD OF WARNING!
//!
//! tokio::spawn is FORBIDDEN in this module!  Only ever use
//! tokio::spawn_local!
//!
//! Most likely you will want to simply call `new_async_thread(...)`

use crate::Result;
use std::{
    future::Future,
    sync::{
        mpsc::{sync_channel, Receiver as SyncReceiver, SyncSender},
        Arc,
    },
};
use tokio::{
    runtime::{Builder, Runtime},
    task::LocalSet,
};

pub mod reader;
pub mod writer;

/// An arbitrary buffer scheme size called "commonbuf"
///
/// Standardises the size of channel buffers based on a common scheme
/// of sub-diving chunk/ block sizes.  This provides a unified
/// mechanism to limit memory size.
///
/// Completely arbitrarily: 8MB divided by the size of a chunk, so 1M
/// chunk => 8 buffer slots.  1K chunk => 8192 buffer slots.
pub const fn size_commonbuf_t<const T: usize>() -> usize {
    (1024 * 1024 * 8) / T
}

/// Encapsulates a single threaded async system
pub struct AsyncSystem {
    #[allow(unused)]
    label: String,
    rt: Runtime,
    #[allow(unused)]
    set: LocalSet,
    irq: (SyncSender<()>, SyncReceiver<()>),
}

impl AsyncSystem {
    pub fn new(label: String, stack_mb: usize) -> Arc<Self> {
        Arc::new(Self {
            rt: Builder::new_multi_thread()
                .thread_name(&label)
                .enable_io()
                .enable_time()
                .thread_stack_size(1024 * 1024 /* MibiByte */ * stack_mb)
                .build()
                .expect("failed to start async thread!"),
            set: LocalSet::new(),
            label,
            irq: sync_channel(4),
        })
    }

    #[inline]
    pub fn block_on<O>(&self, f: impl Future<Output = O>) -> O {
        self.rt.block_on(f)
    }

    pub fn async_interrupt(self: &Arc<Self>) {
        let _ = self.irq.0.send(());
    }

    pub fn exec<O>(&self, f: impl Future<Output = O>) -> O {
        self.rt.block_on(async { self.set.run_until(f).await })
    }
}

// static THREAD_JOIN_MAP: Lazy<Arc<Mutex<Vec<JoinHandle<()>>>>> =
//     Lazy::new(|| Arc::new(Mutex::new(vec![])));

/// Spawn new worker thread with an async system launcher
pub fn new_async_thread<S, F, O>(label: S, stack_mb: usize, f: F)
where
    S: Into<String>,
    F: Future<Output = Result<O>> + Send + 'static,
    O: Sized + Send + 'static,
{
    let label = label.into();
    std::thread::Builder::new()
        .name(label.clone())
        .stack_size(stack_mb * 1024 * 1024)
        .spawn(move || {
            trace!("Starting new async thread system: {label}");
            let system = AsyncSystem::new(label.clone(), stack_mb);
            match system.exec(f) {
                Ok(_) => trace!("Worker thread {label} completed successfully!"),
                Err(ref e) => error!("Worker thread {label} encountered a fatal error: {e}"),
            }
        })
        .expect("failed to spawn thread");
}

#[test]
fn simple_tcp_transfer() {
    use crate::rt::{
        reader::{AsyncVecReader, LengthReader},
        writer::{write_u32, AsyncWriter},
    };
    use rand::RngCore;
    use std::time::Duration;
    use tokio::{
        net::{TcpListener, TcpStream},
        sync::mpsc::channel,
        time::timeout,
    };

    // Receiver
    let (tx, mut rx) = channel(1);

    new_async_thread("tcp server", 32, async move {
        let l = TcpListener::bind("localhost:5555").await.unwrap();
        let (mut stream, _addr) = l.accept().await.unwrap();

        let length = LengthReader::new(&mut stream).read_u32().await.unwrap();

        tx.send(
            AsyncVecReader::new(length as usize, &mut stream)
                .read_to_vec()
                .await,
        )
        .await
        .unwrap();

        Ok(())
    });

    let mut input_data = vec![0; 1024 * 8];
    rand::thread_rng().fill_bytes(&mut input_data);

    // Sender
    let to_send = input_data.clone();
    new_async_thread("tcp client", 32, async move {
        let to_send = to_send.clone();

        let mut stream = timeout(Duration::from_secs(2), TcpStream::connect("localhost:5555"))
            .await
            .unwrap()
            .unwrap();

        write_u32(&mut stream, to_send.len() as u32).await.unwrap();
        AsyncWriter::new(to_send.as_slice(), &mut stream)
            .write_buffer()
            .await
            .unwrap();

        Ok(())
    });

    let received_data = AsyncSystem::new("main".into(), 1)
        .exec(async move { rx.recv().await.unwrap() })
        .unwrap();

    println!("We got {} datas", received_data.len());
    assert_eq!(input_data, received_data);
}

#[test]
fn block_on() {
    let system = AsyncSystem::new("block_on".to_string(), 1);
    system.block_on(async {
        println!("Simple block on");
    });
}

#[test]
#[should_panic]
fn nested_block_on_panics() {
    let system = AsyncSystem::new("nested_block_on_panics".to_string(), 1);
    system.clone().block_on(async move {
        system.clone().block_on(async move {
            println!("Nested block on");
        });
    });
}

#[test]
fn test_spawn() {
    use tokio::{sync::mpsc, time};

    async fn wait_n_send(s: mpsc::Sender<String>, n: u64) {
        time::sleep(std::time::Duration::from_secs(n)).await;
        s.send("Waited {n} and meowed".to_string()).await.unwrap();
    }

    async fn recv_and_print(mut r: mpsc::Receiver<String>) {
        let mut ctr = 0;
        while let Some(msg) = r.recv().await {
            println!("Msg: {msg}");
            ctr += 1;
        }

        // Enforce that we did indeed receive three messages
        assert_eq!(ctr, 3);
    }

    async fn root_job() {
        use tokio::task::spawn;
        let (tx, rx) = mpsc::channel(8);
        spawn(wait_n_send(tx.clone(), 1));
        spawn(wait_n_send(tx.clone(), 2));
        spawn(wait_n_send(tx, 3));

        recv_and_print(rx).await;
    }

    AsyncSystem::new("test_spawn".to_owned(), 1).exec(root_job());
}

#[test]
fn send_between_systems() {
    let (tx, mut rx) = tokio::sync::mpsc::channel(1);

    new_async_thread("receiver-system", 1, async move {
        let msg = rx.recv().await;
        println!("Received {:?} from across the (memory) pond :3", msg);
        assert!(msg.is_some());
        Ok(())
    });

    new_async_thread("sender-system", 1, async move {
        tx.send("Bonk! ^w^".to_owned()).await.unwrap();
        Ok(())
    });
}
