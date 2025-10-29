use esport::Operation;
use linera_sdk::test::{QueryOutcome, TestValidator};
use linera_sdk::linera_base_types::AccountOwner;

#[tokio::test(flavor = "multi_thread")]
async fn multiple_matches_increment_next_match_id() {
    let (validator, module_id) =
        TestValidator::with_current_module::<esport::DiceAbi, (), u64>().await;
    let mut chain = validator.new_chain().await;

    let initial_state = 0u64;
    let application_id = chain
        .create_application(module_id, (), initial_state, vec![])
        .await;

    // Start two matches in two separate blocks.
    chain
        .add_block(|block| {
            block.with_operation(
                application_id,
                Operation::StartMatch {
                    players: [AccountOwner::Reserved(1), AccountOwner::Reserved(2)],
                    rounds: 3,
                },
            );
        })
        .await;

    chain
        .add_block(|block| {
            block.with_operation(
                application_id,
                Operation::StartMatch {
                    players: [AccountOwner::Reserved(3), AccountOwner::Reserved(4)],
                    rounds: 2,
                },
            );
        })
        .await;

    // Query nextMatchId (should have advanced twice)
    let QueryOutcome { response, .. } =
        chain.graphql_query(application_id, "query { nextMatchId }").await;
    let next_match_id = response["nextMatchId"].as_u64().expect("Failed to get u64");

    assert_eq!(next_match_id, 2);
}

#[tokio::test(flavor = "multi_thread")]
async fn multiple_starts_in_single_block_apply() {
    let (validator, module_id) =
        TestValidator::with_current_module::<esport::DiceAbi, (), u64>().await;
    let mut chain = validator.new_chain().await;

    let initial_state = 0u64;
    let application_id = chain
        .create_application(module_id, (), initial_state, vec![])
        .await;

    // Two starts in the same block should both apply (nextMatchId increments twice).
    chain
        .add_block(|block| {
            block.with_operation(
                application_id,
                Operation::StartMatch {
                    players: [AccountOwner::Reserved(1), AccountOwner::Reserved(2)],
                    rounds: 1,
                },
            );
            block.with_operation(
                application_id,
                Operation::StartMatch {
                    players: [AccountOwner::Reserved(3), AccountOwner::Reserved(4)],
                    rounds: 1,
                },
            );
        })
        .await;

    let QueryOutcome { response, .. } =
        chain.graphql_query(application_id, "query { nextMatchId }").await;
    let next_match_id = response["nextMatchId"].as_u64().expect("Failed to get u64");

    assert_eq!(next_match_id, 2);
}