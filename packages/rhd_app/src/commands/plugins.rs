//! Plugin operations command

use rhd_chat_client::ChatClient;
use rhd_chat_api::{GetPluginsParams, RemovePluginParams};
use crate::cli::PluginsSubcommand;

pub async fn execute(subcommand: PluginsSubcommand) -> Result<(), Box<dyn std::error::Error>> {
    let client = ChatClient::connect("ws://127.0.0.1:8080/").await?;
    
    match subcommand {
        PluginsSubcommand::List => {
            let result = client.get_plugins(GetPluginsParams {}).await?;
            println!("{}", serde_json::to_string_pretty(&result.plugins)?);
            Ok(())
        }
        PluginsSubcommand::Remove { plugin_id } => {
            client.remove_plugin(RemovePluginParams { plugin_id }).await?;
            println!("Plugin removed successfully");
            Ok(())
        }
    }
}
