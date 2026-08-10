#![deny(unsafe_code)]

mod auth;
mod config;
mod error;
mod fetch;
mod html;
mod http;
mod key_init;
mod quota;
mod search;
mod security;
mod server;

pub use config::{Config, LISTEN_PATH, REQUEST_SCHEMA, RESPONSE_SCHEMA};
pub use error::{Error, Result};
pub use fetch::{
    FETCH_PATH, FETCH_REQUEST_SCHEMA, FETCH_RESPONSE_SCHEMA, FetchBackend, FetchRequest,
    FetchResponse,
};
pub use key_init::{KeyInitialization, initialize_response_keypair};
pub use search::{BraveSearch, SearchBackend, SearchRequest, SearchResponse, SearchResult};
pub use security::is_public_upstream_ip;
pub use server::{Admission, run};
