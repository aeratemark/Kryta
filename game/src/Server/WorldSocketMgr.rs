//! The world's listener: the socket manager with a gameplay session behind it.
//!
//! Reference: `src/server/game/Server/WorldSocketMgr.{h,cpp}`, which is a `SocketMgr<WorldSocket>`
//! with the world's own socket options. This is the same object at the size this tree has reached:
//! the base manager, a factory that builds one [`WorldSession`] per connection, and the world's
//! configuration passed in rather than read here.
//!
//! Reference: `WorldSocketMgr::StartWorldNetwork(ioContext, bindIp, port, networkThreads)`.

use common::logging::Log;
use database::database::DatabaseWorkerPool;
use shared::network::socket::{Session, SessionFactory};
use shared::network::socket_mgr::SocketMgr;
use std::sync::{Arc, Mutex};

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
    /// and neither changes while the listener runs. The character pool is shared too, because a
    /// creation screen's confirm writes a character row and nothing else may open a second one. The
    /// login pool is shared so a connection can claim the travel record its login wrote.
    #[allow(clippy::too_many_arguments)]
    pub fn start(
        world: Arc<World>,
        characters: Option<Arc<Mutex<DatabaseWorkerPool>>>,
        login: Option<Arc<Mutex<DatabaseWorkerPool>>>,
        log: Log,
        bind_ip: &str,
        port: u16,
        network_threads: usize,
        trace: bool,
    ) -> Result<Self, String> {
        let factory: SessionFactory = {
            let log = log.clone();
            let world = Arc::clone(&world);
            let characters = characters.clone();
            let login = login.clone();
            Arc::new(move || {
                let session = match (characters.as_ref(), login.as_ref()) {
                    (Some(characters), Some(login)) => WorldSession::with_characters_and_login(
                        log.clone(),
                        &world,
                        Arc::clone(characters),
                        Arc::clone(login),
                    ),
                    (Some(characters), None) => {
                        WorldSession::with_characters(log.clone(), &world, Arc::clone(characters))
                    }
                    (None, _) => WorldSession::new(log.clone(), &world),
                };
                Box::new(session) as Box<dyn Session>
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

    /// A handle on the live count - the connections being served right now - for a status writer.
    pub fn live_handle(&self) -> Arc<std::sync::atomic::AtomicUsize> {
        self.sockets.live_handle()
    }

    /// Close the listener and wait for its threads.
    ///
    /// Reference: `WorldSocketMgr::StopNetwork`.
    pub fn stop(mut self) {
        self.sockets.stop_network();
    }
}
