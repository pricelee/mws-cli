use std::path::PathBuf;
use std::sync::Arc;

use mws_auth::AccountStore;
use mws_keyring::{DiskVault, Vault};
use mws_output::Format;

pub struct CliContext {
    pub account_name: String,
    pub tenant: String,
    pub client_id: String,
    pub format: Format,
    pub beta: bool,
    pub verbose: bool,
    pub store: AccountStore,
    pub config_dir: PathBuf,
}

impl CliContext {
    pub fn build(args: &crate::cli::Cli) -> anyhow::Result<Self> {
        let proj = directories::ProjectDirs::from("com", "mws", "mws").ok_or_else(|| anyhow::anyhow!("no config dir"))?;
        let config_dir = proj.config_dir().to_path_buf();
        std::fs::create_dir_all(&config_dir)?;
        let vault: Arc<dyn Vault> = Arc::new(DiskVault::new(config_dir.join("accounts"), "mws"));
        let store = AccountStore::new(vault);
        let format = match args.output.as_deref() {
            Some(s) => Format::parse(s).map_err(|e| anyhow::anyhow!(e.to_string()))?,
            None => {
                use std::io::IsTerminal;
                Format::auto(std::io::stdout().is_terminal())
            }
        };
        Ok(Self {
            account_name: args.account.clone().unwrap_or_else(|| "default".to_string()),
            tenant: args.tenant.clone().unwrap_or_else(|| mws_auth::DEFAULT_TENANT.to_string()),
            client_id: args.client_id.clone().unwrap_or_else(|| mws_auth::DEFAULT_CLIENT_ID.to_string()),
            format,
            beta: args.beta,
            verbose: args.verbose,
            store,
            config_dir,
        })
    }
}
