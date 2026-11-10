//! The world's listener: the socket manager with a gameplay session behind it.
//!
//! Reference: `src/server/game/Server/WorldSocketMgr.{h,cpp}`, which is a `SocketMgr<WorldSocket>`
//! with the world's own socket options. This is the same object at the size this tree has reached:
//! the base manager, a factory that builds one [`WorldSession`] per connection, and the world's
//! configuration passed in rather than read here.
//!
//! Reference: `WorldSocketMgr::StartWorldNetwork(ioContext, bindIp, port, networkThreads)`.

use common::logging::Log;
use shared::network::socket::{Session, SessionFactory};
use shared::network::socket_mgr::SocketMgr;
use std::sync::Arc;

use crate::server::world_session::WorldSession;
use crate::world::World;

/// The listener a world program opens, and the sessions it builds.
pub struct WorldSocketMgr {
    sockets: SocketMgr,
}

impl WorldSocketMgr {
    /// Bind and start serving gameplay connections.
    ///
    /// The world is shared with every session: each connection needs the world's id and its maps,
    /// and neither changes while the listener runs.
    pub fn start(
        world: Arc<World>,
        log: Log,
        bind_ip: &str,
        port: u16,
        network_threads: usize,
        trace: bool,
    ) -> Result<Self, String> {
        let factory: SessionFactory = {
            let log = log.clone();
            let world = Arc::clone(&world);
            Arc::new(move || {
                Box::new(WorldSession::new(log.clone(), &world)) as Box<dyn Session>
            })
        };

        let mut sockets = SocketMgr::new(factory);
        sockets.set_log(log.clone());
        if trace {
            sockets.set_trace(log);
        }
        sockets
            .start_network(bind_ip, port, network_threads)
            .map_err(|error| format!("Failed to initialize network: {error}"))?;

        Ok(Self { sockets })
    }

    /// The port the listener bound.
    pub fn port(&self) -> Option<u16> {
        self.sockets.port()
    }

    /// How many connections have been accepted since it started.
    pub fn accepted_count(&self) -> usize {
        self.sockets.accepted_count()
    }

    /// A handle on that count, for a status writer.
    pub fn accepted_handle(&self) -> Arc<std::sync::atomic::AtomicUsize> {
        self.sockets.accepted_handle()
    }

    /// Close the listener and wait for its threads.
    ///
    /// Reference: `WorldSocketMgr::StopNetwork`.
    pub fn stop(mut self) {
        self.sockets.stop_network();
    }
}
