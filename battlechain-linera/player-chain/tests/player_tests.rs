use battlechain_shared_types::{CharacterClass, CharacterNFT};
use linera_sdk::{
    linera_base_types::{AccountOwner, Amount, ApplicationId, ChainId},
    test::{ActiveChain, TestValidator},
    QueryOutcome,
};
use player_chain::{Message, Operation, PlayerChainAbi};

type Owner = AccountOwner;

/// Test character creation
#[tokio::test(flavor = "multi_thread")]
async fn test_create_character() {
    let (validator, module_id) =
        TestValidator::with_current_module::<PlayerChainAbi, (), ()>().await;

    let mut chain = validator.new_chain().await;
    let owner = chain.owner();

    let application_id = chain
        .create_application(module_id, (), (), vec![])
        .await;

    // Create a character
    let character_id = "char_001".to_string();
    let nft_id = "nft_001".to_string();
    let class = CharacterClass::Warrior;

    chain
        .add_block(|block| {
            block.with_operation(
                application_id,
                Operation::CreateCharacter {
                    character_id: character_id.clone(),
                    nft_id: nft_id.clone(),
                    class,
                },
            );
        })
        .await;

    // Query character count
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

/// Test character leveling up
#[tokio::test(flavor = "multi_thread")]
async fn test_level_up_character() {
    let (validator, module_id) =
        TestValidator::with_current_module::<PlayerChainAbi, (), ()>().await;

    let mut chain = validator.new_chain().await;

    let application_id = chain
        .create_application(module_id, (), (), vec![])
        .await;

    let character_id = "char_level_test".to_string();

    // Create character
    chain
        .add_block(|block| {
            block.with_operation(
                application_id,
                Operation::CreateCharacter {
                    character_id: character_id.clone(),
                    nft_id: "nft_level_test".to_string(),
                    class: CharacterClass::Mage,
                },
            );
        })
        .await;

    // Level up character
    chain
        .add_block(|block| {
            block.with_operation(
                application_id,
                Operation::LevelUp {
                    character_id: character_id.clone(),
                },
            );
        })
        .await;

    // Verify character stats increased
    // (In real implementation, you'd query the character's level)
}

/// Test multiple characters of different classes
#[tokio::test(flavor = "multi_thread")]
async fn test_multiple_characters() {
    let (validator, module_id) =
        TestValidator::with_current_module::<PlayerChainAbi, (), ()>().await;

    let mut chain = validator.new_chain().await;

    let application_id = chain
        .create_application(module_id, (), (), vec![])
        .await;

    let classes = vec![
        CharacterClass::Warrior,
        CharacterClass::Mage,
        CharacterClass::Rogue,
        CharacterClass::Healer,
    ];

    // Create multiple characters
    for (i, class) in classes.iter().enumerate() {
        chain
            .add_block(|block| {
                block.with_operation(
                    application_id,
                    Operation::CreateCharacter {
                        character_id: format!("char_{:03}", i),
                        nft_id: format!("nft_{:03}", i),
                        class: *class,
                    },
                );
            })
            .await;
    }

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

    assert_eq!(total_characters, 4);
}

/// Test invalid character creation (duplicate ID)
#[tokio::test(flavor = "multi_thread")]
async fn test_duplicate_character_creation() {
    let (validator, module_id) =
        TestValidator::with_current_module::<PlayerChainAbi, (), ()>().await;

    let mut chain = validator.new_chain().await;

    let application_id = chain
        .create_application(module_id, (), (), vec![])
        .await;

    let character_id = "duplicate_char".to_string();

    // Create first character
    chain
        .add_block(|block| {
            block.with_operation(
                application_id,
                Operation::CreateCharacter {
                    character_id: character_id.clone(),
                    nft_id: "nft_1".to_string(),
                    class: CharacterClass::Warrior,
                },
            );
        })
        .await;

    // Attempt to create duplicate character
    chain
        .add_block(|block| {
            block.with_operation(
                application_id,
                Operation::CreateCharacter {
                    character_id: character_id.clone(),
                    nft_id: "nft_2".to_string(),
                    class: CharacterClass::Mage,
                },
            );
        })
        .await;

    // Verify still only 1 character (duplicate rejected)
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

/// Test character equip item
#[tokio::test(flavor = "multi_thread")]
async fn test_equip_item() {
    let (validator, module_id) =
        TestValidator::with_current_module::<PlayerChainAbi, (), ()>().await;

    let mut chain = validator.new_chain().await;

    let application_id = chain
        .create_application(module_id, (), (), vec![])
        .await;

    let character_id = "char_equip_test".to_string();

    // Create character
    chain
        .add_block(|block| {
            block.with_operation(
                application_id,
                Operation::CreateCharacter {
                    character_id: character_id.clone(),
                    nft_id: "nft_equip".to_string(),
                    class: CharacterClass::Warrior,
                },
            );
        })
        .await;

    // Equip item
    chain
        .add_block(|block| {
            block.with_operation(
                application_id,
                Operation::EquipItem {
                    character_id: character_id.clone(),
                    item_id: "sword_001".to_string(),
                },
            );
        })
        .await;

    // Verify item equipped (query would check equipped items)
}
