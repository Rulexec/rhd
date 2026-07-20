mod simple;
mod tools;

use reqwest::Client;


pub(crate) struct StreamExecutor {
    pub(crate) base_url: String,
    pub(crate) api_key: String,
    pub(crate) http: Client,
}
