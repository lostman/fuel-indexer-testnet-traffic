use fuels::prelude::*;

fuels::prelude::abigen!(Contract(
    name = "Subcurrency",
    abi = "contracts/subcurrency/out/debug/subcurrency-abi.json"
));

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    println!("> Launching node and creating wallets");

    let wallet_config = WalletsConfig::new(Some(10), Some(10), None);
    let mut provider_config = Config::default();
    provider_config.block_production = Trigger::Never;
    let wallets =
        launch_custom_provider_and_get_wallets(wallet_config, Some(provider_config), None).await?;
    let mint_wallet = wallets[0].clone();

    let node = mint_wallet.provider().unwrap().clone();

    let configurables = SubcurrencyConfigurables::new().with_MINTER(mint_wallet.address().into());

    println!("> Deploying contract");

    let contract_id = Contract::load_from(
        "contracts/subcurrency/out/debug/subcurrency.bin",
        LoadConfiguration::default().with_configurables(configurables),
    )?
    .deploy(&mint_wallet, TxPolicies::default());

    let block = async {
        tokio::time::sleep(std::time::Duration::from_secs(1)).await;
        node.produce_blocks(1, None).await.unwrap();
    };

    let (contract_id, _) = tokio::join!(contract_id, block);
    let contract_id = contract_id.unwrap();

    let contract_instance = Subcurrency::new(contract_id.clone(), mint_wallet.clone());

    println!(
        "> Block height: {}",
        node.latest_block_height().await.unwrap()
    );

    println!("> Minting coins");

    let mut results = vec![];
    for i in 0..wallets.len() {
        println!("Minting 100 coins to {}", wallets[i].address());
        let resp = contract_instance
            .methods()
            .mint(wallets[i].address(), 100)
            .submit()
            .await
            .unwrap();
        results.push(resp)
    }

    node.produce_blocks(1, None).await.unwrap();

    // Check the responses
    for r in results {
        r.response().await.unwrap();
    }

    println!(
        "> Block height: {}",
        node.latest_block_height().await.unwrap()
    );

    println!("> Generating transfers");

    let mut results = vec![];
    for w in wallets.iter() {
        let contract_instance = Subcurrency::new(contract_id.clone(), w.clone());
        println!(
            "Sending 3 coins to from {} to {}",
            w.address(),
            mint_wallet.address()
        );
        let resp = contract_instance
            .methods()
            .send(mint_wallet.address(), 3)
            .submit()
            .await
            .unwrap();
        results.push(resp);
    }

    node.produce_blocks(1, None).await.unwrap();

    // Check the responses
    for r in results {
        r.response().await.unwrap();
    }

    println!(
        "> Block height: {}",
        node.latest_block_height().await.unwrap()
    );

    println!("> Checking balances");

    for w in wallets.iter() {
        let contract_instance = Subcurrency::new(contract_id.clone(), w.clone());
        let resp = contract_instance
            .methods()
            .balance(w.address())
            .simulate()
            .await
            .unwrap();
        println!("Balance {} {}", w.address(), resp.value);
    }

    let chain_info = node.chain_info().await.unwrap();

    println!("> Latest Block:");
    println!("{:#?}", chain_info.latest_block);

    println!("X Done.");

    Ok(())
}
