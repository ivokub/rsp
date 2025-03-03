#![warn(unused_crate_dependencies)]

use alloy_provider::{Network, RootProvider};
use clap::Parser;
use op_alloy_network::Ethereum;
use reth_evm::execute::BlockExecutionStrategyFactory;
use reth_primitives::NodePrimitives;
use rsp_client_executor::{io::ClientExecutorInput, IntoInput, IntoPrimitives};
use rsp_host_executor::{create_eth_block_execution_strategy_factory, HostExecutor};
use rsp_primitives::genesis::Genesis;
use rsp_rpc_db::RpcDb;
use serde::de::DeserializeOwned;
use std::path::PathBuf;
use tracing;
use tracing_subscriber::{
    filter::EnvFilter, fmt, prelude::__tracing_subscriber_SubscriberExt, util::SubscriberInitExt,
};

mod cli;
use cli::ProviderArgs;

/// The arguments for the host executable.
#[derive(Debug, Clone, Parser)]
struct CachemakerArgs {
    /// The block number of the block to execute.
    #[clap(long)]
    block_number: u64,

    // How many blocks to cache
    #[clap(long)]
    block_count: u64,

    #[clap(flatten)]
    provider: ProviderArgs,

    /// Optional path to the directory containing cached client input. A new cache file will be
    /// created from RPC data if it doesn't already exist.
    #[clap(long)]
    cache_dir: Option<PathBuf>,
}

#[tokio::main]
async fn main() -> eyre::Result<()> {
    // Initialize the environment variables.
    dotenv::dotenv().ok();

    if std::env::var("RUST_LOG").is_err() {
        std::env::set_var("RUST_LOG", "info");
    }

    // Initialize the logger.
    tracing_subscriber::registry().with(fmt::layer()).with(EnvFilter::from_default_env()).init();

    // Parse the command line arguments.
    let args = CachemakerArgs::parse();
    let provider_config = args.provider.clone().into_provider().await?;
    let rpc_url = provider_config.rpc_url.unwrap();
    let chain_id = provider_config.chain_id;

    let genesis = provider_config.chain_id.try_into()?;
    let block_execution_strategy_factory =
        create_eth_block_execution_strategy_factory(&genesis, None);

    let cache_dir = args.cache_dir.unwrap();
    let block_numbers: Vec<u64> =
        (args.block_number..args.block_number + args.block_count).collect();

    let futures = block_numbers.into_iter().map(|block_number| {
        execute::<Ethereum, _, _>(
            rpc_url.clone(),
            chain_id,
            genesis.clone(),
            block_execution_strategy_factory.clone(),
            &cache_dir,
            block_number,
        )
    });

    futures::future::try_join_all(futures).await?;

    Ok(())
}

async fn execute<N, NP, F>(
    // provider_config: ProviderConfig,
    rpc_url: url::Url,
    chain_id: u64,
    genesis: Genesis,
    block_execution_strategy_factory: F,
    cache_dir: &PathBuf,
    block_number: u64,
) -> eyre::Result<()>
where
    N: Network,
    NP: NodePrimitives + DeserializeOwned,
    F: BlockExecutionStrategyFactory<Primitives = NP>,
    F::Primitives: IntoPrimitives<N> + IntoInput,
{
    let client_input_from_cache =
        try_load_input_from_cache::<NP>(cache_dir.to_path_buf(), chain_id, block_number)?;

    if client_input_from_cache.is_none() {
        tracing::info!("Cache {} does not exist", block_number);
        let provider = RootProvider::<N>::new_http(rpc_url);

        // Setup the host executor.
        let host_executor = HostExecutor::new(block_execution_strategy_factory);

        let rpc_db = RpcDb::new(provider.clone(), block_number - 1);

        // Execute the host.
        let client_input =
            host_executor.execute(block_number, &rpc_db, &provider, genesis, None).await?;

        let input_folder = cache_dir.join(format!("input/{}", chain_id));
        if !input_folder.exists() {
            std::fs::create_dir_all(&input_folder)?;
        }

        let input_path = input_folder.join(format!("{}.bin", block_number));
        let mut cache_file = std::fs::File::create(input_path)?;

        bincode::serialize_into(&mut cache_file, &client_input)?;
    } else {
        tracing::info!("Cache {} exists", block_number);
    }

    Ok(())
}

fn try_load_input_from_cache<P: NodePrimitives + DeserializeOwned>(
    cache_dir: PathBuf,
    chain_id: u64,
    block_number: u64,
) -> eyre::Result<Option<ClientExecutorInput<P>>> {
    Ok({
        let cache_path = cache_dir.join(format!("input/{}/{}.bin", chain_id, block_number));

        if cache_path.exists() {
            // TODO: prune the cache if invalid instead
            let mut cache_file = std::fs::File::open(cache_path)?;
            let client_input: ClientExecutorInput<P> = bincode::deserialize_from(&mut cache_file)?;

            Some(client_input)
        } else {
            None
        }
    })
}
