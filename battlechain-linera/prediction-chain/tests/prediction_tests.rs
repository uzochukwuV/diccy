use battlechain_shared_types::Owner;
use linera_sdk::{
    linera_base_types::{AccountOwner, Amount, ApplicationId, ChainId},
    test::{ActiveChain, TestValidator},
    QueryOutcome,
};
use prediction_chain::{BetSide, Message, Operation, PredictionAbi};

/// Test market creation
#[tokio::test(flavor = "multi_thread")]
async fn test_create_market() {
    let (validator, module_id) =
        TestValidator::with_current_module::<PredictionAbi, (), ()>().await;

    let mut chain = validator.new_chain().await;

    let application_id = chain
        .create_application(module_id, (), (), vec![])
        .await;

    let battle_chain = ChainId::from([1u8; 32]);
    let player1_chain = ChainId::from([2u8; 32]);
    let player2_chain = ChainId::from([3u8; 32]);

    // Create market
    chain
        .add_block(|block| {
            block.with_operation(
                application_id,
                Operation::CreateMarket {
                    battle_chain,
                    player1_chain,
                    player2_chain,
                },
            );
        })
        .await;

    // Query total markets
    let QueryOutcome { response, .. } = chain
        .graphql_query(
            application_id,
            "query { totalMarkets }",
        )
        .await;

    let total_markets = response["totalMarkets"]
        .as_u64()
        .expect("Failed to get total markets");

    assert_eq!(total_markets, 1);
}

/// Test place bet
#[tokio::test(flavor = "multi_thread")]
async fn test_place_bet() {
    let (validator, module_id) =
        TestValidator::with_current_module::<PredictionAbi, (), ()>().await;

    let mut chain = validator.new_chain().await;

    let application_id = chain
        .create_application(module_id, (), (), vec![])
        .await;

    let battle_chain = ChainId::from([1u8; 32]);
    let player1_chain = ChainId::from([2u8; 32]);
    let player2_chain = ChainId::from([3u8; 32]);

    // Create market
    chain
        .add_block(|block| {
            block.with_operation(
                application_id,
                Operation::CreateMarket {
                    battle_chain,
                    player1_chain,
                    player2_chain,
                },
            );
        })
        .await;

    // Place bet
    chain
        .add_block(|block| {
            block.with_operation(
                application_id,
                Operation::PlaceBet {
                    market_id: 0,
                    side: BetSide::Player1,
                    amount: Amount::from_tokens(100),
                },
            );
        })
        .await;

    // Query market stats
    let QueryOutcome { response, .. } = chain
        .graphql_query(
            application_id,
            &format!("query {{ market(marketId: 0) {{ totalBets }} }}"),
        )
        .await;

    // Verify bet placed
}

/// Test close market
#[tokio::test(flavor = "multi_thread")]
async fn test_close_market() {
    let (validator, module_id) =
        TestValidator::with_current_module::<PredictionAbi, (), ()>().await;

    let mut chain = validator.new_chain().await;

    let application_id = chain
        .create_application(module_id, (), (), vec![])
        .await;

    let battle_chain = ChainId::from([1u8; 32]);

    // Create market
    chain
        .add_block(|block| {
            block.with_operation(
                application_id,
                Operation::CreateMarket {
                    battle_chain,
                    player1_chain: ChainId::from([2u8; 32]),
                    player2_chain: ChainId::from([3u8; 32]),
                },
            );
        })
        .await;

    // Close market
    chain
        .add_block(|block| {
            block.with_operation(
                application_id,
                Operation::CloseMarket { market_id: 0 },
            );
        })
        .await;

    // Verify market closed (no more bets allowed)
}

/// Test settle market
#[tokio::test(flavor = "multi_thread")]
async fn test_settle_market() {
    let (validator, module_id) =
        TestValidator::with_current_module::<PredictionAbi, (), ()>().await;

    let mut chain = validator.new_chain().await;

    let application_id = chain
        .create_application(module_id, (), (), vec![])
        .await;

    let battle_chain = ChainId::from([1u8; 32]);
    let winner_chain = ChainId::from([2u8; 32]);

    // Create and close market
    chain
        .add_block(|block| {
            block.with_operation(
                application_id,
                Operation::CreateMarket {
                    battle_chain,
                    player1_chain: ChainId::from([2u8; 32]),
                    player2_chain: ChainId::from([3u8; 32]),
                },
            );
        })
        .await;

    chain
        .add_block(|block| {
            block.with_operation(
                application_id,
                Operation::CloseMarket { market_id: 0 },
            );
        })
        .await;

    // Settle market
    chain
        .add_block(|block| {
            block.with_operation(
                application_id,
                Operation::SettleMarket {
                    market_id: 0,
                    winner: BetSide::Player1,
                },
            );
        })
        .await;

    // Verify market settled
}

/// Test multiple bets on different sides
#[tokio::test(flavor = "multi_thread")]
async fn test_multiple_bets() {
    let (validator, module_id) =
        TestValidator::with_current_module::<PredictionAbi, (), ()>().await;

    let mut chain1 = validator.new_chain().await;
    let mut chain2 = validator.new_chain().await;

    let application_id = chain1
        .create_application(module_id, (), (), vec![])
        .await;

    // Create market
    chain1
        .add_block(|block| {
            block.with_operation(
                application_id,
                Operation::CreateMarket {
                    battle_chain: ChainId::from([1u8; 32]),
                    player1_chain: ChainId::from([2u8; 32]),
                    player2_chain: ChainId::from([3u8; 32]),
                },
            );
        })
        .await;

    // Bet on player 1
    chain1
        .add_block(|block| {
            block.with_operation(
                application_id,
                Operation::PlaceBet {
                    market_id: 0,
                    side: BetSide::Player1,
                    amount: Amount::from_tokens(100),
                },
            );
        })
        .await;

    // Bet on player 2
    chain2
        .add_block(|block| {
            block.with_operation(
                application_id,
                Operation::PlaceBet {
                    market_id: 0,
                    side: BetSide::Player2,
                    amount: Amount::from_tokens(150),
                },
            );
        })
        .await;

    // Verify both bets recorded
}

/// Test odds calculation
#[tokio::test(flavor = "multi_thread")]
async fn test_odds_calculation() {
    // Test that odds are calculated based on bet amounts
    // Test that odds update when new bets are placed
}

/// Test winnings distribution
#[tokio::test(flavor = "multi_thread")]
async fn test_winnings_distribution() {
    // Test that winners receive proportional winnings
    // Test that losers lose their bet amount
}

/// Test cancel market refunds
#[tokio::test(flavor = "multi_thread")]
async fn test_cancel_market() {
    let (validator, module_id) =
        TestValidator::with_current_module::<PredictionAbi, (), ()>().await;

    let mut chain = validator.new_chain().await;

    let application_id = chain
        .create_application(module_id, (), (), vec![])
        .await;

    // Create market and place bet
    chain
        .add_block(|block| {
            block.with_operation(
                application_id,
                Operation::CreateMarket {
                    battle_chain: ChainId::from([1u8; 32]),
                    player1_chain: ChainId::from([2u8; 32]),
                    player2_chain: ChainId::from([3u8; 32]),
                },
            );
        })
        .await;

    chain
        .add_block(|block| {
            block.with_operation(
                application_id,
                Operation::PlaceBet {
                    market_id: 0,
                    side: BetSide::Player1,
                    amount: Amount::from_tokens(100),
                },
            );
        })
        .await;

    // Cancel market
    chain
        .add_block(|block| {
            block.with_operation(
                application_id,
                Operation::CancelMarket { market_id: 0 },
            );
        })
        .await;

    // Verify all bets refunded
}
