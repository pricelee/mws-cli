use std::path::PathBuf;
use std::sync::Arc;

use crate::auth::AccountStore;
use crate::keyring::{DiskVault, Vault};
use crate::output::Format;

pub struct CliContext {
    pub account_name: String,
    pub tenant: String,
    pub client_id: String,
    pub format: Format,
    pub verbose: bool,
    pub all: bool,
    pub dry_run: bool,
    pub yes: bool,
    pub store: AccountStore,
    pub config_dir: PathBuf,
    pub graph_base: String,
}

impl CliContext {
    pub fn build(args: &crate::cli::Cli) -> anyhow::Result<Self> {
        let (config_dir, vault_service) = {
            #[cfg(feature = "test-helpers")]
            {
                if let Some(dir) = args.config_dir.clone() {
                    // Test override: derive a deterministic, unique service name from the
                    // directory path so parallel test runs each get their own Windows
                    // Credential Manager entry and cannot collide.
                    let svc = format!("mws-test-{}", fnv1a_hex(dir.to_string_lossy().as_bytes()));
                    (dir, svc)
                } else {
                    let proj = directories::ProjectDirs::from("com", "mws-cli", "mws-cli")
                        .ok_or_else(|| anyhow::anyhow!("no config dir"))?;
                    (proj.config_dir().to_path_buf(), "mws-cli".to_string())
                }
            }
            #[cfg(not(feature = "test-helpers"))]
            {
                let proj = directories::ProjectDirs::from("com", "mws-cli", "mws-cli")
                    .ok_or_else(|| anyhow::anyhow!("no config dir"))?;
                (proj.config_dir().to_path_buf(), "mws-cli".to_string())
            }
        };
        std::fs::create_dir_all(&config_dir)?;
        let vault: Arc<dyn Vault> = Arc::new(DiskVault::new(config_dir.join("accounts"), &vault_service));
        let store = AccountStore::new(vault);
        let format = match args.output.as_deref() {
            Some(s) => Format::parse(s).map_err(|e| anyhow::anyhow!(e.to_string()))?,
            None => {
                use std::io::IsTerminal;
                Format::auto(std::io::stdout().is_terminal())
            }
        };
        let graph_base = args.graph_base.clone().unwrap_or_else(|| {
            if args.beta {
                "https://graph.microsoft.com/beta".to_string()
            } else {
                "https://graph.microsoft.com/v1.0".to_string()
            }
        });
        Ok(Self {
            account_name: args.account.clone().unwrap_or_else(|| "default".to_string()),
            tenant: args.tenant.clone().unwrap_or_else(|| crate::auth::DEFAULT_TENANT.to_string()),
            client_id: args.client_id.clone().unwrap_or_else(|| crate::auth::DEFAULT_CLIENT_ID.to_string()),
            format,
            verbose: args.verbose,
            all: args.all,
            dry_run: args.dry_run,
            yes: args.yes,
            store,
            config_dir,
            graph_base,
        })
    }
}

/// FNV-1a 64-bit hash — deterministic across processes and platforms.
/// Used to derive a unique Windows Credential Manager service name from a
/// test-supplied config-dir path so parallel test invocations cannot collide.
#[cfg(feature = "test-helpers")]
fn fnv1a_hex(data: &[u8]) -> String {
    const FNV_OFFSET: u64 = 14_695_981_039_346_656_037;
    const FNV_PRIME: u64 = 1_099_511_628_211;
    let mut hash = FNV_OFFSET;
    for &byte in data {
        hash ^= u64::from(byte);
        hash = hash.wrapping_mul(FNV_PRIME);
    }
    format!("{hash:016x}")
}
