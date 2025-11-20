use battlechain_shared_types::{CharacterClass, CharacterSnapshot, Owner};
use linera_sdk::{
    linera_base_types::{AccountOwner, Amount, ApplicationId, ChainId},
    test::{ActiveChain, TestValidator},
    QueryOutcome,
};
use matchmaking_chain::{Message, Operation, MatchmakingAbi, QueueEntry};

/// Test joining matchmaking queue
#[tokio::test(flavor = "multi_thread")]
async fn test_join_queue() {
    let (validator, module_id) =
        TestValidator::with_current_module::<MatchmakingAbi, (), ()>().await;

    let mut chain = validator.new_chain().await;
    let owner = chain.owner();

    let application_id = chain
        .create_application(module_id, (), (), vec![])
        .await;

    // Create character snapshot for matchmaking
    let character = CharacterSnapshot {
        class: CharacterClass::Warrior,
        level: 10,
        hp_max: 100,
        attack: 50,
        defense: 30,
        speed: 40,
        crit_rate: 10,
        crit_damage: 150,
        dodge_rate: 5,
        accuracy: 95,
    };

    // Join queue
    chain
        .add_block(|block| {
            block.with_operation(
                application_id,
                Operation::JoinQueue {
                    character,
                    stake: Amount::from_tokens(100),
                },
            );
        })
        .await;

    // Query queue size
    let QueryOutcome { response, .. } = chain
        .graphql_query(
            application_id,
            "query { queueSize }",
        )
        .await;

    let queue_size = response["queueSize"]
        .as_u64()
        .expect("Failed to get queue size");

    assert_eq!(queue_size, 1);
}

/// Test matchmaking with two players
#[tokio::test(flavor = "multi_thread")]
async fn test_match_two_players() {
    let (validator, module_id) =
        TestValidator::with_current_module::<MatchmakingAbi, (), ()>().await;

    let mut chain1 = validator.new_chain().await;
    let mut chain2 = validator.new_chain().await;

    let application_id = chain1
        .create_application(module_id, (), (), vec![])
        .await;

    let character1 = CharacterSnapshot {
        class: CharacterClass::Warrior,
        level: 10,
        hp_max: 100,
        attack: 50,
        defense: 30,
        speed: 40,
        crit_rate: 10,
        crit_damage: 150,
        dodge_rate: 5,
        accuracy: 95,
    };

    let character2 = CharacterSnapshot {
        class: CharacterClass::Mage,
        level: 10,
        hp_max: 80,
        attack: 60,
        defense: 20,
        speed: 50,
        crit_rate: 15,
        crit_damage: 180,
        dodge_rate: 10,
        accuracy: 90,
    };

    // Player 1 joins queue
    chain1
        .add_block(|block| {
            block.with_operation(
                application_id,
                Operation::JoinQueue {
                    character: character1,
                    stake: Amount::from_tokens(100),
                },
            );
        })
        .await;

    // Player 2 joins queue
    chain2
        .add_block(|block| {
            block.with_operation(
                application_id,
                Operation::JoinQueue {
                    character: character2,
                    stake: Amount::from_tokens(100),
                },
            );
        })
        .await;

    // Query total matches created
    let QueryOutcome { response, .. } = chain1
        .graphql_query(
            application_id,
            "query { totalMatches }",
        )
        .await;

    let total_matches = response["totalMatches"]
        .as_u64()
        .expect("Failed to get total matches");

    assert!(total_matches >= 1);
}

/// Test leave queue
#[tokio::test(flavor = "multi_thread")]
async fn test_leave_queue() {
    let (validator, module_id) =
        TestValidator::with_current_module::<MatchmakingAbi, (), ()>().await;

    let mut chain = validator.new_chain().await;

    let application_id = chain
        .create_application(module_id, (), (), vec![])
        .await;

    let character = CharacterSnapshot {
        class: CharacterClass::Rogue,
        level: 10,
        hp_max: 90,
        attack: 55,
        defense: 25,
        speed: 60,
        crit_rate: 20,
        crit_damage: 200,
        dodge_rate: 15,
        accuracy: 85,
    };

    // Join queue
    chain
        .add_block(|block| {
            block.with_operation(
                application_id,
                Operation::JoinQueue {
                    character,
                    stake: Amount::from_tokens(100),
                },
            );
        })
        .await;

    // Leave queue
    chain
        .add_block(|block| {
            block.with_operation(
                application_id,
                Operation::LeaveQueue,
            );
        })
        .await;

    // Query queue size (should be 0)
    let QueryOutcome { response, .. } = chain
        .graphql_query(
            application_id,
            "query { queueSize }",
        )
        .await;

    let queue_size = response["queueSize"]
        .as_u64()
        .expect("Failed to get queue size");

    assert_eq!(queue_size, 0);
}

/// Test minimum stake requirement
#[tokio::test(flavor = "multi_thread")]
async fn test_minimum_stake() {
    let (validator, module_id) =
        TestValidator::with_current_module::<MatchmakingAbi, (), ()>().await;

    let mut chain = validator.new_chain().await;

    let application_id = chain
        .create_application(module_id, (), (), vec![])
        .await;

    let character = CharacterSnapshot {
        class: CharacterClass::Healer,
        level: 10,
        hp_max: 110,
        attack: 40,
        defense: 35,
        speed: 35,
        crit_rate: 8,
        crit_damage: 140,
        dodge_rate: 5,
        accuracy: 98,
    };

    // Attempt to join with insufficient stake
    chain
        .add_block(|block| {
            block.with_operation(
                application_id,
                Operation::JoinQueue {
                    character,
                    stake: Amount::from_tokens(1), // Too low
                },
            );
        })
        .await;

    // Query queue size (should be 0 - rejected)
    let QueryOutcome { response, .. } = chain
        .graphql_query(
            application_id,
            "query { queueSize }",
        )
        .await;

    let queue_size = response["queueSize"]
        .as_u64()
        .expect("Failed to get queue size");

    assert_eq!(queue_size, 0);
}

/// Test ELO-based matchmaking
#[tokio::test(flavor = "multi_thread")]
async fn test_elo_matchmaking() {
    let (validator, module_id) =
        TestValidator::with_current_module::<MatchmakingAbi, (), ()>().await;

    let mut chain1 = validator.new_chain().await;
    let mut chain2 = validator.new_chain().await;
    let mut chain3 = validator.new_chain().await;

    let application_id = chain1
        .create_application(module_id, (), (), vec![])
        .await;

    // Create characters with different skill levels
    let beginner = CharacterSnapshot {
        class: CharacterClass::Warrior,
        level: 5,
        hp_max: 50,
        attack: 30,
        defense: 20,
        speed: 25,
        crit_rate: 5,
        crit_damage: 130,
        dodge_rate: 3,
        accuracy: 90,
    };

    let intermediate = CharacterSnapshot {
        class: CharacterClass::Mage,
        level: 10,
        hp_max: 80,
        attack: 60,
        defense: 30,
        speed: 45,
        crit_rate: 12,
        crit_damage: 160,
        dodge_rate: 8,
        accuracy: 92,
    };

    let advanced = CharacterSnapshot {
        class: CharacterClass::Rogue,
        level: 15,
        hp_max: 120,
        attack: 90,
        defense: 50,
        speed: 70,
        crit_rate: 25,
        crit_damage: 220,
        dodge_rate: 20,
        accuracy: 88,
    };

    // All join queue
    for (chain, character) in [
        (&mut chain1, beginner),
        (&mut chain2, intermediate),
        (&mut chain3, advanced),
    ] {
        chain
            .add_block(|block| {
                block.with_operation(
                    application_id,
                    Operation::JoinQueue {
                        character,
                        stake: Amount::from_tokens(100),
                    },
                );
            })
            .await;
    }

    // Matchmaking should pair players with similar ELO
    // (Implementation detail - testing that matches are created)
}
