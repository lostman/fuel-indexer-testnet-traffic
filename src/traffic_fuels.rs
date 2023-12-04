use fuels::prelude::*;
use fuels::programs::call_response::FuelCallResponse;

fuels::prelude::abigen!(Contract(
    name = "Subcurrency",
    abi = "contracts/subcurrency/out/debug/subcurrency-abi.json"
));

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    println!("Launching node and creating wallets");

    let wallet_config = WalletsConfig::new(Some(10), None, None);
    let mut provider_config = Config::default();
    provider_config.block_production = Trigger::Never;
    let wallets =
        launch_custom_provider_and_get_wallets(wallet_config, Some(provider_config), None).await?;
    let mint_wallet = wallets[0].clone();
    let wallets = wallets[1..].to_vec();

    let node = mint_wallet.provider().unwrap().clone();

    let configurables = SubcurrencyConfigurables::new().with_MINTER(mint_wallet.address().into());

    // Produce 1 block / 5 seconds
    tokio::task::spawn({
        let node = node.clone();
        async move {
            loop {
                tokio::time::sleep(std::time::Duration::from_secs(5)).await;
                let height = node.produce_blocks(1, None).await.unwrap();
                println!("Block #{height}");
            }
        }
    });

    println!("Deploying contract");

    let contract_id = Contract::load_from(
        "contracts/subcurrency/out/debug/subcurrency.bin",
        LoadConfiguration::default().with_configurables(configurables),
    )?
    .deploy(&mint_wallet, TxPolicies::default())
    .await?;

    let contract_instance = Subcurrency::new(contract_id.clone(), mint_wallet.clone());

    println!(
        "Block height: {}",
        node.latest_block_height().await.unwrap()
    );

    println!("Minting coins");

    contract_instance
        .methods()
        .mint(mint_wallet.address(), 300)
        .call()
        .await
        .unwrap();

    println!("Minting more coins...");

    for i in 0..wallets.len() {
        println!("Minting 100 coins to {}", wallets[i].address());
        contract_instance
            .methods()
            .mint(wallets[i].address(), 100)
            .call()
            .await
            .unwrap();
    }

    println!(
        "Block height: {}",
        node.latest_block_height().await.unwrap()
    );

    println!("Generating transfers");

    for w in wallets.iter() {
        let contract_instance = Subcurrency::new(contract_id.clone(), w.clone());
        println!(
            "Sending 3 coins to from {} to {}",
            w.address(),
            mint_wallet.address()
        );
        contract_instance
            .methods()
            .send(mint_wallet.address(), 3)
            .call()
            .await
            .unwrap();
    }

    println!("Checking balances");

    for w in wallets.iter() {
        let contract_instance = Subcurrency::new(contract_id.clone(), w.clone());
        let balance: FuelCallResponse<u64> = contract_instance
            .methods()
            .balance(w.address())
            .call()
            .await
            .unwrap();
        println!("Balance {} {}", w.address(), balance.value);
    }

    // let r: FuelCallResponse<((), ())> = multi_call_handler.call().await.unwrap();
    // println!("{:#?}", r.receipts);

    let chain_info = node.chain_info().await.unwrap();

    println!("Chain Info:");
    println!("{chain_info:#?}");

    println!("Done.");

    Ok(())
}
