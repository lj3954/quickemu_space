// SPDX-License-Identifier: GPL-3.0-only

use std::path::PathBuf;

use cosmic::cosmic_config::{self, cosmic_config_derive::CosmicConfigEntry, CosmicConfigEntry};

#[derive(Debug, Default, Clone, CosmicConfigEntry, Eq, PartialEq)]
#[version = 1]
pub struct Config {
    pub default_vm_dir: PathBuf,
    pub existing_vm_configs: Vec<PathBuf>,
}
