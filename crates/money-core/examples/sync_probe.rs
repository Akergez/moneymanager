//! Syncs a ledger directory with a remote from the command line, and says
//! what is in it afterwards.
//!
//! ```sh
//! cargo run -p money-core --example sync_probe -- <ledger-dir> 'tresse://…'
//! ```
//!
//! For checking a storage's credentials, or what a device would receive,
//! without an interface in the way. The directory is created if it is not
//! there; pointing it at a real ledger syncs that ledger for real.

use money_core::models::{Account, Category, Expense, TopUp, TopUpCategory, Transfer};
use money_core::remote::{self, RemoteConfig};
use money_core::store::Store;

fn main() -> Result<(), String> {
    let mut args = std::env::args().skip(1);
    let (Some(dir), Some(config_string)) = (args.next(), args.next()) else {
        return Err("usage: sync_probe <ledger-dir> <config-string>".to_string());
    };
    let remote = RemoteConfig::from_share_string(&config_string)?;
    remote.validate()?;

    // A throwaway source: the probe writes nothing of its own to stamp.
    let mut store = Store::open(std::path::Path::new(&dir), remote::generate_source())?;
    money_core::tresse::config::write_remote(std::path::Path::new(&dir), Some(&remote))?;
    let report = store.sync()?;
    println!("pulled {} chunks, pushed {}", report.pulled, report.pushed);
    println!("accounts: {}", Account::read_all(&store)?.len());
    for account in Account::read_all(&store)? {
        println!(
            "  {} · {} · opening {}",
            account.name, account.currency, account.opening_balance
        );
    }
    println!("expense categories: {}", Category::read_all(&store)?.len());
    println!(
        "income categories: {}",
        TopUpCategory::read_all(&store)?.len()
    );
    println!("expenses: {}", Expense::read_all(&store)?.len());
    println!("income: {}", TopUp::read_all(&store)?.len());
    println!("transfers: {}", Transfer::read_all(&store)?.len());
    Ok(())
}
