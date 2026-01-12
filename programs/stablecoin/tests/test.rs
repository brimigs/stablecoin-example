use litesvm::LiteSVM;
use solana_sdk::{
    instruction::{AccountMeta, Instruction},
    native_token::LAMPORTS_PER_SOL,
    pubkey::Pubkey,
    signature::Keypair,
    signer::Signer,
    system_program,
    transaction::Transaction,
};
use sha2::{Sha256, Digest};

// Program ID from declare_id! in lib.rs
const PROGRAM_ID: Pubkey = solana_sdk::pubkey!("2hFkP8rkdPzyMsjsp5AddPyfpu1aY69qkjXf1Xd97b6K");

// SPL Token Program ID
const TOKEN_PROGRAM_ID: Pubkey = solana_sdk::pubkey!("TokenkegQfeZyiNwAJbNbGKPFXCWuBvf9Ss623VQ5DA");

// Associated Token Program ID
const ASSOCIATED_TOKEN_PROGRAM_ID: Pubkey = solana_sdk::pubkey!("ATokenGPvbdGVxr1b2hvZbsiqW5xWH25efTNsLJA8knL");

// Helper function to compute Anchor instruction discriminator
fn get_discriminator(instruction_name: &str) -> [u8; 8] {
    let mut hasher = Sha256::new();
    hasher.update(format!("global:{}", instruction_name).as_bytes());
    let result = hasher.finalize();
    let mut discriminator = [0u8; 8];
    discriminator.copy_from_slice(&result[..8]);
    discriminator
}

// Helper function to compute associated token address
fn get_associated_token_address(wallet: &Pubkey, mint: &Pubkey) -> Pubkey {
    Pubkey::find_program_address(
        &[
            wallet.as_ref(),
            TOKEN_PROGRAM_ID.as_ref(),
            mint.as_ref(),
        ],
        &ASSOCIATED_TOKEN_PROGRAM_ID,
    ).0
}

// Derive PDAs
fn get_config_pda() -> (Pubkey, u8) {
    Pubkey::find_program_address(&[b"config"], &PROGRAM_ID)
}

fn get_mint_pda() -> (Pubkey, u8) {
    Pubkey::find_program_address(&[b"mint"], &PROGRAM_ID)
}

fn get_minter_config_pda(minter: &Pubkey) -> (Pubkey, u8) {
    Pubkey::find_program_address(&[b"minter", minter.as_ref()], &PROGRAM_ID)
}

// Setup a new LiteSVM instance with the stablecoin program loaded
fn setup_svm() -> LiteSVM {
    let mut svm = LiteSVM::new();

    // Load the compiled program
    let program_bytes = include_bytes!("../../../target/deploy/stablecoin.so");
    svm.add_program(PROGRAM_ID, program_bytes);

    svm
}

// ============================================================================
// Initialize Tests
// ============================================================================

#[test]
fn test_initialize() {
    let mut svm = setup_svm();

    let admin = Keypair::new();
    svm.airdrop(&admin.pubkey(), LAMPORTS_PER_SOL * 10).unwrap();

    let (config_pda, _) = get_config_pda();
    let (mint_pda, _) = get_mint_pda();

    // Build initialize instruction
    let ix_data = get_discriminator("initialize");

    let ix = Instruction {
        program_id: PROGRAM_ID,
        accounts: vec![
            AccountMeta::new(admin.pubkey(), true),           // admin (signer, mut)
            AccountMeta::new(config_pda, false),              // config PDA
            AccountMeta::new(mint_pda, false),                // mint PDA
            AccountMeta::new_readonly(TOKEN_PROGRAM_ID, false), // token_program
            AccountMeta::new_readonly(system_program::id(), false), // system_program
        ],
        data: ix_data.to_vec(),
    };

    let tx = Transaction::new_signed_with_payer(
        &[ix],
        Some(&admin.pubkey()),
        &[&admin],
        svm.latest_blockhash(),
    );

    let result = svm.send_transaction(tx);
    assert!(result.is_ok(), "Initialize should succeed: {:?}", result.err());

    // Verify config account was created
    let config_account = svm.get_account(&config_pda);
    assert!(config_account.is_some(), "Config account should exist");

    // Verify mint account was created
    let mint_account = svm.get_account(&mint_pda);
    assert!(mint_account.is_some(), "Mint account should exist");
}

#[test]
fn test_initialize_twice_fails() {
    let mut svm = setup_svm();

    let admin = Keypair::new();
    svm.airdrop(&admin.pubkey(), LAMPORTS_PER_SOL * 10).unwrap();

    let (config_pda, _) = get_config_pda();
    let (mint_pda, _) = get_mint_pda();

    let ix_data = get_discriminator("initialize");

    let ix = Instruction {
        program_id: PROGRAM_ID,
        accounts: vec![
            AccountMeta::new(admin.pubkey(), true),
            AccountMeta::new(config_pda, false),
            AccountMeta::new(mint_pda, false),
            AccountMeta::new_readonly(TOKEN_PROGRAM_ID, false),
            AccountMeta::new_readonly(system_program::id(), false),
        ],
        data: ix_data.to_vec(),
    };

    // First initialize should succeed
    let tx1 = Transaction::new_signed_with_payer(
        &[ix.clone()],
        Some(&admin.pubkey()),
        &[&admin],
        svm.latest_blockhash(),
    );
    let result1 = svm.send_transaction(tx1);
    assert!(result1.is_ok(), "First initialize should succeed");

    // Second initialize should fail
    let tx2 = Transaction::new_signed_with_payer(
        &[ix],
        Some(&admin.pubkey()),
        &[&admin],
        svm.latest_blockhash(),
    );
    let result2 = svm.send_transaction(tx2);
    assert!(result2.is_err(), "Second initialize should fail");
}

// ============================================================================
// Configure Minter Tests
// ============================================================================

fn initialize_program(svm: &mut LiteSVM, admin: &Keypair) {
    let (config_pda, _) = get_config_pda();
    let (mint_pda, _) = get_mint_pda();

    let ix_data = get_discriminator("initialize");

    let ix = Instruction {
        program_id: PROGRAM_ID,
        accounts: vec![
            AccountMeta::new(admin.pubkey(), true),
            AccountMeta::new(config_pda, false),
            AccountMeta::new(mint_pda, false),
            AccountMeta::new_readonly(TOKEN_PROGRAM_ID, false),
            AccountMeta::new_readonly(system_program::id(), false),
        ],
        data: ix_data.to_vec(),
    };

    let tx = Transaction::new_signed_with_payer(
        &[ix],
        Some(&admin.pubkey()),
        &[&admin],
        svm.latest_blockhash(),
    );

    svm.send_transaction(tx).expect("Initialize should succeed");
}

#[test]
fn test_configure_minter() {
    let mut svm = setup_svm();

    let admin = Keypair::new();
    svm.airdrop(&admin.pubkey(), LAMPORTS_PER_SOL * 10).unwrap();

    // Initialize first
    initialize_program(&mut svm, &admin);

    let (config_pda, _) = get_config_pda();
    let minter = Keypair::new();
    let (minter_config_pda, _) = get_minter_config_pda(&minter.pubkey());

    // Build configure_minter instruction
    // Data: discriminator + allowance (u64)
    let allowance: u64 = 1_000_000_000; // 1000 tokens with 6 decimals
    let mut ix_data = get_discriminator("configure_minter").to_vec();
    ix_data.extend_from_slice(&allowance.to_le_bytes());

    let ix = Instruction {
        program_id: PROGRAM_ID,
        accounts: vec![
            AccountMeta::new(admin.pubkey(), true),           // admin
            AccountMeta::new_readonly(config_pda, false),     // config
            AccountMeta::new_readonly(minter.pubkey(), false), // minter
            AccountMeta::new(minter_config_pda, false),       // minter_config
            AccountMeta::new_readonly(system_program::id(), false), // system_program
        ],
        data: ix_data,
    };

    let tx = Transaction::new_signed_with_payer(
        &[ix],
        Some(&admin.pubkey()),
        &[&admin],
        svm.latest_blockhash(),
    );

    let result = svm.send_transaction(tx);
    assert!(result.is_ok(), "Configure minter should succeed: {:?}", result.err());

    // Verify minter config account was created
    let minter_config_account = svm.get_account(&minter_config_pda);
    assert!(minter_config_account.is_some(), "Minter config account should exist");
}

#[test]
fn test_configure_minter_unauthorized() {
    let mut svm = setup_svm();

    let admin = Keypair::new();
    let unauthorized = Keypair::new();
    svm.airdrop(&admin.pubkey(), LAMPORTS_PER_SOL * 10).unwrap();
    svm.airdrop(&unauthorized.pubkey(), LAMPORTS_PER_SOL * 10).unwrap();

    // Initialize with admin
    initialize_program(&mut svm, &admin);

    let (config_pda, _) = get_config_pda();
    let minter = Keypair::new();
    let (minter_config_pda, _) = get_minter_config_pda(&minter.pubkey());

    let allowance: u64 = 1_000_000_000;
    let mut ix_data = get_discriminator("configure_minter").to_vec();
    ix_data.extend_from_slice(&allowance.to_le_bytes());

    // Try to configure minter with unauthorized user
    let ix = Instruction {
        program_id: PROGRAM_ID,
        accounts: vec![
            AccountMeta::new(unauthorized.pubkey(), true),    // unauthorized user trying to be admin
            AccountMeta::new_readonly(config_pda, false),
            AccountMeta::new_readonly(minter.pubkey(), false),
            AccountMeta::new(minter_config_pda, false),
            AccountMeta::new_readonly(system_program::id(), false),
        ],
        data: ix_data,
    };

    let tx = Transaction::new_signed_with_payer(
        &[ix],
        Some(&unauthorized.pubkey()),
        &[&unauthorized],
        svm.latest_blockhash(),
    );

    let result = svm.send_transaction(tx);
    assert!(result.is_err(), "Unauthorized configure_minter should fail");
}

#[test]
fn test_update_minter_allowance() {
    let mut svm = setup_svm();

    let admin = Keypair::new();
    svm.airdrop(&admin.pubkey(), LAMPORTS_PER_SOL * 10).unwrap();

    initialize_program(&mut svm, &admin);

    let (config_pda, _) = get_config_pda();
    let minter = Keypair::new();
    let (minter_config_pda, _) = get_minter_config_pda(&minter.pubkey());

    // First configure with initial allowance
    let allowance1: u64 = 1_000_000_000;
    let mut ix_data1 = get_discriminator("configure_minter").to_vec();
    ix_data1.extend_from_slice(&allowance1.to_le_bytes());

    let ix1 = Instruction {
        program_id: PROGRAM_ID,
        accounts: vec![
            AccountMeta::new(admin.pubkey(), true),
            AccountMeta::new_readonly(config_pda, false),
            AccountMeta::new_readonly(minter.pubkey(), false),
            AccountMeta::new(minter_config_pda, false),
            AccountMeta::new_readonly(system_program::id(), false),
        ],
        data: ix_data1,
    };

    let tx1 = Transaction::new_signed_with_payer(
        &[ix1],
        Some(&admin.pubkey()),
        &[&admin],
        svm.latest_blockhash(),
    );
    svm.send_transaction(tx1).expect("First configure should succeed");

    // Update with new allowance
    let allowance2: u64 = 2_000_000_000;
    let mut ix_data2 = get_discriminator("configure_minter").to_vec();
    ix_data2.extend_from_slice(&allowance2.to_le_bytes());

    let ix2 = Instruction {
        program_id: PROGRAM_ID,
        accounts: vec![
            AccountMeta::new(admin.pubkey(), true),
            AccountMeta::new_readonly(config_pda, false),
            AccountMeta::new_readonly(minter.pubkey(), false),
            AccountMeta::new(minter_config_pda, false),
            AccountMeta::new_readonly(system_program::id(), false),
        ],
        data: ix_data2,
    };

    let tx2 = Transaction::new_signed_with_payer(
        &[ix2],
        Some(&admin.pubkey()),
        &[&admin],
        svm.latest_blockhash(),
    );

    let result = svm.send_transaction(tx2);
    assert!(result.is_ok(), "Update minter allowance should succeed: {:?}", result.err());
}

// ============================================================================
// Remove Minter Tests
// ============================================================================

fn configure_minter(svm: &mut LiteSVM, admin: &Keypair, minter: &Pubkey, allowance: u64) {
    let (config_pda, _) = get_config_pda();
    let (minter_config_pda, _) = get_minter_config_pda(minter);

    let mut ix_data = get_discriminator("configure_minter").to_vec();
    ix_data.extend_from_slice(&allowance.to_le_bytes());

    let ix = Instruction {
        program_id: PROGRAM_ID,
        accounts: vec![
            AccountMeta::new(admin.pubkey(), true),
            AccountMeta::new_readonly(config_pda, false),
            AccountMeta::new_readonly(*minter, false),
            AccountMeta::new(minter_config_pda, false),
            AccountMeta::new_readonly(system_program::id(), false),
        ],
        data: ix_data,
    };

    let tx = Transaction::new_signed_with_payer(
        &[ix],
        Some(&admin.pubkey()),
        &[admin],
        svm.latest_blockhash(),
    );

    svm.send_transaction(tx).expect("Configure minter should succeed");
}

#[test]
fn test_remove_minter() {
    let mut svm = setup_svm();

    let admin = Keypair::new();
    svm.airdrop(&admin.pubkey(), LAMPORTS_PER_SOL * 10).unwrap();

    initialize_program(&mut svm, &admin);

    let minter = Keypair::new();
    configure_minter(&mut svm, &admin, &minter.pubkey(), 1_000_000_000);

    let (config_pda, _) = get_config_pda();
    let (minter_config_pda, _) = get_minter_config_pda(&minter.pubkey());

    // Verify minter config exists
    assert!(svm.get_account(&minter_config_pda).is_some(), "Minter config should exist");

    // Remove minter
    let ix_data = get_discriminator("remove_minter");

    let ix = Instruction {
        program_id: PROGRAM_ID,
        accounts: vec![
            AccountMeta::new(admin.pubkey(), true),           // admin
            AccountMeta::new_readonly(config_pda, false),     // config
            AccountMeta::new_readonly(minter.pubkey(), false), // minter
            AccountMeta::new(minter_config_pda, false),       // minter_config (will be closed)
        ],
        data: ix_data.to_vec(),
    };

    let tx = Transaction::new_signed_with_payer(
        &[ix],
        Some(&admin.pubkey()),
        &[&admin],
        svm.latest_blockhash(),
    );

    let result = svm.send_transaction(tx);
    assert!(result.is_ok(), "Remove minter should succeed: {:?}", result.err());

    // Verify minter config was closed
    let minter_config_account = svm.get_account(&minter_config_pda);
    assert!(minter_config_account.is_none() || minter_config_account.unwrap().lamports == 0,
        "Minter config account should be closed");
}

// ============================================================================
// Mint Tokens Tests
// ============================================================================

#[test]
fn test_mint_tokens() {
    let mut svm = setup_svm();

    let admin = Keypair::new();
    let minter = Keypair::new();
    let recipient = Keypair::new();

    svm.airdrop(&admin.pubkey(), LAMPORTS_PER_SOL * 10).unwrap();
    svm.airdrop(&minter.pubkey(), LAMPORTS_PER_SOL * 10).unwrap();

    initialize_program(&mut svm, &admin);
    configure_minter(&mut svm, &admin, &minter.pubkey(), 1_000_000_000);

    let (config_pda, _) = get_config_pda();
    let (mint_pda, _) = get_mint_pda();
    let (minter_config_pda, _) = get_minter_config_pda(&minter.pubkey());
    let destination_ata = get_associated_token_address(&recipient.pubkey(), &mint_pda);

    // Mint tokens
    let mint_amount: u64 = 100_000_000; // 100 tokens
    let mut ix_data = get_discriminator("mint_tokens").to_vec();
    ix_data.extend_from_slice(&mint_amount.to_le_bytes());

    let ix = Instruction {
        program_id: PROGRAM_ID,
        accounts: vec![
            AccountMeta::new(minter.pubkey(), true),          // minter
            AccountMeta::new_readonly(config_pda, false),     // config
            AccountMeta::new(minter_config_pda, false),       // minter_config
            AccountMeta::new(mint_pda, false),                // mint
            AccountMeta::new(destination_ata, false),         // destination ATA
            AccountMeta::new_readonly(recipient.pubkey(), false), // destination_owner
            AccountMeta::new_readonly(TOKEN_PROGRAM_ID, false), // token_program
            AccountMeta::new_readonly(ASSOCIATED_TOKEN_PROGRAM_ID, false), // associated_token_program
            AccountMeta::new_readonly(system_program::id(), false), // system_program
        ],
        data: ix_data,
    };

    let tx = Transaction::new_signed_with_payer(
        &[ix],
        Some(&minter.pubkey()),
        &[&minter],
        svm.latest_blockhash(),
    );

    let result = svm.send_transaction(tx);
    assert!(result.is_ok(), "Mint tokens should succeed: {:?}", result.err());

    // Verify destination token account was created and has tokens
    let destination_account = svm.get_account(&destination_ata);
    assert!(destination_account.is_some(), "Destination token account should exist");
}

#[test]
fn test_mint_exceeds_allowance() {
    let mut svm = setup_svm();

    let admin = Keypair::new();
    let minter = Keypair::new();
    let recipient = Keypair::new();

    svm.airdrop(&admin.pubkey(), LAMPORTS_PER_SOL * 10).unwrap();
    svm.airdrop(&minter.pubkey(), LAMPORTS_PER_SOL * 10).unwrap();

    initialize_program(&mut svm, &admin);
    configure_minter(&mut svm, &admin, &minter.pubkey(), 100_000_000); // 100 token allowance

    let (config_pda, _) = get_config_pda();
    let (mint_pda, _) = get_mint_pda();
    let (minter_config_pda, _) = get_minter_config_pda(&minter.pubkey());
    let destination_ata = get_associated_token_address(&recipient.pubkey(), &mint_pda);

    // Try to mint more than allowance
    let mint_amount: u64 = 200_000_000; // 200 tokens (exceeds 100 allowance)
    let mut ix_data = get_discriminator("mint_tokens").to_vec();
    ix_data.extend_from_slice(&mint_amount.to_le_bytes());

    let ix = Instruction {
        program_id: PROGRAM_ID,
        accounts: vec![
            AccountMeta::new(minter.pubkey(), true),
            AccountMeta::new_readonly(config_pda, false),
            AccountMeta::new(minter_config_pda, false),
            AccountMeta::new(mint_pda, false),
            AccountMeta::new(destination_ata, false),
            AccountMeta::new_readonly(recipient.pubkey(), false),
            AccountMeta::new_readonly(TOKEN_PROGRAM_ID, false),
            AccountMeta::new_readonly(ASSOCIATED_TOKEN_PROGRAM_ID, false),
            AccountMeta::new_readonly(system_program::id(), false),
        ],
        data: ix_data,
    };

    let tx = Transaction::new_signed_with_payer(
        &[ix],
        Some(&minter.pubkey()),
        &[&minter],
        svm.latest_blockhash(),
    );

    let result = svm.send_transaction(tx);
    assert!(result.is_err(), "Mint exceeding allowance should fail");
}

#[test]
fn test_mint_unauthorized() {
    let mut svm = setup_svm();

    let admin = Keypair::new();
    let unauthorized = Keypair::new();
    let recipient = Keypair::new();

    svm.airdrop(&admin.pubkey(), LAMPORTS_PER_SOL * 10).unwrap();
    svm.airdrop(&unauthorized.pubkey(), LAMPORTS_PER_SOL * 10).unwrap();

    initialize_program(&mut svm, &admin);
    // Note: unauthorized is NOT configured as a minter

    let (config_pda, _) = get_config_pda();
    let (mint_pda, _) = get_mint_pda();
    let (minter_config_pda, _) = get_minter_config_pda(&unauthorized.pubkey());
    let destination_ata = get_associated_token_address(&recipient.pubkey(), &mint_pda);

    let mint_amount: u64 = 100_000_000;
    let mut ix_data = get_discriminator("mint_tokens").to_vec();
    ix_data.extend_from_slice(&mint_amount.to_le_bytes());

    let ix = Instruction {
        program_id: PROGRAM_ID,
        accounts: vec![
            AccountMeta::new(unauthorized.pubkey(), true),
            AccountMeta::new_readonly(config_pda, false),
            AccountMeta::new(minter_config_pda, false),
            AccountMeta::new(mint_pda, false),
            AccountMeta::new(destination_ata, false),
            AccountMeta::new_readonly(recipient.pubkey(), false),
            AccountMeta::new_readonly(TOKEN_PROGRAM_ID, false),
            AccountMeta::new_readonly(ASSOCIATED_TOKEN_PROGRAM_ID, false),
            AccountMeta::new_readonly(system_program::id(), false),
        ],
        data: ix_data,
    };

    let tx = Transaction::new_signed_with_payer(
        &[ix],
        Some(&unauthorized.pubkey()),
        &[&unauthorized],
        svm.latest_blockhash(),
    );

    let result = svm.send_transaction(tx);
    assert!(result.is_err(), "Unauthorized mint should fail");
}

// ============================================================================
// Burn Tokens Tests
// ============================================================================

fn mint_tokens(svm: &mut LiteSVM, minter: &Keypair, recipient: &Pubkey, amount: u64) {
    let (config_pda, _) = get_config_pda();
    let (mint_pda, _) = get_mint_pda();
    let (minter_config_pda, _) = get_minter_config_pda(&minter.pubkey());
    let destination_ata = get_associated_token_address(recipient, &mint_pda);

    let mut ix_data = get_discriminator("mint_tokens").to_vec();
    ix_data.extend_from_slice(&amount.to_le_bytes());

    let ix = Instruction {
        program_id: PROGRAM_ID,
        accounts: vec![
            AccountMeta::new(minter.pubkey(), true),
            AccountMeta::new_readonly(config_pda, false),
            AccountMeta::new(minter_config_pda, false),
            AccountMeta::new(mint_pda, false),
            AccountMeta::new(destination_ata, false),
            AccountMeta::new_readonly(*recipient, false),
            AccountMeta::new_readonly(TOKEN_PROGRAM_ID, false),
            AccountMeta::new_readonly(ASSOCIATED_TOKEN_PROGRAM_ID, false),
            AccountMeta::new_readonly(system_program::id(), false),
        ],
        data: ix_data,
    };

    let tx = Transaction::new_signed_with_payer(
        &[ix],
        Some(&minter.pubkey()),
        &[minter],
        svm.latest_blockhash(),
    );

    svm.send_transaction(tx).expect("Mint should succeed");
}

#[test]
fn test_burn_tokens() {
    let mut svm = setup_svm();

    let admin = Keypair::new();
    let minter = Keypair::new();
    let user = Keypair::new();

    svm.airdrop(&admin.pubkey(), LAMPORTS_PER_SOL * 10).unwrap();
    svm.airdrop(&minter.pubkey(), LAMPORTS_PER_SOL * 10).unwrap();
    svm.airdrop(&user.pubkey(), LAMPORTS_PER_SOL * 10).unwrap();

    initialize_program(&mut svm, &admin);
    configure_minter(&mut svm, &admin, &minter.pubkey(), 1_000_000_000);
    mint_tokens(&mut svm, &minter, &user.pubkey(), 100_000_000);

    let (config_pda, _) = get_config_pda();
    let (mint_pda, _) = get_mint_pda();
    let user_ata = get_associated_token_address(&user.pubkey(), &mint_pda);

    // Burn tokens
    let burn_amount: u64 = 50_000_000; // 50 tokens
    let mut ix_data = get_discriminator("burn_tokens").to_vec();
    ix_data.extend_from_slice(&burn_amount.to_le_bytes());

    let ix = Instruction {
        program_id: PROGRAM_ID,
        accounts: vec![
            AccountMeta::new_readonly(user.pubkey(), true),   // owner (signer)
            AccountMeta::new_readonly(config_pda, false),     // config
            AccountMeta::new(mint_pda, false),                // mint
            AccountMeta::new(user_ata, false),                // token_account
            AccountMeta::new_readonly(TOKEN_PROGRAM_ID, false), // token_program
        ],
        data: ix_data,
    };

    let tx = Transaction::new_signed_with_payer(
        &[ix],
        Some(&user.pubkey()),
        &[&user],
        svm.latest_blockhash(),
    );

    let result = svm.send_transaction(tx);
    assert!(result.is_ok(), "Burn tokens should succeed: {:?}", result.err());
}

#[test]
fn test_burn_more_than_balance() {
    let mut svm = setup_svm();

    let admin = Keypair::new();
    let minter = Keypair::new();
    let user = Keypair::new();

    svm.airdrop(&admin.pubkey(), LAMPORTS_PER_SOL * 10).unwrap();
    svm.airdrop(&minter.pubkey(), LAMPORTS_PER_SOL * 10).unwrap();
    svm.airdrop(&user.pubkey(), LAMPORTS_PER_SOL * 10).unwrap();

    initialize_program(&mut svm, &admin);
    configure_minter(&mut svm, &admin, &minter.pubkey(), 1_000_000_000);
    mint_tokens(&mut svm, &minter, &user.pubkey(), 100_000_000); // 100 tokens

    let (config_pda, _) = get_config_pda();
    let (mint_pda, _) = get_mint_pda();
    let user_ata = get_associated_token_address(&user.pubkey(), &mint_pda);

    // Try to burn more than balance
    let burn_amount: u64 = 200_000_000; // 200 tokens (only have 100)
    let mut ix_data = get_discriminator("burn_tokens").to_vec();
    ix_data.extend_from_slice(&burn_amount.to_le_bytes());

    let ix = Instruction {
        program_id: PROGRAM_ID,
        accounts: vec![
            AccountMeta::new_readonly(user.pubkey(), true),
            AccountMeta::new_readonly(config_pda, false),
            AccountMeta::new(mint_pda, false),
            AccountMeta::new(user_ata, false),
            AccountMeta::new_readonly(TOKEN_PROGRAM_ID, false),
        ],
        data: ix_data,
    };

    let tx = Transaction::new_signed_with_payer(
        &[ix],
        Some(&user.pubkey()),
        &[&user],
        svm.latest_blockhash(),
    );

    let result = svm.send_transaction(tx);
    assert!(result.is_err(), "Burn more than balance should fail");
}

// ============================================================================
// Pause/Unpause Tests
// ============================================================================

#[test]
fn test_pause() {
    let mut svm = setup_svm();

    let admin = Keypair::new();
    svm.airdrop(&admin.pubkey(), LAMPORTS_PER_SOL * 10).unwrap();

    initialize_program(&mut svm, &admin);

    let (config_pda, _) = get_config_pda();

    // Pause
    let ix_data = get_discriminator("pause");

    let ix = Instruction {
        program_id: PROGRAM_ID,
        accounts: vec![
            AccountMeta::new_readonly(admin.pubkey(), true), // admin
            AccountMeta::new(config_pda, false),             // config
        ],
        data: ix_data.to_vec(),
    };

    let tx = Transaction::new_signed_with_payer(
        &[ix],
        Some(&admin.pubkey()),
        &[&admin],
        svm.latest_blockhash(),
    );

    let result = svm.send_transaction(tx);
    assert!(result.is_ok(), "Pause should succeed: {:?}", result.err());
}

#[test]
fn test_pause_unauthorized() {
    let mut svm = setup_svm();

    let admin = Keypair::new();
    let unauthorized = Keypair::new();
    svm.airdrop(&admin.pubkey(), LAMPORTS_PER_SOL * 10).unwrap();
    svm.airdrop(&unauthorized.pubkey(), LAMPORTS_PER_SOL * 10).unwrap();

    initialize_program(&mut svm, &admin);

    let (config_pda, _) = get_config_pda();

    let ix_data = get_discriminator("pause");

    let ix = Instruction {
        program_id: PROGRAM_ID,
        accounts: vec![
            AccountMeta::new_readonly(unauthorized.pubkey(), true),
            AccountMeta::new(config_pda, false),
        ],
        data: ix_data.to_vec(),
    };

    let tx = Transaction::new_signed_with_payer(
        &[ix],
        Some(&unauthorized.pubkey()),
        &[&unauthorized],
        svm.latest_blockhash(),
    );

    let result = svm.send_transaction(tx);
    assert!(result.is_err(), "Unauthorized pause should fail");
}

fn pause_program(svm: &mut LiteSVM, admin: &Keypair) {
    let (config_pda, _) = get_config_pda();

    let ix_data = get_discriminator("pause");

    let ix = Instruction {
        program_id: PROGRAM_ID,
        accounts: vec![
            AccountMeta::new_readonly(admin.pubkey(), true),
            AccountMeta::new(config_pda, false),
        ],
        data: ix_data.to_vec(),
    };

    let tx = Transaction::new_signed_with_payer(
        &[ix],
        Some(&admin.pubkey()),
        &[admin],
        svm.latest_blockhash(),
    );

    svm.send_transaction(tx).expect("Pause should succeed");
}

#[test]
fn test_mint_when_paused() {
    let mut svm = setup_svm();

    let admin = Keypair::new();
    let minter = Keypair::new();
    let recipient = Keypair::new();

    svm.airdrop(&admin.pubkey(), LAMPORTS_PER_SOL * 10).unwrap();
    svm.airdrop(&minter.pubkey(), LAMPORTS_PER_SOL * 10).unwrap();

    initialize_program(&mut svm, &admin);
    configure_minter(&mut svm, &admin, &minter.pubkey(), 1_000_000_000);
    pause_program(&mut svm, &admin);

    let (config_pda, _) = get_config_pda();
    let (mint_pda, _) = get_mint_pda();
    let (minter_config_pda, _) = get_minter_config_pda(&minter.pubkey());
    let destination_ata = get_associated_token_address(&recipient.pubkey(), &mint_pda);

    // Try to mint when paused
    let mint_amount: u64 = 100_000_000;
    let mut ix_data = get_discriminator("mint_tokens").to_vec();
    ix_data.extend_from_slice(&mint_amount.to_le_bytes());

    let ix = Instruction {
        program_id: PROGRAM_ID,
        accounts: vec![
            AccountMeta::new(minter.pubkey(), true),
            AccountMeta::new_readonly(config_pda, false),
            AccountMeta::new(minter_config_pda, false),
            AccountMeta::new(mint_pda, false),
            AccountMeta::new(destination_ata, false),
            AccountMeta::new_readonly(recipient.pubkey(), false),
            AccountMeta::new_readonly(TOKEN_PROGRAM_ID, false),
            AccountMeta::new_readonly(ASSOCIATED_TOKEN_PROGRAM_ID, false),
            AccountMeta::new_readonly(system_program::id(), false),
        ],
        data: ix_data,
    };

    let tx = Transaction::new_signed_with_payer(
        &[ix],
        Some(&minter.pubkey()),
        &[&minter],
        svm.latest_blockhash(),
    );

    let result = svm.send_transaction(tx);
    assert!(result.is_err(), "Mint when paused should fail");
}

#[test]
fn test_unpause() {
    let mut svm = setup_svm();

    let admin = Keypair::new();
    svm.airdrop(&admin.pubkey(), LAMPORTS_PER_SOL * 10).unwrap();

    initialize_program(&mut svm, &admin);
    pause_program(&mut svm, &admin);

    let (config_pda, _) = get_config_pda();

    // Unpause
    let ix_data = get_discriminator("unpause");

    let ix = Instruction {
        program_id: PROGRAM_ID,
        accounts: vec![
            AccountMeta::new_readonly(admin.pubkey(), true),
            AccountMeta::new(config_pda, false),
        ],
        data: ix_data.to_vec(),
    };

    let tx = Transaction::new_signed_with_payer(
        &[ix],
        Some(&admin.pubkey()),
        &[&admin],
        svm.latest_blockhash(),
    );

    let result = svm.send_transaction(tx);
    assert!(result.is_ok(), "Unpause should succeed: {:?}", result.err());
}

#[test]
fn test_mint_after_unpause() {
    let mut svm = setup_svm();

    let admin = Keypair::new();
    let minter = Keypair::new();
    let recipient = Keypair::new();

    svm.airdrop(&admin.pubkey(), LAMPORTS_PER_SOL * 10).unwrap();
    svm.airdrop(&minter.pubkey(), LAMPORTS_PER_SOL * 10).unwrap();

    initialize_program(&mut svm, &admin);
    configure_minter(&mut svm, &admin, &minter.pubkey(), 1_000_000_000);
    pause_program(&mut svm, &admin);

    // Unpause
    let (config_pda, _) = get_config_pda();
    let unpause_ix_data = get_discriminator("unpause");
    let unpause_ix = Instruction {
        program_id: PROGRAM_ID,
        accounts: vec![
            AccountMeta::new_readonly(admin.pubkey(), true),
            AccountMeta::new(config_pda, false),
        ],
        data: unpause_ix_data.to_vec(),
    };
    let unpause_tx = Transaction::new_signed_with_payer(
        &[unpause_ix],
        Some(&admin.pubkey()),
        &[&admin],
        svm.latest_blockhash(),
    );
    svm.send_transaction(unpause_tx).expect("Unpause should succeed");

    // Now mint should work
    let (mint_pda, _) = get_mint_pda();
    let (minter_config_pda, _) = get_minter_config_pda(&minter.pubkey());
    let destination_ata = get_associated_token_address(&recipient.pubkey(), &mint_pda);

    let mint_amount: u64 = 100_000_000;
    let mut mint_ix_data = get_discriminator("mint_tokens").to_vec();
    mint_ix_data.extend_from_slice(&mint_amount.to_le_bytes());

    let mint_ix = Instruction {
        program_id: PROGRAM_ID,
        accounts: vec![
            AccountMeta::new(minter.pubkey(), true),
            AccountMeta::new_readonly(config_pda, false),
            AccountMeta::new(minter_config_pda, false),
            AccountMeta::new(mint_pda, false),
            AccountMeta::new(destination_ata, false),
            AccountMeta::new_readonly(recipient.pubkey(), false),
            AccountMeta::new_readonly(TOKEN_PROGRAM_ID, false),
            AccountMeta::new_readonly(ASSOCIATED_TOKEN_PROGRAM_ID, false),
            AccountMeta::new_readonly(system_program::id(), false),
        ],
        data: mint_ix_data,
    };

    let mint_tx = Transaction::new_signed_with_payer(
        &[mint_ix],
        Some(&minter.pubkey()),
        &[&minter],
        svm.latest_blockhash(),
    );

    let result = svm.send_transaction(mint_tx);
    assert!(result.is_ok(), "Mint after unpause should succeed: {:?}", result.err());
}

// ============================================================================
// Integration Tests
// ============================================================================

#[test]
fn test_full_stablecoin_flow() {
    let mut svm = setup_svm();

    let admin = Keypair::new();
    let minter = Keypair::new();
    let user1 = Keypair::new();
    let user2 = Keypair::new();

    svm.airdrop(&admin.pubkey(), LAMPORTS_PER_SOL * 10).unwrap();
    svm.airdrop(&minter.pubkey(), LAMPORTS_PER_SOL * 10).unwrap();
    svm.airdrop(&user1.pubkey(), LAMPORTS_PER_SOL * 10).unwrap();
    svm.airdrop(&user2.pubkey(), LAMPORTS_PER_SOL * 10).unwrap();

    // 1. Initialize
    initialize_program(&mut svm, &admin);

    // 2. Configure minter
    configure_minter(&mut svm, &admin, &minter.pubkey(), 1_000_000_000);

    // 3. Mint to user1
    mint_tokens(&mut svm, &minter, &user1.pubkey(), 100_000_000);

    // 4. Mint to user2
    mint_tokens(&mut svm, &minter, &user2.pubkey(), 200_000_000);

    // 5. User1 burns some tokens
    let (config_pda, _) = get_config_pda();
    let (mint_pda, _) = get_mint_pda();
    let user1_ata = get_associated_token_address(&user1.pubkey(), &mint_pda);

    let burn_amount: u64 = 50_000_000;
    let mut burn_ix_data = get_discriminator("burn_tokens").to_vec();
    burn_ix_data.extend_from_slice(&burn_amount.to_le_bytes());

    let burn_ix = Instruction {
        program_id: PROGRAM_ID,
        accounts: vec![
            AccountMeta::new_readonly(user1.pubkey(), true),
            AccountMeta::new_readonly(config_pda, false),
            AccountMeta::new(mint_pda, false),
            AccountMeta::new(user1_ata, false),
            AccountMeta::new_readonly(TOKEN_PROGRAM_ID, false),
        ],
        data: burn_ix_data,
    };

    let burn_tx = Transaction::new_signed_with_payer(
        &[burn_ix],
        Some(&user1.pubkey()),
        &[&user1],
        svm.latest_blockhash(),
    );
    assert!(svm.send_transaction(burn_tx).is_ok(), "Burn should succeed");

    // 6. Pause and unpause
    pause_program(&mut svm, &admin);

    let unpause_ix_data = get_discriminator("unpause");
    let unpause_ix = Instruction {
        program_id: PROGRAM_ID,
        accounts: vec![
            AccountMeta::new_readonly(admin.pubkey(), true),
            AccountMeta::new(config_pda, false),
        ],
        data: unpause_ix_data.to_vec(),
    };

    let unpause_tx = Transaction::new_signed_with_payer(
        &[unpause_ix],
        Some(&admin.pubkey()),
        &[&admin],
        svm.latest_blockhash(),
    );
    assert!(svm.send_transaction(unpause_tx).is_ok(), "Unpause should succeed");

    // 7. Remove minter
    let (minter_config_pda, _) = get_minter_config_pda(&minter.pubkey());

    let remove_minter_ix_data = get_discriminator("remove_minter");
    let remove_minter_ix = Instruction {
        program_id: PROGRAM_ID,
        accounts: vec![
            AccountMeta::new(admin.pubkey(), true),
            AccountMeta::new_readonly(config_pda, false),
            AccountMeta::new_readonly(minter.pubkey(), false),
            AccountMeta::new(minter_config_pda, false),
        ],
        data: remove_minter_ix_data.to_vec(),
    };

    let remove_minter_tx = Transaction::new_signed_with_payer(
        &[remove_minter_ix],
        Some(&admin.pubkey()),
        &[&admin],
        svm.latest_blockhash(),
    );
    assert!(svm.send_transaction(remove_minter_tx).is_ok(), "Remove minter should succeed");
}

#[test]
fn test_multiple_minters() {
    let mut svm = setup_svm();

    let admin = Keypair::new();
    let minter1 = Keypair::new();
    let minter2 = Keypair::new();
    let user = Keypair::new();

    svm.airdrop(&admin.pubkey(), LAMPORTS_PER_SOL * 10).unwrap();
    svm.airdrop(&minter1.pubkey(), LAMPORTS_PER_SOL * 10).unwrap();
    svm.airdrop(&minter2.pubkey(), LAMPORTS_PER_SOL * 10).unwrap();

    initialize_program(&mut svm, &admin);

    // Configure two minters with different allowances
    configure_minter(&mut svm, &admin, &minter1.pubkey(), 500_000_000);
    configure_minter(&mut svm, &admin, &minter2.pubkey(), 1_000_000_000);

    // Both minters mint to the same user
    mint_tokens(&mut svm, &minter1, &user.pubkey(), 100_000_000);
    mint_tokens(&mut svm, &minter2, &user.pubkey(), 200_000_000);

    // Verify user received tokens from both minters
    let (mint_pda, _) = get_mint_pda();
    let user_ata = get_associated_token_address(&user.pubkey(), &mint_pda);
    let user_token_account = svm.get_account(&user_ata);
    assert!(user_token_account.is_some(), "User should have token account");
}
