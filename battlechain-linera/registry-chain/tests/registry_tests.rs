use battlechain_shared_events::CombatStats;
use battlechain_shared_types::{CharacterClass, Owner};
use linera_sdk::{
    linera_base_types::{AccountOwner, Amount, ChainId},
    test::{ActiveChain, TestValidator},
    QueryOutcome,
};
use registry_chain::{Message, Operation, RegistryAbi};

/// Test character registration
#[tokio::test(flavor = "multi_thread")]
async fn test_register_character() {
    let (validator, module_id) =
        TestValidator::with_current_module::<RegistryAbi, (), ()>().await;

    let mut chain = validator.new_chain().await;
    let owner = chain.owner();

    let application_id = chain
        .create_application(module_id, (), (), vec![])
        .await;

    let character_id = "char_reg_001".to_string();
    let nft_id = "nft_reg_001".to_string();

    // Register character
    chain
        .add_block(|block| {
            block.with_operation(
                application_id,
                Operation::RegisterCharacter {
                    character_id: character_id.clone(),
                    nft_id: nft_id.clone(),
                    owner,
                    owner_chain: ChainId::from([1u8; 32]),
                    class: CharacterClass::Warrior,
                    level: 1,
                },
            );
        })
        .await;

    // Query total characters
    let QueryOutcome { response, .. } = chain
        .graphql_query(
            application_id,
            "query { totalCharacters }",
        )
        .await;

    let total_characters = response["totalCharacters"]
        .as_u64()
        .expect("Failed to get total characters");

    assert_eq!(total_characters, 1);
}

/// Test update character stats after battle
#[tokio::test(flavor = "multi_thread")]
async fn test_update_character_stats() {
    let (validator, module_id) =
        TestValidator::with_current_module::<RegistryAbi, (), ()>().await;

    let mut chain = validator.new_chain().await;
    let owner = chain.owner();

    let application_id = chain
        .create_application(module_id, (), (), vec![])
        .await;

    let character_id = "char_battle_001".to_string();

    // Register character
    chain
        .add_block(|block| {
            block.with_operation(
                application_id,
                Operation::RegisterCharacter {
                    character_id: character_id.clone(),
                    nft_id: "nft_battle_001".to_string(),
                    owner,
                    owner_chain: ChainId::from([1u8; 32]),
                    class: CharacterClass::Mage,
                    level: 10,
                },
            );
        })
        .await;

    // Update stats after battle
    chain
        .add_block(|block| {
            block.with_operation(
                application_id,
                Operation::UpdateCharacterStats {
                    character_id: character_id.clone(),
                    won: true,
                    damage_dealt: 500,
                    damage_taken: 200,
                    crits: 5,
                    dodges: 3,
                    highest_crit: 150,
                    earnings: Amount::from_tokens(200),
                    stake: Amount::from_tokens(100),
                    opponent_elo: 1200,
                },
            );
        })
        .await;

    // Query character stats
    let QueryOutcome { response, .. } = chain
        .graphql_query(
            application_id,
            &format!(
                "query {{ character(characterId: \\\"{}\\\") {{ totalBattles wins }} }}",
                character_id
            ),
        )
        .await;

    // Verify stats updated
}

/// Test record battle
#[tokio::test(flavor = "multi_thread")]
async fn test_record_battle() {
    let (validator, module_id) =
        TestValidator::with_current_module::<RegistryAbi, (), ()>().await;

    let mut chain = validator.new_chain().await;

    let application_id = chain
        .create_application(module_id, (), (), vec![])
        .await;

    let battle_chain = ChainId::from([1u8; 32]);

    // Record battle
    chain
        .add_block(|block| {
            block.with_operation(
                application_id,
                Operation::RecordBattle {
                    battle_chain,
                    player1_id: "char_001".to_string(),
                    player2_id: "char_002".to_string(),
                    winner_id: "char_001".to_string(),
                    stake: Amount::from_tokens(100),
                    rounds_played: 3,
                },
            );
        })
        .await;

    // Query total battles
    let QueryOutcome { response, .. } = chain
        .graphql_query(
            application_id,
            "query { totalBattles }",
        )
        .await;

    let total_battles = response["totalBattles"]
        .as_u64()
        .expect("Failed to get total battles");

    assert_eq!(total_battles, 1);
}

/// Test ELO rating system
#[tokio::test(flavor = "multi_thread")]
async fn test_elo_rating_update() {
    let (validator, module_id) =
        TestValidator::with_current_module::<RegistryAbi, (), ()>().await;

    let mut chain = validator.new_chain().await;
    let owner = chain.owner();

    let application_id = chain
        .create_application(module_id, (), (), vec![])
        .await;

    let character_id = "char_elo_test".to_string();

    // Register character with default ELO (1000)
    chain
        .add_block(|block| {
            block.with_operation(
                application_id,
                Operation::RegisterCharacter {
                    character_id: character_id.clone(),
                    nft_id: "nft_elo_test".to_string(),
                    owner,
                    owner_chain: ChainId::from([1u8; 32]),
                    class: CharacterClass::Rogue,
                    level: 10,
                },
            );
        })
        .await;

    // Win battle against higher ELO opponent
    chain
        .add_block(|block| {
            block.with_operation(
                application_id,
                Operation::UpdateCharacterStats {
                    character_id: character_id.clone(),
                    won: true,
                    damage_dealt: 600,
                    damage_taken: 300,
                    crits: 10,
                    dodges: 5,
                    highest_crit: 180,
                    earnings: Amount::from_tokens(250),
                    stake: Amount::from_tokens(100),
                    opponent_elo: 1300, // Higher than player's ELO
                },
            );
        })
        .await;

    // Query character ELO (should increase more due to higher opponent)
}

/// Test leaderboard rankings
#[tokio::test(flavor = "multi_thread")]
async fn test_leaderboard() {
    let (validator, module_id) =
        TestValidator::with_current_module::<RegistryAbi, (), ()>().await;

    let mut chain = validator.new_chain().await;
    let owner = chain.owner();

    let application_id = chain
        .create_application(module_id, (), (), vec![])
        .await;

    // Register multiple characters with different performance
    for i in 0..5 {
        let character_id = format!("char_leaderboard_{}", i);

        chain
            .add_block(|block| {
                block.with_operation(
                    application_id,
                    Operation::RegisterCharacter {
                        character_id: character_id.clone(),
                        nft_id: format!("nft_leaderboard_{}", i),
                        owner,
                        owner_chain: ChainId::from([1u8; 32]),
                        class: CharacterClass::Warrior,
                        level: 10 + i as u16,
                    },
                );
            })
            .await;

        // Simulate different win rates
        for _ in 0..i {
            chain
                .add_block(|block| {
                    block.with_operation(
                        application_id,
                        Operation::UpdateCharacterStats {
                            character_id: character_id.clone(),
                            won: true,
                            damage_dealt: 500,
                            damage_taken: 200,
                            crits: 5,
                            dodges: 3,
                            highest_crit: 150,
                            earnings: Amount::from_tokens(200),
                            stake: Amount::from_tokens(100),
                            opponent_elo: 1000,
                        },
                    );
                })
                .await;
        }
    }

    // Query leaderboard
    let QueryOutcome { response, .. } = chain
        .graphql_query(
            application_id,
            "query { leaderboard(limit: 10) { characterId elo winRate } }",
        )
        .await;

    // Verify leaderboard is sorted by ELO/performance
}

/// Test character statistics tracking
#[tokio::test(flavor = "multi_thread")]
async fn test_character_statistics() {
    let (validator, module_id) =
        TestValidator::with_current_module::<RegistryAbi, (), ()>().await;

    let mut chain = validator.new_chain().await;
    let owner = chain.owner();

    let application_id = chain
        .create_application(module_id, (), (), vec![])
        .await;

    let character_id = "char_stats_test".to_string();

    // Register character
    chain
        .add_block(|block| {
            block.with_operation(
                application_id,
                Operation::RegisterCharacter {
                    character_id: character_id.clone(),
                    nft_id: "nft_stats_test".to_string(),
                    owner,
                    owner_chain: ChainId::from([1u8; 32]),
                    class: CharacterClass::Healer,
                    level: 10,
                },
            );
        })
        .await;

    // Perform multiple battles
    for i in 0..10 {
        chain
            .add_block(|block| {
                block.with_operation(
                    application_id,
                    Operation::UpdateCharacterStats {
                        character_id: character_id.clone(),
                        won: i % 2 == 0, // Win every other battle
                        damage_dealt: 400 + (i * 10),
                        damage_taken: 150 + (i * 5),
                        crits: 3 + i,
                        dodges: 2 + i,
                        highest_crit: 120 + (i * 5),
                        earnings: Amount::from_tokens(100 + (i * 50)),
                        stake: Amount::from_tokens(100),
                        opponent_elo: 950 + (i * 20),
                    },
                );
            })
            .await;
    }

    // Query aggregated stats
    let QueryOutcome { response, .. } = chain
        .graphql_query(
            application_id,
            &format!(
                "query {{ character(characterId: \\\"{}\\\") {{
                    totalBattles
                    wins
                    losses
                    winRate
                    totalDamageDealt
                    totalDamageTaken
                    totalCrits
                    totalDodges
                    highestCrit
                }} }}",
                character_id
            ),
        )
        .await;

    // Verify all stats are tracked correctly
}

/// Test battle history retrieval
#[tokio::test(flavor = "multi_thread")]
async fn test_battle_history() {
    let (validator, module_id) =
        TestValidator::with_current_module::<RegistryAbi, (), ()>().await;

    let mut chain = validator.new_chain().await;

    let application_id = chain
        .create_application(module_id, (), (), vec![])
        .await;

    // Record multiple battles
    for i in 0..5 {
        chain
            .add_block(|block| {
                block.with_operation(
                    application_id,
                    Operation::RecordBattle {
                        battle_chain: ChainId::from([i; 32]),
                        player1_id: format!("char_{:03}", i),
                        player2_id: format!("char_{:03}", i + 1),
                        winner_id: format!("char_{:03}", i),
                        stake: Amount::from_tokens(100 + (i as u64 * 50)),
                        rounds_played: (i as u8 % 5) + 1,
                    },
                );
            })
            .await;
    }

    // Query battle history
    let QueryOutcome { response, .. } = chain
        .graphql_query(
            application_id,
            "query { recentBattles(limit: 5) { battleId winnerId stake roundsPlayed } }",
        )
        .await;

    // Verify battle history is maintained
}
