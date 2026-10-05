//! The ledger of Money Manager, with no interface attached.
//!
//! Everything that decides what is stored and how it merges lives here, so the
//! application above it can change toolkits without the data noticing:
//!
//! - [`models`] — the records, as plain structs;
//! - [`store`] — the RDX CRDT document they are kept in, chunked for sync;
//! - [`services`] — how each record is laid out inside that document;
//! - [`ledger`] — per-account views and balances, derived at read time;
//! - [`remote`] — the S3 remote and the one-string form of its credentials;
//! - [`format`] — how amounts are written out.
//!
//! The on-disk and on-wire format is shared with the terminal version of the
//! application and with every device already syncing a ledger. Nothing in
//! [`store`] or [`services`] may change what a record looks like.

pub mod format;
pub mod ledger;
pub mod models;
pub mod remote;
mod services;
pub mod store;

pub use rdx_sync::SyncReport;
