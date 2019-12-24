//! This module is responsible for sending heart beat packets periodically to the judge board
//! server.
//!

use std::time::Duration;
use std::sync::Arc;

use procfs::{CpuInfo, Meminfo};

use crate::restful::RestfulClient;
use crate::restful::entities::Heartbeat;

error_chain::error_chain! {
    types {
        Error, ErrorKind, ResultExt, Result;
    }

    foreign_links {
        ProcError(::procfs::ProcError);
    }
}

/// Get number of CPU cores installed on the judge node.
fn get_cores() -> Result<u32> {
    Ok(CpuInfo::new()?.num_cores() as u32)
}

/// Provide information about memory footprint of the current judge node.
#[derive(Debug, Clone, Copy)]
struct MemoryFootprint {
    /// The total physical memory size, in bytes.
    total_physical_memory: u64,

    /// The free physical memory size, in bytes.
    free_physical_memory: u64,

    /// The total swap space size, in bytes.
    total_swap_space: u64,

    /// The free swap space size, in bytes.
    free_swap_space: u64,

    /// The cached swap size, in bytes.
    cached_swap_space: u64,
}

impl MemoryFootprint {
    /// Create a new `MemoryFootprint` value.
    fn new() -> Result<MemoryFootprint> {
        let memory = Meminfo::new()?;
        Ok(MemoryFootprint {
            total_physical_memory: memory.mem_total,
            free_physical_memory: memory.mem_free,
            total_swap_space: memory.swap_total,
            free_swap_space: memory.swap_free,
            cached_swap_space: memory.swap_cached,
        })
    }
}

/// Create a new heartbeat packet.
fn create_heartbeat() -> Result<Heartbeat> {
    let mut hb = Heartbeat::new();
    let memory = MemoryFootprint::new()?;

    hb.cores = get_cores()?;
    hb.total_physical_memory = memory.total_physical_memory;
    hb.free_physical_memory = memory.free_physical_memory;
    hb.total_swap_space = memory.total_swap_space;
    hb.free_swap_space = memory.free_swap_space;
    hb.cached_swap_space = memory.cached_swap_space;

    Ok(hb)
}

/// The minimal number of seconds between two adjacent heartbeat packets.
const MIN_HEARTBEAT_INTERVAL: Duration = Duration::from_secs(3);

/// This function is the entry point of the heartbeat daemon thread.
fn heartbeat_daemon_entry(options: HeartbeatDaemonOptions) {
    let heartbeat_interval = *crate::utils::max(
        &options.heartbeat_interval, &MIN_HEARTBEAT_INTERVAL);

    loop {
        std::thread::sleep(heartbeat_interval);

        let heartbeat = match create_heartbeat() {
            Ok(hb) => hb,
            Err(e) => {
                log::error!("failed to create heartbeat packet: {}", e);
                continue;
            }
        };

        match options.rest.patch_heartbeat(&heartbeat) {
            Ok(..) => (),
            Err(e) => log::error!("failed to send heartbeat packet: {}", e)
        };

        log::trace!("heartbeat packet sent successfully.");
    }
}

/// Provide options for heartbeat daemons.
pub struct HeartbeatDaemonOptions {
    /// The RESTful client, connected to the judge board server.
    pub rest: Arc<RestfulClient>,

    /// The interval between two consecutive heartbeat packets, in seconds.
    pub heartbeat_interval: Duration,
}

impl HeartbeatDaemonOptions {
    /// Create a new `HeartbeatDaemonOptions` value.
    pub fn new(rest: Arc<RestfulClient>, heartbeat_interval: Duration) -> Self {
        HeartbeatDaemonOptions { rest, heartbeat_interval }
    }
}

/// Start the heartbeat daemon thread.
pub fn start_daemon(options: HeartbeatDaemonOptions) {
    std::thread::spawn(move || heartbeat_daemon_entry(options));
}
