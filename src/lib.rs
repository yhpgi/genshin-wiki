pub mod api;
pub mod cli;
pub mod config;
pub mod core;
pub mod error;
pub mod io;
pub mod logging;
pub mod model;
pub mod testing;
pub mod transform;
pub mod utils;

#[cfg(not(target_env = "msvc"))]
use tikv_jemallocator::Jemalloc;

#[cfg(not(target_env = "msvc"))]
#[global_allocator]
static GLOBAL: Jemalloc = Jemalloc;
