mod handler;
mod net;
mod server;
mod threadpool;

#[macro_use]
mod utils;

#[macro_use]
extern crate lazy_static;

pub use server::local_client;
pub use server::server::run_server;
pub use utils::utils::{is_root_user, set_log_level};
