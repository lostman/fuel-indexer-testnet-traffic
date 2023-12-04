use fuel_core::service::config::Trigger;
use fuel_core_types::{
    fuel_crypto::SecretKey,
    fuel_tx::{Finalizable, Output, Script, Transaction, TransactionBuilder},
    fuel_types::AssetId,
};
use rand::{rngs::StdRng, Rng, SeedableRng};
use std::sync::Arc;
use test_helpers::builder::TestSetupBuilder;

// Use Jemalloc during benchmarks
#[global_allocator]
static GLOBAL: tikv_jemallocator::Jemalloc = tikv_jemallocator::Jemalloc;

async fn signed_transfers(num_blocks: usize, num_transactions: usize) {
    let generator = |rng: &mut StdRng| {
        TransactionBuilder::script(vec![], vec![])
            .script_gas_limit(10000)
            .gas_price(1)
            .add_unsigned_coin_input(
                SecretKey::random(rng),
                rng.gen(),
                1000,
                Default::default(),
                Default::default(),
                Default::default(),
            )
            .add_unsigned_coin_input(
                SecretKey::random(rng),
                rng.gen(),
                1000,
                Default::default(),
                Default::default(),
                Default::default(),
            )
            .add_output(Output::coin(rng.gen(), 50, AssetId::default()))
            .add_output(Output::change(rng.gen(), 0, AssetId::default()))
            .finalize()
    };
    bench_txs(num_blocks, num_transactions, generator).await;
}

async fn bench_txs<F>(num_blocks: usize, num_transactions: usize, f: F)
where
    F: Fn(&mut StdRng) -> Script,
{
    let mut rng = rand::rngs::StdRng::seed_from_u64(2322u64);

    let mut transactions = vec![];
    // Generate transactions for all future blocks upfront.
    println!("Generating {} transactions", num_blocks * num_transactions);
    for _ in 0..(num_blocks * num_transactions) {
        transactions.push(f(&mut rng));
    }

    let mut test_builder = TestSetupBuilder::new(2322);
    // Setup genesis block with coins that transactions can spend
    test_builder.config_coin_inputs_from_transactions(&transactions.iter().collect::<Vec<_>>());
    // Disable automated block production
    test_builder.trigger = Trigger::Never;
    test_builder.utxo_validation = true;

    let transactions: Vec<Transaction> = transactions.into_iter().map(|tx| tx.into()).collect();

    // Spin up node

    println!("Starting the producer node");
    let producer = test_builder.finalize().await;

    println!("Starting the validator node");
    let validator = test_builder.finalize().await;

    for (i, transactions) in transactions.chunks(num_transactions).enumerate() {
        let sealed_block = {
            let transactions = transactions.iter().map(|tx| Arc::new(tx.clone())).collect();

            // Insert all transactions
            producer.srv.shared.txpool.insert(transactions).await;
            let _ = producer.client.produce_blocks(1, None).await;

            // Sanity check block to ensure the transactions were actually processed
            println!("Checking block#{}", i + 1);
            let block = producer
                .srv
                .shared
                .database
                .get_sealed_block_by_height(&((i + 1) as u32).into())
                .unwrap()
                .unwrap();
            assert_eq!(
                block.entity.transactions().len(),
                (num_transactions + 1) as usize
            );
            block
        };

        validator
            .srv
            .shared
            .block_importer
            .execute_and_commit(sealed_block)
            .await
            .expect("Should validate the block");
    }
}

#[tokio::main]
async fn main() {
    signed_transfers(100, 10).await;
}
