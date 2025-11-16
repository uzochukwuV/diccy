use battle_token::{BattleTokenAbi, Message, Operation, BattleTokenContract};
use linera_sdk::{
    base::{Amount, Owner},
    test::{ActiveChain, TestValidator},
    QueryOutcome,
};

/// Test basic token transfer
#[tokio::test(flavor = "multi_thread")]
async fn test_token_transfer() {
    // Setup validator and deploy token
    let initial_supply = Amount::from_tokens(1_000_000_000); // 1 billion BATTLE
    let (validator, module_id) =
        TestValidator::with_current_module::<BattleTokenAbi, Amount, ()>().await;

    let mut chain = validator.new_chain().await;
    let owner = chain.owner();

    // Create token application
    let application_id = chain
        .create_application(module_id, initial_supply, (), vec![])
        .await;

    // Query initial balance
    let QueryOutcome { response, .. } = chain
        .graphql_query(
            application_id,
            "query { stats { totalSupply totalHolders } }",
        )
        .await;

    let total_supply = response["stats"]["totalSupply"]
        .as_str()
        .expect("Failed to get total supply");
    assert_eq!(total_supply, initial_supply.to_string());

    // Create recipient
    let recipient = Owner::from([2u8; 32]);
    let transfer_amount = Amount::from_tokens(100); // Transfer 100 BATTLE

    // Execute transfer
    chain
        .add_block(|block| {
            block.with_operation(
                application_id,
                Operation::Transfer {
                    to: recipient,
                    amount: transfer_amount,
                },
            );
        })
        .await;

    // Verify transfer stats updated
    let QueryOutcome { response, .. } = chain
        .graphql_query(
            application_id,
            "query { stats { totalHolders totalTransfers } }",
        )
        .await;

    let total_holders = response["stats"]["totalHolders"]
        .as_u64()
        .expect("Failed to get total holders");
    let total_transfers = response["stats"]["totalTransfers"]
        .as_u64()
        .expect("Failed to get total transfers");

    assert_eq!(total_holders, 2); // Owner + recipient
    assert_eq!(total_transfers, 1);
}

/// Test insufficient balance error
#[tokio::test(flavor = "multi_thread")]
async fn test_insufficient_balance() {
    let initial_supply = Amount::from_tokens(1_000);
    let (validator, module_id) =
        TestValidator::with_current_module::<BattleTokenAbi, Amount, ()>().await;

    let mut chain = validator.new_chain().await;

    let application_id = chain
        .create_application(module_id, initial_supply, (), vec![])
        .await;

    let recipient = Owner::from([2u8; 32]);
    let excessive_amount = Amount::from_tokens(10_000); // More than balance

    // Attempt transfer with insufficient balance
    // This should NOT panic the application, but log an error
    chain
        .add_block(|block| {
            block.with_operation(
                application_id,
                Operation::Transfer {
                    to: recipient,
                    amount: excessive_amount,
                },
            );
        })
        .await;

    // Verify no transfer occurred (still 1 holder, 0 transfers)
    let QueryOutcome { response, .. } = chain
        .graphql_query(
            application_id,
            "query { stats { totalHolders totalTransfers } }",
        )
        .await;

    let total_transfers = response["stats"]["totalTransfers"]
        .as_u64()
        .expect("Failed to get total transfers");

    assert_eq!(total_transfers, 0); // Transfer failed
}

/// Test approve and transferFrom
#[tokio::test(flavor = "multi_thread")]
async fn test_approve_and_transfer_from() {
    let initial_supply = Amount::from_tokens(1_000_000);
    let (validator, module_id) =
        TestValidator::with_current_module::<BattleTokenAbi, Amount, ()>().await;

    let mut chain_owner = validator.new_chain().await;
    let owner = chain_owner.owner();

    let application_id = chain_owner
        .create_application(module_id, initial_supply, (), vec![])
        .await;

    // Create spender chain
    let mut chain_spender = validator.new_chain().await;
    let spender = chain_spender.owner();

    // Create recipient
    let recipient = Owner::from([3u8; 32]);
    let allowance_amount = Amount::from_tokens(500);
    let transfer_amount = Amount::from_tokens(200);

    // Owner approves spender
    chain_owner
        .add_block(|block| {
            block.with_operation(
                application_id,
                Operation::Approve {
                    spender,
                    amount: allowance_amount,
                },
            );
        })
        .await;

    // Spender transfers from owner to recipient
    chain_spender
        .add_block(|block| {
            block.with_operation(
                application_id,
                Operation::TransferFrom {
                    from: owner,
                    to: recipient,
                    amount: transfer_amount,
                },
            );
        })
        .await;

    // Verify transfer occurred
    let QueryOutcome { response, .. } = chain_owner
        .graphql_query(
            application_id,
            "query { stats { totalHolders totalTransfers } }",
        )
        .await;

    let total_transfers = response["stats"]["totalTransfers"]
        .as_u64()
        .expect("Failed to get total transfers");

    assert_eq!(total_transfers, 1); // TransferFrom succeeded
}

/// Test token burning
#[tokio::test(flavor = "multi_thread")]
async fn test_burn_tokens() {
    let initial_supply = Amount::from_tokens(1_000_000);
    let (validator, module_id) =
        TestValidator::with_current_module::<BattleTokenAbi, Amount, ()>().await;

    let mut chain = validator.new_chain().await;

    let application_id = chain
        .create_application(module_id, initial_supply, (), vec![])
        .await;

    let burn_amount = Amount::from_tokens(100_000);

    // Burn tokens
    chain
        .add_block(|block| {
            block.with_operation(
                application_id,
                Operation::Burn {
                    amount: burn_amount,
                },
            );
        })
        .await;

    // Verify total burned and circulating supply
    let QueryOutcome { response, .. } = chain
        .graphql_query(
            application_id,
            "query { stats { totalSupply totalBurned circulatingSupply } }",
        )
        .await;

    let total_supply = response["stats"]["totalSupply"]
        .as_str()
        .expect("Failed to get total supply");
    let total_burned = response["stats"]["totalBurned"]
        .as_str()
        .expect("Failed to get total burned");
    let circulating = response["stats"]["circulatingSupply"]
        .as_str()
        .expect("Failed to get circulating supply");

    assert_eq!(total_supply, initial_supply.to_string());
    assert_eq!(total_burned, burn_amount.to_string());
    assert_eq!(circulating, (initial_supply - burn_amount).to_string());
}

/// Test multiple transfers and holder tracking
#[tokio::test(flavor = "multi_thread")]
async fn test_multiple_transfers() {
    let initial_supply = Amount::from_tokens(10_000);
    let (validator, module_id) =
        TestValidator::with_current_module::<BattleTokenAbi, Amount, ()>().await;

    let mut chain = validator.new_chain().await;

    let application_id = chain
        .create_application(module_id, initial_supply, (), vec![])
        .await;

    // Create multiple recipients
    let recipient1 = Owner::from([2u8; 32]);
    let recipient2 = Owner::from([3u8; 32]);
    let recipient3 = Owner::from([4u8; 32]);

    // Transfer to recipient1
    chain
        .add_block(|block| {
            block.with_operation(
                application_id,
                Operation::Transfer {
                    to: recipient1,
                    amount: Amount::from_tokens(1000),
                },
            );
        })
        .await;

    // Transfer to recipient2
    chain
        .add_block(|block| {
            block.with_operation(
                application_id,
                Operation::Transfer {
                    to: recipient2,
                    amount: Amount::from_tokens(2000),
                },
            );
        })
        .await;

    // Transfer to recipient3
    chain
        .add_block(|block| {
            block.with_operation(
                application_id,
                Operation::Transfer {
                    to: recipient3,
                    amount: Amount::from_tokens(3000),
                },
            );
        })
        .await;

    // Verify stats
    let QueryOutcome { response, .. } = chain
        .graphql_query(
            application_id,
            "query { stats { totalHolders totalTransfers } }",
        )
        .await;

    let total_holders = response["stats"]["totalHolders"]
        .as_u64()
        .expect("Failed to get total holders");
    let total_transfers = response["stats"]["totalTransfers"]
        .as_u64()
        .expect("Failed to get total transfers");

    assert_eq!(total_holders, 4); // Owner + 3 recipients
    assert_eq!(total_transfers, 3);
}

/// Test zero amount transfer rejection
#[tokio::test(flavor = "multi_thread")]
async fn test_zero_amount_transfer() {
    let initial_supply = Amount::from_tokens(1_000);
    let (validator, module_id) =
        TestValidator::with_current_module::<BattleTokenAbi, Amount, ()>().await;

    let mut chain = validator.new_chain().await;

    let application_id = chain
        .create_application(module_id, initial_supply, (), vec![])
        .await;

    let recipient = Owner::from([2u8; 32]);

    // Attempt zero amount transfer
    chain
        .add_block(|block| {
            block.with_operation(
                application_id,
                Operation::Transfer {
                    to: recipient,
                    amount: Amount::ZERO,
                },
            );
        })
        .await;

    // Verify no transfer occurred
    let QueryOutcome { response, .. } = chain
        .graphql_query(
            application_id,
            "query { stats { totalTransfers } }",
        )
        .await;

    let total_transfers = response["stats"]["totalTransfers"]
        .as_u64()
        .expect("Failed to get total transfers");

    assert_eq!(total_transfers, 0); // Zero amount rejected
}

/// Test self-transfer rejection
#[tokio::test(flavor = "multi_thread")]
async fn test_self_transfer() {
    let initial_supply = Amount::from_tokens(1_000);
    let (validator, module_id) =
        TestValidator::with_current_module::<BattleTokenAbi, Amount, ()>().await;

    let mut chain = validator.new_chain().await;
    let owner = chain.owner();

    let application_id = chain
        .create_application(module_id, initial_supply, (), vec![])
        .await;

    // Attempt self-transfer
    chain
        .add_block(|block| {
            block.with_operation(
                application_id,
                Operation::Transfer {
                    to: owner,
                    amount: Amount::from_tokens(100),
                },
            );
        })
        .await;

    // Verify no transfer occurred
    let QueryOutcome { response, .. } = chain
        .graphql_query(
            application_id,
            "query { stats { totalTransfers } }",
        )
        .await;

    let total_transfers = response["stats"]["totalTransfers"]
        .as_u64()
        .expect("Failed to get total transfers");

    assert_eq!(total_transfers, 0); // Self-transfer rejected
}

/// Test allowance deduction on transferFrom
#[tokio::test(flavor = "multi_thread")]
async fn test_allowance_deduction() {
    let initial_supply = Amount::from_tokens(1_000_000);
    let (validator, module_id) =
        TestValidator::with_current_module::<BattleTokenAbi, Amount, ()>().await;

    let mut chain_owner = validator.new_chain().await;
    let owner = chain_owner.owner();

    let application_id = chain_owner
        .create_application(module_id, initial_supply, (), vec![])
        .await;

    let mut chain_spender = validator.new_chain().await;
    let spender = chain_spender.owner();

    let recipient = Owner::from([3u8; 32]);
    let allowance = Amount::from_tokens(1000);
    let first_transfer = Amount::from_tokens(400);
    let second_transfer = Amount::from_tokens(700); // This exceeds remaining allowance (600)

    // Approve spender
    chain_owner
        .add_block(|block| {
            block.with_operation(
                application_id,
                Operation::Approve {
                    spender,
                    amount: allowance,
                },
            );
        })
        .await;

    // First transferFrom (400 BATTLE)
    chain_spender
        .add_block(|block| {
            block.with_operation(
                application_id,
                Operation::TransferFrom {
                    from: owner,
                    to: recipient,
                    amount: first_transfer,
                },
            );
        })
        .await;

    // Second transferFrom (700 BATTLE) - should fail
    chain_spender
        .add_block(|block| {
            block.with_operation(
                application_id,
                Operation::TransferFrom {
                    from: owner,
                    to: recipient,
                    amount: second_transfer,
                },
            );
        })
        .await;

    // Verify only one transfer succeeded
    let QueryOutcome { response, .. } = chain_owner
        .graphql_query(
            application_id,
            "query { stats { totalTransfers } }",
        )
        .await;

    let total_transfers = response["stats"]["totalTransfers"]
        .as_u64()
        .expect("Failed to get total transfers");

    assert_eq!(total_transfers, 1); // Only first transfer succeeded
}

/// Test token minting (admin operation)
#[tokio::test(flavor = "multi_thread")]
async fn test_mint_tokens() {
    let initial_supply = Amount::from_tokens(1_000_000);
    let (validator, module_id) =
        TestValidator::with_current_module::<BattleTokenAbi, Amount, ()>().await;

    let mut chain = validator.new_chain().await;

    let application_id = chain
        .create_application(module_id, initial_supply, (), vec![])
        .await;

    let recipient = Owner::from([2u8; 32]);
    let mint_amount = Amount::from_tokens(500_000);

    // Mint tokens
    chain
        .add_block(|block| {
            block.with_operation(
                application_id,
                Operation::Mint {
                    to: recipient,
                    amount: mint_amount,
                },
            );
        })
        .await;

    // Verify total supply increased
    let QueryOutcome { response, .. } = chain
        .graphql_query(
            application_id,
            "query { stats { totalSupply totalHolders } }",
        )
        .await;

    let total_supply = response["stats"]["totalSupply"]
        .as_str()
        .expect("Failed to get total supply");
    let total_holders = response["stats"]["totalHolders"]
        .as_u64()
        .expect("Failed to get total holders");

    let expected_supply = initial_supply + mint_amount;
    assert_eq!(total_supply, expected_supply.to_string());
    assert_eq!(total_holders, 2); // Owner + recipient
}

/// Test high-volume transfers
#[tokio::test(flavor = "multi_thread")]
async fn test_high_volume_transfers() {
    let initial_supply = Amount::from_tokens(1_000_000_000); // 1 billion
    let (validator, module_id) =
        TestValidator::with_current_module::<BattleTokenAbi, Amount, ()>().await;

    let mut chain = validator.new_chain().await;

    let application_id = chain
        .create_application(module_id, initial_supply, (), vec![])
        .await;

    // Perform 10 transfers
    for i in 0..10 {
        let recipient = Owner::from([i + 2; 32]);
        chain
            .add_block(|block| {
                block.with_operation(
                    application_id,
                    Operation::Transfer {
                        to: recipient,
                        amount: Amount::from_tokens(1000),
                    },
                );
            })
            .await;
    }

    // Verify all transfers succeeded
    let QueryOutcome { response, .. } = chain
        .graphql_query(
            application_id,
            "query { stats { totalTransfers totalHolders } }",
        )
        .await;

    let total_transfers = response["stats"]["totalTransfers"]
        .as_u64()
        .expect("Failed to get total transfers");
    let total_holders = response["stats"]["totalHolders"]
        .as_u64()
        .expect("Failed to get total holders");

    assert_eq!(total_transfers, 10);
    assert_eq!(total_holders, 11); // Owner + 10 recipients
}
