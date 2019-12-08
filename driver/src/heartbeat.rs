//! This module is responsible for sending heart beat packets periodically to the judge board
//! server.
//!

use std::time::{Duration, SystemTime};

use procfs::{CpuInfo, Meminfo};
use serde::Serialize;

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

/// Represent a heartbeat packet that will be sent to the judge board server.
#[derive(Debug, Serialize)]
struct Heartbeat {
    /// Timestamp of the heartbeat packet.
    #[serde(rename = "timestamp")]
    timestamp: SystemTime,

    /// Number of CPU cores installed on this judge node.
    #[serde(rename = "cores")]
    cores: u32,

    /// Total physical memory installed on this judge node, in bytes.
    #[serde(rename = "totalPhysicalMemory")]
    total_physical_memory: u64,

    /// Free physical memory installed on this judge node, in bytes.
    #[serde(rename = "freePhysicalMemory")]
    free_physical_memory: u64,

    /// Total size of swap space, in bytes.
    #[serde(rename = "totalSwapSpace")]
    total_swap_space: u64,

    /// Size of free swap space, in bytes.
    #[serde(rename = "freeSwapSpace")]
    free_swap_space: u64,

    #[serde(rename = "cachedSwapSpace")]
    cached_swap_space: u64,
}

impl Heartbeat {
    /// Create a new `Heartbeat` value.
    fn new() -> Result<Heartbeat> {
        let memory = MemoryFootprint::new()?;
        Ok(Heartbeat {
            timestamp: SystemTime::now(),
            cores: get_cores()?,
            total_physical_memory: memory.total_physical_memory,
            free_physical_memory: memory.free_physical_memory,
            total_swap_space: memory.total_swap_space,
            free_swap_space: memory.free_swap_space,
            cached_swap_space: memory.cached_swap_space
        })
    }
}

/// The minimal number of seconds between two adjacent heartbeat packets.
const MIN_HEARTBEAT_INTERVAL: u32 = 3;

/// This function is the entry point of the heartbeat daemon thread.
fn heartbeat_daemon_entry() {
    let config = crate::config::app_config();
    let heartbeat_interval = Duration::from_secs(
        *crate::utils::max(&config.heartbeat_interval, &MIN_HEARTBEAT_INTERVAL) as u64);

    loop {
        std::thread::sleep(heartbeat_interval);

        let heartbeat = match Heartbeat::new() {
            Ok(hb) => hb,
            Err(e) => {
                log::error!("failed to create heartbeat packet: {}", e);
                continue;
            }
        };

        match crate::restful::patch("/judges", &heartbeat) {
            Ok(..) => (),
            Err(e) => log::error!("failed to send heartbeat packet: {}", e)
        };

        log::trace!("heartbeat packet sent successfully.");
    }
}

/// Start the heartbeat daemon thread.
pub fn start_daemon() {
    std::thread::spawn(heartbeat_daemon_entry);
}
