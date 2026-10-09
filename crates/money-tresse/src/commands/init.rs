use std::path::Path;

use crate::cas::open_local_store;
use crate::config::TresseConfig;

pub fn run(root: &Path) -> Result<(), Box<dyn std::error::Error>> {
    let tresse_dir = root.join(".tresse");
    std::fs::create_dir_all(&tresse_dir)?;
    if !root.join("tresse.toml").exists() {
        TresseConfig::init(root)?;
    }

    open_local_store(root)?;

    println!("initialized tresse repo at {}", root.display());
    Ok(())
}
