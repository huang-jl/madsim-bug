use futures::task::LocalSpawnExt;
use log::{info, warn};
use madsim::{
    net::{
        rpc::{Deserialize, Serialize},
        NetLocalHandle,
    },
    Handle, LocalHandle, Request,
};
use std::sync::Arc;
use std::{
    net::SocketAddr,
    sync::atomic::{AtomicU64, Ordering},
    time::Duration,
};

#[derive(Debug, Clone, Copy, Serialize, Deserialize, Request)]
#[rtype("()")]
struct Incr;

#[derive(Debug, Clone)]
struct MockSercer {
    inner: Arc<AtomicU64>,
}

#[derive(Clone)]
struct Client {
    handle: LocalHandle,
    server_addr: SocketAddr,
}

#[madsim::service]
impl MockSercer {
    pub fn new() -> Self {
        let server = MockSercer {
            inner: Arc::new(AtomicU64::new(0)),
        };
        server.add_rpc_handler();
        server
    }

    #[rpc]
    fn incr(&self, request: Incr) {
        info!(
            "Receive increment request, local addr = {}",
            NetLocalHandle::current().local_addr()
        );
        self.inner.fetch_add(1, Ordering::SeqCst);
    }
}

impl Client {
    pub fn new(addr: SocketAddr, server_addr: SocketAddr) -> Self {
        let handle = Handle::current();
        let local_handle = handle.create_host(addr).unwrap();
        Client {
            server_addr,
            handle: local_handle,
        }
    }

    pub async fn send_req(&self) {
        let this = self.clone();
        self.handle
            .spawn(async move {
                info!(
                    "Send Incr request from {} to {}",
                    NetLocalHandle::current().local_addr(),
                    this.server_addr
                );
                let net = NetLocalHandle::current();
                match net
                    .call_timeout(this.server_addr, Incr, Duration::from_secs(1))
                    .await
                {
                    Ok(_) => info!("Send incr success"),
                    Err(err) => warn!("Send incr fail: {}", err),
                }
            })
            .await;
    }
}

#[cfg(test)]
mod tests {
    use crate::{Client, MockSercer};
    use log::warn;
    use madsim::{time::sleep, Handle};
    use std::net::SocketAddr;

    #[madsim::test]
    async fn simple_test() {
        let server_addr = "10.0.0.1:8000".parse::<SocketAddr>().unwrap();
        let handle = Handle::current();
        let _server = handle
            .create_host(server_addr)
            .unwrap()
            .spawn(async move { MockSercer::new() })
            .await;

        let client_addr = "10.0.0.2:1234".parse::<SocketAddr>().unwrap();
        let client = Client::new(client_addr, server_addr);

        for _ in 0..10 {
            client.send_req().await;
        }

        handle.kill(server_addr);
        warn!("Server crash!");

        sleep(Duration::from_secs(5)).await;

        let _server = handle
            .create_host(server_addr)
            .unwrap()
            .spawn(async move { MockSercer::new() })
            .await;

        warn!("Server restart!");
        sleep(Duration::from_secs(5)).await;

        for _ in 0..10 {
            client.send_req().await;
        }
    }
}
