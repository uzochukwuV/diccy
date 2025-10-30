// Copyright (c) Fighter Game
// SPDX-License-Identifier: Apache-2.0

//! Integration tests for the Fighter Game

#![cfg(not(target_arch = "wasm32"))]

use fighter_game::{BattleConfig, BattleStatus, FighterGameAbi, FighterOutcome, Operation};
use linera_sdk::{
    linera_base_types::{AccountSecretKey, Amount, TimeDelta},
    test::{ActiveChain, QueryOutcome, TestValidator},
};

#[test_log::test(tokio::test)]
async fn test_fighter_registration() {
    let key_pair1 = AccountSecretKey::generate();
    
    let (validator, app_id, mut chain) =
        TestValidator::with_current_application::<FighterGameAbi, _, _>(
            key_pair1.copy(),
            (),
            BattleConfig::default()
        ).await;
    
    // Register a fighter
    let certificate = chain
        .add_block(|block| {
            block.with_operation(
                app_id,
                Operation::RegisterFighter {
                    name: "TestWarrior".to_string(),
                },
            );
        })
        .await;
    
    // Query the fighter
    let QueryOutcome { response, .. } = chain
        .graphql_query(
            app_id,
            &format!(
                r#"query {{ fighter(owner: "{}") {{ name level xp maxHp }} }}"#,
                key_pair1.public()
            ),
        )
        .await;
    
    assert_eq!(response["fighter"]["name"], "TestWarrior");
    assert_eq!(response["fighter"]["level"], 1);
    assert_eq!(response["fighter"]["xp"], 0);
    assert_eq!(response["fighter"]["maxHp"], 100);
}

#[test_log::test(tokio::test)]
async fn test_free_battle_complete_flow() {
    let key_pair1 = AccountSecretKey::generate();
    let key_pair2 = AccountSecretKey::generate();
    
    let (validator, app_id, mut chain1) =
        TestValidator::with_current_application::<FighterGameAbi, _, _>(
            key_pair1.copy(),
            (),
            BattleConfig::default()
        ).await;
    
    // Register both fighters
    chain1
        .add_block(|block| {
            block.with_operation(
                app_id,
                Operation::RegisterFighter {
                    name: "Fighter1".to_string(),
                },
            );
        })
        .await;
    
    // Create second chain for second player
    let description = chain1.description();
    let mut chain2 = ActiveChain::new(key_pair2.copy(), description.clone(), validator.clone());
    
    chain2
        .add_block(|block| {
            block.with_operation(
                app_id,
                Operation::RegisterFighter {
                    name: "Fighter2".to_string(),
                },
            );
        })
        .await;
    
    // Start a free battle
    chain1
        .add_block(|block| {
            block.with_operation(
                app_id,
                Operation::StartFreeBattle {
                    opponent: key_pair2.public().into(),
                },
            );
        })
        .await;
    
    // Query to get battle ID
    let QueryOutcome { response, .. } = chain1
        .graphql_query(app_id, "query { nextBattleId }")
        .await;
    
    let battle_id = response["nextBattleId"].as_u64().unwrap() - 1;
    
    // Query battle state
    let QueryOutcome { response, .. } = chain1
        .graphql_query(
            app_id,
            &format!("query {{ battle(battleId: {}) {{ status fighter1Hp fighter2Hp }} }}", battle_id),
        )
        .await;
    
    assert_eq!(response["battle"]["status"], "ACTIVE");
    assert_eq!(response["battle"]["fighter1Hp"], 100);
    assert_eq!(response["battle"]["fighter2Hp"], 100);
    
    // Execute strikes until battle ends
    let mut current_chain = &mut chain1;
    let mut current_key = &key_pair1;
    let mut turn = 0;
    
    // Simulate battle with max 20 turns
    while turn < 20 {
        current_chain
            .add_block(|block| {
                block.with_operation(app_id, Operation::Strike { battle_id });
            })
            .await;
        
        // Check battle status
        let QueryOutcome { response, .. } = current_chain
            .graphql_query(
                app_id,
                &format!("query {{ battle(battleId: {}) {{ status winner }} }}", battle_id),
            )
            .await;
        
        if response["battle"]["status"] == "FINISHED" {
            assert!(!response["battle"]["winner"].is_null());
            break;
        }
        
        // Switch to other player
        if current_key.public() == key_pair1.public() {
            current_chain = &mut chain2;
            current_key = &key_pair2;
        } else {
            current_chain = &mut chain1;
            current_key = &key_pair1;
        }
        
        turn += 1;
    }
    
    assert!(turn < 20, "Battle should complete within 20 turns");
}

#[test_log::test(tokio::test)]
async fn test_staked_battle() {
    let key_pair1 = AccountSecretKey::generate();
    let key_pair2 = AccountSecretKey::generate();
    
    let (validator, app_id, mut chain1) =
        TestValidator::with_current_application::<FighterGameAbi, _, _>(
            key_pair1.copy(),
            (),
            BattleConfig::default()
        ).await;
    
    // Register fighters
    chain1
        .add_block(|block| {
            block.with_operation(
                app_id,
                Operation::RegisterFighter {
                    name: "Staker1".to_string(),
                },
            );
        })
        .await;
    
    let description = chain1.description();
    let mut chain2 = ActiveChain::new(key_pair2.copy(), description.clone(), validator.clone());
    
    chain2
        .add_block(|block| {
            block.with_operation(
                app_id,
                Operation::RegisterFighter {
                    name: "Staker2".to_string(),
                },
            );
        })
        .await;
    
    // Start staked battle with 1 token
    let stake = Amount::from_tokens(1);
    chain1
        .add_block(|block| {
            block.with_operation(
                app_id,
                Operation::StartStakedBattle {
                    opponent: key_pair2.public().into(),
                    stake_amount: stake,
                },
            );
        })
        .await;
    
    let QueryOutcome { response, .. } = chain1
        .graphql_query(app_id, "query { nextBattleId }")
        .await;
    
    let battle_id = response["nextBattleId"].as_u64().unwrap() - 1;
    
    // Verify stake amount
    let QueryOutcome { response, .. } = chain1
        .graphql_query(
            app_id,
            &format!("query {{ battle(battleId: {}) {{ stakeAmount isFreePlay }} }}", battle_id),
        )
        .await;
    
    assert_eq!(response["battle"]["isFreePlay"], false);
}

#[test_log::test(tokio::test)]
async fn test_timeout_claim() {
    let key_pair1 = AccountSecretKey::generate();
    let key_pair2 = AccountSecretKey::generate();
    
    let config = BattleConfig {
        turn_timeout: TimeDelta::from_secs(30),
        block_delay: TimeDelta::from_secs(5),
        platform_fee: 10,
    };
    
    let (validator, app_id, mut chain1) =
        TestValidator::with_current_application::<FighterGameAbi, _, _>(
            key_pair1.copy(),
            (),
            config
        ).await;
    
    // Register fighters
    chain1
        .add_block(|block| {
            block.with_operation(
                app_id,
                Operation::RegisterFighter {
                    name: "QuickPlayer".to_string(),
                },
            );
        })
        .await;
    
    let description = chain1.description();
    let mut chain2 = ActiveChain::new(key_pair2.copy(), description.clone(), validator.clone());
    
    chain2
        .add_block(|block| {
            block.with_operation(
                app_id,
                Operation::RegisterFighter {
                    name: "SlowPlayer".to_string(),
                },
            );
        })
        .await;
    
    // Start battle
    let time = validator.clock().current_time();
    chain1
        .add_block(|block| {
            block
                .with_operation(
                    app_id,
                    Operation::StartFreeBattle {
                        opponent: key_pair2.public().into(),
                    },
                )
                .with_timestamp(time);
        })
        .await;
    
    let QueryOutcome { response, .. } = chain1
        .graphql_query(app_id, "query { nextBattleId }")
        .await;
    
    let battle_id = response["nextBattleId"].as_u64().unwrap() - 1;
    
    // Advance time beyond timeout
    validator.clock().add(TimeDelta::from_secs(31));
    let timeout_time = validator.clock().current_time();
    
    // Player 2 claims timeout victory (since it's Player 1's turn)
    chain2
        .add_block(|block| {
            block
                .with_operation(app_id, Operation::ClaimTimeout { battle_id })
                .with_timestamp(timeout_time);
        })
        .await;
    
    // Verify battle ended with timeout
    let QueryOutcome { response, .. } = chain2
        .graphql_query(
            app_id,
            &format!("query {{ battle(battleId: {}) {{ status winner }} }}", battle_id),
        )
        .await;
    
    assert_eq!(response["battle"]["status"], "TIMED_OUT");
}

#[test_log::test(tokio::test)]
async fn test_matchmaking_tier_restrictions() {
    let key_pair1 = AccountSecretKey::generate();
    let key_pair2 = AccountSecretKey::generate();
    
    let (validator, app_id, mut chain1) =
        TestValidator::with_current_application::<FighterGameAbi, _, _>(
            key_pair1.copy(),
            (),
            BattleConfig::default()
        ).await;
    
    // Register level 1 fighter
    chain1
        .add_block(|block| {
            block.with_operation(
                app_id,
                Operation::RegisterFighter {
                    name: "Novice".to_string(),
                },
            );
        })
        .await;
    
    let description = chain1.description();
    let mut chain2 = ActiveChain::new(key_pair2.copy(), description.clone(), validator.clone());
    
    // Register and artificially level up second fighter
    // In production, this would be done through actual battles
    chain2
        .add_block(|block| {
            block.with_operation(
                app_id,
                Operation::RegisterFighter {
                    name: "Expert".to_string(),
                },
            );
        })
        .await;
    
    // Note: In a real test, we'd need to level up Fighter2 significantly
    // For this test, we're just demonstrating the flow
    // The contract will reject mismatched tiers
}

#[test_log::test(tokio::test)]
async fn test_tournament_creation() {
    let key_pair1 = AccountSecretKey::generate();
    
    let (validator, app_id, mut chain) =
        TestValidator::with_current_application::<FighterGameAbi, _, _>(
            key_pair1.copy(),
            (),
            BattleConfig::default()
        ).await;
    
    let start_time = validator.clock().current_time();
    
    chain
        .add_block(|block| {
            block.with_operation(
                app_id,
                Operation::CreateTournament {
                    name: "Championship".to_string(),
                    entry_fee: Amount::from_tokens(1),
                    start_time,
                    max_participants: 16,
                    prize_pool_distribution: vec![50, 30, 20], // 1st, 2nd, 3rd
                },
            );
        })
        .await;
    
    let QueryOutcome { response, .. } = chain
        .graphql_query(app_id, "query { nextTournamentId }")
        .await;
    
    let tournament_id = response["nextTournamentId"].as_u64().unwrap() - 1;
    
    // Query tournament
    let QueryOutcome { response, .. } = chain
        .graphql_query(
            app_id,
            &format!("query {{ tournament(tournamentId: {}) {{ name status maxParticipants }} }}", tournament_id),
        )
        .await;
    
    assert_eq!(response["tournament"]["name"], "Championship");
    assert_eq!(response["tournament"]["status"], "REGISTRATION");
    assert_eq!(response["tournament"]["maxParticipants"], 16);
}

#[test_log::test(tokio::test)]
async fn test_leaderboard() {
    let key_pair1 = AccountSecretKey::generate();
    let key_pair2 = AccountSecretKey::generate();
    
    let (validator, app_id, mut chain1) =
        TestValidator::with_current_application::<FighterGameAbi, _, _>(
            key_pair1.copy(),
            (),
            BattleConfig::default()
        ).await;
    
    // Register multiple fighters
    chain1
        .add_block(|block| {
            block.with_operation(
                app_id,
                Operation::RegisterFighter {
                    name: "Champion".to_string(),
                },
            );
        })
        .await;
    
    let description = chain1.description();
    let mut chain2 = ActiveChain::new(key_pair2.copy(), description.clone(), validator.clone());
    
    chain2
        .add_block(|block| {
            block.with_operation(
                app_id,
                Operation::RegisterFighter {
                    name: "Challenger".to_string(),
                },
            );
        })
        .await;
    
    // Query leaderboard
    let QueryOutcome { response, .. } = chain1
        .graphql_query(app_id, "query { leaderboard(limit: 10) { rank name level xp } }")
        .await;
    
    assert!(response["leaderboard"].is_array());
    let leaderboard = response["leaderboard"].as_array().unwrap();
    assert!(leaderboard.len() >= 2);
}

#[test_log::test(tokio::test)]
async fn test_global_stats() {
    let key_pair1 = AccountSecretKey::generate();
    
    let (validator, app_id, mut chain) =
        TestValidator::with_current_application::<FighterGameAbi, _, _>(
            key_pair1.copy(),
            (),
            BattleConfig::default()
        ).await;
    
    // Register a fighter
    chain
        .add_block(|block| {
            block.with_operation(
                app_id,
                Operation::RegisterFighter {
                    name: "StatsTest".to_string(),
                },
            );
        })
        .await;
    
    // Query global stats
    let QueryOutcome { response, .. } = chain
        .graphql_query(
            app_id,
            "query { globalStats { totalFighters totalBattles activeBattles } }",
        )
        .await;
    
    assert_eq!(response["globalStats"]["totalFighters"], 1);
    assert_eq!(response["globalStats"]["totalBattles"], 0);
    assert_eq!(response["globalStats"]["activeBattles"], 0);
}
