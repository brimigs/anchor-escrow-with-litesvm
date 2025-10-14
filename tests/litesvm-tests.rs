use litesvm::LiteSVM;
use solana_sdk::{
    instruction::{AccountMeta, Instruction},
    pubkey::Pubkey,
    signature::{Keypair, Signer, read_keypair_file},
    system_program,
    transaction::Transaction,
};
use litesvm_token::{
    spl_token::{self, native_mint::DECIMALS},
    CreateAssociatedTokenAccount, CreateMint, MintTo,
};
use spl_associated_token_account::get_associated_token_address;
use sha2::{Sha256, Digest};

#[derive(Debug)]
struct MakeArgs {
    seed: u64,
    receive: u64,
    amount: u64,
}

#[test]
fn test_make_and_take_with_litesvm() {
    // ============================================================================
    // Test Env Setup: Initialize environment and deploy escrow program
    // ============================================================================
    let mut svm = LiteSVM::new();

    let program_keypair = read_keypair_file("target/deploy/anchor_escrow-keypair.json").unwrap();
    let program_id = program_keypair.pubkey();
    let program_bytes = include_bytes!("../target/deploy/anchor_escrow.so");
    svm.add_program(program_id, program_bytes);

    // ============================================================================
    // Create and fund test accounts
    // ============================================================================
    let maker = Keypair::new();
    let taker = Keypair::new();
    svm.airdrop(&maker.pubkey(), 10_000_000_000).unwrap(); // 10 SOL
    svm.airdrop(&taker.pubkey(), 10_000_000_000).unwrap(); // 10 SOL

    // ============================================================================
    // Token Setup: Create mints and token accounts
    // Token swap flow: Maker offers mint_a tokens, wants mint_b tokens in return
    // ============================================================================

    // Create two token mints with maker as authority
    let mint_a = CreateMint::new(&mut svm, &maker)
        .authority(&maker.pubkey())
        .decimals(DECIMALS)
        .send()
        .unwrap();

    let mint_b = CreateMint::new(&mut svm, &maker)
        .authority(&maker.pubkey())
        .decimals(DECIMALS)
        .send()
        .unwrap();

    // Create all associated token accounts upfront for clarity
    // Maker's account for mint_a (will deposit into escrow)
    let maker_ata_a = CreateAssociatedTokenAccount::new(&mut svm, &maker, &mint_a)
        .owner(&maker.pubkey())
        .send()
        .unwrap();

    // Taker's account for mint_b (will send to maker)
    let taker_ata_b = CreateAssociatedTokenAccount::new(&mut svm, &taker, &mint_b)
        .owner(&taker.pubkey())
        .send()
        .unwrap();

    // Taker's account for mint_a (will receive from escrow)
    let taker_ata_a = CreateAssociatedTokenAccount::new(&mut svm, &taker, &mint_a)
        .owner(&taker.pubkey())
        .send()
        .unwrap();

    // Maker's account for mint_b (will receive from taker)
    let maker_ata_b = CreateAssociatedTokenAccount::new(&mut svm, &maker, &mint_b)
        .owner(&maker.pubkey())
        .send()
        .unwrap();

    // Mint initial token balances using litesvm-token MintTo builder
    MintTo::new(&mut svm, &maker, &mint_a, &maker_ata_a, 1_000_000_000) // 1.0 tokens
        .send()
        .unwrap();

    MintTo::new(&mut svm, &maker, &mint_b, &taker_ata_b, 500_000_000) // 0.5 tokens
        .send()
        .unwrap();

    // ============================================================================
    // Test: Execute the "make" instruction to create escrow
    // ============================================================================
    let seed: u64 = 42;
    let (escrow_pda, _bump) = Pubkey::find_program_address(
        &[b"escrow", maker.pubkey().as_ref(), &seed.to_le_bytes()],
        &program_id,
    );

    let vault = get_associated_token_address(&escrow_pda, &mint_a);

    // Build make instruction discriminator
    let mut hasher = Sha256::new();
    hasher.update(b"global:make");
    let hash = hasher.finalize();
    let mut make_discriminator = [0u8; 8];
    make_discriminator.copy_from_slice(&hash[..8]);

    // Serialize make instruction arguments
    let make_args = MakeArgs {
        seed,
        receive: 500_000_000, // 0.5 tokens
        amount: 1_000_000_000, // 1 token
    };

    let mut make_instruction_data = make_discriminator.to_vec();
    make_instruction_data.extend_from_slice(&seed.to_le_bytes());
    make_instruction_data.extend_from_slice(&make_args.receive.to_le_bytes());
    make_instruction_data.extend_from_slice(&make_args.amount.to_le_bytes());

    // Build the make instruction
    // NOTE: ORDER MATTERS!! The accounts must be listed in the correct order for the transaction of the instruction to successfully execute 
    let make_instruction = Instruction {
        program_id,
        accounts: vec![
            AccountMeta::new(maker.pubkey(), true),  // maker
            AccountMeta::new(escrow_pda, false),      // escrow
            AccountMeta::new_readonly(mint_a, false), // mint_a
            AccountMeta::new_readonly(mint_b, false), // mint_b
            AccountMeta::new(maker_ata_a, false),     // maker_ata_a
            AccountMeta::new(vault, false),           // vault
            AccountMeta::new_readonly(spl_associated_token_account::id(), false), // associated_token_program
            AccountMeta::new_readonly(spl_token::id(), false), // token_program
            AccountMeta::new_readonly(system_program::id(), false), // system_program
        ],
        data: make_instruction_data,
    };

    // Send make transaction
    let tx = Transaction::new_signed_with_payer(
        &[make_instruction],
        Some(&maker.pubkey()),
        &[&maker],
        svm.latest_blockhash(),
    );
    svm.send_transaction(tx).unwrap();

    println!("Escrow created successfully");

    // Now test the take instruction
    // Note: taker_ata_a and maker_ata_b were already created earlier

    // Build take instruction discriminator
    let mut hasher = Sha256::new();
    hasher.update(b"global:take");
    let hash = hasher.finalize();
    let mut take_discriminator = [0u8; 8];
    take_discriminator.copy_from_slice(&hash[..8]);

    // Take instruction has no arguments, just the discriminator
    let take_instruction_data = take_discriminator.to_vec();

    // Build the take instruction with all required accounts
    let take_instruction = Instruction {
        program_id,
        accounts: vec![
            AccountMeta::new(taker.pubkey(), true),   // taker
            AccountMeta::new(maker.pubkey(), false),  // maker
            AccountMeta::new(escrow_pda, false),      // escrow
            AccountMeta::new_readonly(mint_a, false), // mint_a
            AccountMeta::new_readonly(mint_b, false), // mint_b
            AccountMeta::new(vault, false),           // vault
            AccountMeta::new(taker_ata_a, false),     // taker_ata_a
            AccountMeta::new(taker_ata_b, false),     // taker_ata_b
            AccountMeta::new(maker_ata_b, false),     // maker_ata_b
            AccountMeta::new_readonly(spl_associated_token_account::id(), false), // associated_token_program
            AccountMeta::new_readonly(spl_token::id(), false), // token_program
            AccountMeta::new_readonly(system_program::id(), false), // system_program
        ],
        data: take_instruction_data,
    };

    // Build and send take transaction
    let tx = Transaction::new_signed_with_payer(
        &[take_instruction],
        Some(&taker.pubkey()),
        &[&taker],
        svm.latest_blockhash(),
    );

    // Execute and verify
    let result = svm.send_transaction(tx);

    match result {
        Ok(res) => {
            println!("\nTake transaction succeeded!");

            println!("\nTransaction logs:");
            for log in &res.logs {
                println!("  {}", log);
            }

            // Verify escrow account was closed
            // In LiteSVM, closed accounts might still exist with 0 lamports and 0 data
            let escrow_closed = match svm.get_account(&escrow_pda) {
                None => true,
                Some(account) => account.lamports == 0 && account.data.is_empty(),
            };
            assert!(escrow_closed, "Escrow account should be closed (0 lamports, 0 data)");
            println!("\nEscrow account closed successfully");

            // Verify vault account was closed
            let vault_closed = match svm.get_account(&vault) {
                None => true,
                Some(account) => account.lamports == 0 && account.data.is_empty(),
            };
            assert!(vault_closed, "Vault account should be closed (0 lamports, 0 data)");
            println!("Vault account closed successfully");

            // Check final token balances
            // Taker should have received tokens from mint_a
            let taker_ata_a_state = litesvm_token::get_spl_account::<spl_token::state::Account>(&svm, &taker_ata_a).unwrap();
            assert_eq!(taker_ata_a_state.amount, 1_000_000_000, "Taker should have received 1 token from mint_a");
            println!("Taker received {} tokens from mint_a", taker_ata_a_state.amount as f64 / 1_000_000_000.0);

            // Taker should have sent tokens from mint_b
            let taker_ata_b_state = litesvm_token::get_spl_account::<spl_token::state::Account>(&svm, &taker_ata_b).unwrap();
            assert_eq!(taker_ata_b_state.amount, 0, "Taker should have sent all tokens from mint_b");
            println!("Taker has {} tokens from mint_b (after sending)", taker_ata_b_state.amount);

            // Maker should have received tokens from mint_b
            let maker_ata_b_state = litesvm_token::get_spl_account::<spl_token::state::Account>(&svm, &maker_ata_b).unwrap();
            assert_eq!(maker_ata_b_state.amount, 500_000_000, "Maker should have received 0.5 tokens from mint_b");
            println!("Maker received {} tokens from mint_b", maker_ata_b_state.amount as f64 / 1_000_000_000.0);

            println!("\nTake instruction test passed successfully!");
        }
        Err(e) => {
            panic!("Take transaction failed: {:?}", e);
        }
    }
}