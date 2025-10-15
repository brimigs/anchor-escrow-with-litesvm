use anchor_litesvm::AnchorLiteSVM;
use litesvm_utils::{AssertionHelpers, TestHelpers};
use solana_sdk::{
    signature::{read_keypair_file, Signer},
    system_program,
};
use spl_associated_token_account::get_associated_token_address;
use litesvm_token::spl_token;

// Generate client modules from the program using declare_program!
// This creates the `anchor_escrow::client::accounts` and `anchor_escrow::client::args` modules
anchor_lang::declare_program!(anchor_escrow);

#[test]
fn test_make_and_take_with_utils() {
    // ============================================================================
    // 1. ONE-LINE INITIALIZATION - Initialize AnchorLiteSVM with escrow program
    // ============================================================================
    let program_keypair = read_keypair_file("target/deploy/anchor_escrow-keypair.json").unwrap();
    let program_id = program_keypair.pubkey();

    let mut ctx = AnchorLiteSVM::build_with_program(
        program_id,
        include_bytes!("../target/deploy/anchor_escrow.so"),
    );

    // ============================================================================
    // 2. CREATE TEST ACCOUNTS - Using helper methods
    // ============================================================================
    let maker = ctx.svm.create_funded_account(10_000_000_000).unwrap(); // 10 SOL
    let taker = ctx.svm.create_funded_account(10_000_000_000).unwrap(); // 10 SOL

    // ============================================================================
    // 3. CREATE TOKEN MINTS AND FUNDED TOKEN ACCOUNTS - Using helper methods
    // ============================================================================
    let mint_a = ctx.svm.create_token_mint(&maker, 9).unwrap();
    let mint_b = ctx.svm.create_token_mint(&maker, 9).unwrap();

    // Maker's account for mint_a (will deposit into escrow)
    let maker_ata_a = ctx.svm
        .create_associated_token_account(&mint_a.pubkey(), &maker)
        .unwrap();
    ctx.svm
        .mint_to(&mint_a.pubkey(), &maker_ata_a, &maker, 1_000_000_000)
        .unwrap(); // 1.0 tokens

    // Taker's account for mint_b (will send to maker)
    let taker_ata_b = ctx.svm
        .create_associated_token_account(&mint_b.pubkey(), &taker)
        .unwrap();
    ctx.svm
        .mint_to(&mint_b.pubkey(), &taker_ata_b, &maker, 500_000_000)
        .unwrap(); // 0.5 tokens

    // ============================================================================
    // 4. BUILD "MAKE" INSTRUCTION - Production-compatible syntax
    //    - no manual discriminator handling 
    //    - no manual AccountMeta construction 
    //    - no manual transaction creation 
    //    - accounts do NOT need to be passed in a specific order 
    //    - no need to manually serialize instruction args 
    //    - type safe & syntax is compatible with anchor-client for easy migration     
    // ============================================================================
    let seed: u64 = 42;
    let escrow_pda = ctx.svm.get_pda(
        &[b"escrow", maker.pubkey().as_ref(), &seed.to_le_bytes()],
        &program_id,
    );
    let vault = get_associated_token_address(&escrow_pda, &mint_a.pubkey());

    let make_ix = ctx.program()
        .accounts(anchor_escrow::client::accounts::Make {
            maker: maker.pubkey(),
            escrow: escrow_pda,
            mint_a: mint_a.pubkey(),
            mint_b: mint_b.pubkey(),
            maker_ata_a,
            vault,
            associated_token_program: spl_associated_token_account::id(),
            token_program: spl_token::id(),
            system_program: system_program::id(),
        })
        .args(anchor_escrow::client::args::Make {
            seed,
            receive: 500_000_000,  // 0.5 tokens
            amount: 1_000_000_000, // 1 token
        })
        .instruction()
        .unwrap();

    // ============================================================================
    // 5. EXECUTE AND VERIFY "MAKE" - Single line execution with assertion
    // ============================================================================
    ctx.execute_instruction(make_ix, &[&maker])
        .unwrap()
        .assert_success();

    // Verify escrow was created and tokens were transferred
    assert!(ctx.account_exists(&escrow_pda), "Escrow account should exist");
    ctx.svm.assert_token_balance(&vault, 1_000_000_000);
    ctx.svm.assert_token_balance(&maker_ata_a, 0);

    // ============================================================================
    // 8. BUILD "TAKE" INSTRUCTION - Get token accounts for taker and maker
    // ============================================================================
    let taker_ata_a = get_associated_token_address(&taker.pubkey(), &mint_a.pubkey());
    let maker_ata_b = get_associated_token_address(&maker.pubkey(), &mint_b.pubkey());

    let take_ix = ctx.program()
        .accounts(anchor_escrow::client::accounts::Take {
            taker: taker.pubkey(),
            maker: maker.pubkey(),
            escrow: escrow_pda,
            mint_a: mint_a.pubkey(),
            mint_b: mint_b.pubkey(),
            vault,
            taker_ata_a,
            taker_ata_b,
            maker_ata_b,
            associated_token_program: spl_associated_token_account::id(),
            token_program: spl_token::id(),
            system_program: system_program::id(),
        })
        .args(anchor_escrow::client::args::Take {})
        .instruction()
        .unwrap();

    // ============================================================================
    // 9. EXECUTE AND VERIFY "TAKE" - Single line execution with assertion
    // ============================================================================
    let result = ctx.execute_instruction(take_ix, &[&taker]).unwrap();
    result.assert_success();

    println!("\nTake transaction succeeded!");
    println!("Transaction logs:");
    for log in result.logs() {
        println!("  {}", log);
    }

    // ============================================================================
    // 10. VERIFY FINAL STATE - chainable assertions 
    // ============================================================================

    // Verify accounts were closed
    ctx.svm.assert_account_closed(&escrow_pda);
    ctx.svm.assert_account_closed(&vault);

    // Verify token balances after the swap
    ctx.svm.assert_token_balance(&taker_ata_a, 1_000_000_000); // Taker received mint_a tokens
    ctx.svm.assert_token_balance(&taker_ata_b, 0);             // Taker sent all mint_b tokens
    ctx.svm.assert_token_balance(&maker_ata_b, 500_000_000);   // Maker received mint_b tokens

    println!("\nEscrow take flow test passed successfully!");
}
