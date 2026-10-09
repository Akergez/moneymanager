//! The ledger of Money Manager, with no interface attached.
//!
//! Everything that decides what is stored and how it merges lives here, so the
//! application above it can change toolkits without the data noticing:
//!
//! - [`models`] — the records, as plain structs;
//! - [`store`] — the native RDX document versioned and synchronized by Tresse;
//! - [`services`] — how each record is laid out inside that document;
//! - [`ledger`] — per-account views and balances, derived at read time;
//! - [`remote`] — the Tresse remote and the one-string form of its credentials;
//! - [`format`] — how amounts are written out.
//!
//! The internal record schema is preserved during migration; storage and the
//! remote protocol now belong to Tresse.

pub mod format;
pub mod ledger;
pub mod models;
pub mod remote;
mod services;
pub mod store;

pub use money_tresse as tresse;
pub use store::SyncReport;
