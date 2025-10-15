/// Verification test to demonstrate and prove anchor-litesvm functionality
///
/// This test explicitly shows:
/// 1. Type-safe account structs (auto-generated from IDL)
/// 2. Automatic discriminator calculation
/// 3. Automatic Borsh serialization
/// 4. Account ordering handled by ToAccountMetas
/// 5. Production-compatible syntax
use anchor_litesvm::AnchorLiteSVM;
use litesvm_utils::{AssertionHelpers, TestHelpers};
use solana_sdk::{
    signature::{read_keypair_file, Signer},
    system_program,
};
use spl_associated_token_account::get_associated_token_address;
use litesvm_token::spl_token;
use sha2::{Sha256, Digest};

// Generate client modules from the program using declare_program!
anchor_lang::declare_program!(anchor_escrow);

#[test]
fn verify_type_safe_api() {
    println!("\n============================================================");
    println!("VERIFICATION TEST: Type-Safe Auto-Generated API");
    println!("============================================================\n");

    // Setup
    let program_keypair = read_keypair_file("target/deploy/anchor_escrow-keypair.json").unwrap();
    let program_id = program_keypair.pubkey();

    let mut ctx = AnchorLiteSVM::build_with_program(
        program_id,
        include_bytes!("../target/deploy/anchor_escrow.so"),
    );

    let maker = ctx.svm.create_funded_account(10_000_000_000).unwrap();
    let mint_a = ctx.svm.create_token_mint(&maker, 9).unwrap();
    let mint_b = ctx.svm.create_token_mint(&maker, 9).unwrap();
    let maker_ata_a = ctx.svm.create_associated_token_account(&mint_a.pubkey(), &maker).unwrap();
    ctx.svm.mint_to(&mint_a.pubkey(), &maker_ata_a, &maker, 1_000_000_000).unwrap();

    let seed: u64 = 42;
    let escrow_pda = ctx.svm.get_pda(
        &[b"escrow", maker.pubkey().as_ref(), &seed.to_le_bytes()],
        &program_id,
    );
    let vault = get_associated_token_address(&escrow_pda, &mint_a.pubkey());

    println!("✓ Test setup complete\n");

    // ============================================================================
    // VERIFICATION 1: Type-Safe Account Structs
    // ============================================================================
    println!("📋 VERIFICATION 1: Type-Safe Account Structs");
    println!("─────────────────────────────────────────────");

    // This struct is AUTO-GENERATED from the IDL by declare_program!
    // The compiler verifies all fields match the program's requirements
    let accounts = anchor_escrow::client::accounts::Make {
        maker: maker.pubkey(),
        escrow: escrow_pda,
        mint_a: mint_a.pubkey(),
        mint_b: mint_b.pubkey(),
        maker_ata_a,
        vault,
        associated_token_program: spl_associated_token_account::id(),
        token_program: spl_token::id(),
        system_program: system_program::id(),
    };

    println!("✓ Type-safe account struct created (compiler-verified)");
    println!("  - All fields present: ✓");
    println!("  - Correct types: ✓");
    println!("  - Implements ToAccountMetas trait: ✓\n");

    // ============================================================================
    // VERIFICATION 2: Auto-Generated Args with Type Safety
    // ============================================================================
    println!("📋 VERIFICATION 2: Type-Safe Arguments");
    println!("─────────────────────────────────────");

    // This struct is also AUTO-GENERATED from the IDL
    let args = anchor_escrow::client::args::Make {
        seed,
        receive: 500_000_000_u64,  // Type enforced by compiler
        amount: 1_000_000_000_u64, // Wrong type = compile error!
    };

    println!("✓ Type-safe args struct created");
    println!("  - seed: {}", args.seed);
    println!("  - receive: {}", args.receive);
    println!("  - amount: {}\n", args.amount);

    // ============================================================================
    // VERIFICATION 3: Automatic Discriminator Calculation
    // ============================================================================
    println!("📋 VERIFICATION 3: Automatic Discriminator");
    println!("───────────────────────────────────────────");

    // Calculate what the discriminator SHOULD be (Anchor standard)
    let mut hasher = Sha256::new();
    hasher.update(b"global:make");
    let hash = hasher.finalize();
    let mut expected_discriminator = [0u8; 8];
    expected_discriminator.copy_from_slice(&hash[..8]);

    println!("Expected discriminator (SHA256('global:make')[..8]):");
    println!("  {:?}", expected_discriminator);
    println!("  Hex: {}", hex::encode(&expected_discriminator));

    // ============================================================================
    // VERIFICATION 4: Build Instruction Using Generated API
    // ============================================================================
    println!("\n📋 VERIFICATION 4: Instruction Building");
    println!("───────────────────────────────────────");

    let make_ix = ctx.program()
        .accounts(accounts)  // ToAccountMetas converts this automatically
        .args(args)          // InstructionData serializes this automatically
        .instruction()
        .unwrap();

    println!("✓ Instruction built successfully");
    println!("  - Program ID: {}", make_ix.program_id);
    println!("  - Accounts: {} accounts", make_ix.accounts.len());
    println!("  - Data length: {} bytes", make_ix.data.len());

    // Verify the discriminator is correct
    let actual_discriminator = &make_ix.data[..8];
    println!("\nActual discriminator in instruction data:");
    println!("  {:?}", actual_discriminator);
    println!("  Hex: {}", hex::encode(actual_discriminator));

    assert_eq!(actual_discriminator, expected_discriminator,
        "Discriminator mismatch! Auto-generated discriminator doesn't match expected.");
    println!("\n✅ VERIFIED: Discriminators match!");

    // ============================================================================
    // VERIFICATION 5: Account Metadata Correctness
    // ============================================================================
    println!("\n📋 VERIFICATION 5: Account Metadata");
    println!("────────────────────────────────────");

    for (i, account_meta) in make_ix.accounts.iter().enumerate() {
        println!("Account {}: {}", i, account_meta.pubkey);
        println!("  - Writable: {}", account_meta.is_writable);
        println!("  - Signer: {}", account_meta.is_signer);
    }

    // Verify maker is signer and writable
    assert!(make_ix.accounts[0].is_signer, "Maker should be signer");
    assert!(make_ix.accounts[0].is_writable, "Maker should be writable");
    println!("\n✅ VERIFIED: Account metadata is correct!");

    // ============================================================================
    // VERIFICATION 6: Execute and Verify
    // ============================================================================
    println!("\n📋 VERIFICATION 6: Execution");
    println!("────────────────────────────");

    let result = ctx.execute_instruction(make_ix, &[&maker]).unwrap();
    result.assert_success();

    println!("✓ Instruction executed successfully");
    println!("  - Compute units used: {}", result.compute_units());
    println!("  - Transaction succeeded: ✓");

    // Verify the escrow was created with correct data
    assert!(ctx.account_exists(&escrow_pda), "Escrow should exist");
    ctx.svm.assert_token_balance(&vault, 1_000_000_000);
    ctx.svm.assert_token_balance(&maker_ata_a, 0);

    println!("\n✅ VERIFIED: Escrow created correctly!");
    println!("  - Escrow account exists: ✓");
    println!("  - Vault has 1.0 tokens: ✓");
    println!("  - Maker account has 0 tokens: ✓");

    // ============================================================================
    // VERIFICATION 7: Error Handling
    // ============================================================================
    println!("\n📋 VERIFICATION 7: Type Safety Benefits");
    println!("────────────────────────────────────────");

    println!("The following would cause COMPILE ERRORS:");
    println!("  ❌ Missing required field");
    println!("  ❌ Wrong type for seed (e.g., seed: \"hello\")");
    println!("  ❌ Extra unexpected field");
    println!("  ❌ Misspelled field name");
    println!("\n✅ All caught at COMPILE TIME, not runtime!");

    println!("\n============================================================");
    println!("✅ ALL VERIFICATIONS PASSED!");
    println!("============================================================");
    println!("\n📊 Summary:");
    println!("  ✓ Type-safe account structs (auto-generated)");
    println!("  ✓ Type-safe argument structs (auto-generated)");
    println!("  ✓ Automatic discriminator calculation (SHA256)");
    println!("  ✓ Automatic Borsh serialization");
    println!("  ✓ Correct account metadata (writable/signer flags)");
    println!("  ✓ Production-compatible syntax");
    println!("  ✓ Compile-time type checking");
    println!("  ✓ Runtime execution verified");
    println!("\n🎉 The anchor-litesvm magic is REAL and WORKING!\n");
}

#[test]
fn verify_account_order_independence() {
    println!("\n============================================================");
    println!("VERIFICATION TEST: Account Order Independence");
    println!("============================================================\n");

    // Setup
    let program_keypair = read_keypair_file("target/deploy/anchor_escrow-keypair.json").unwrap();
    let program_id = program_keypair.pubkey();

    let mut ctx = AnchorLiteSVM::build_with_program(
        program_id,
        include_bytes!("../target/deploy/anchor_escrow.so"),
    );

    let maker = ctx.svm.create_funded_account(10_000_000_000).unwrap();
    let mint_a = ctx.svm.create_token_mint(&maker, 9).unwrap();
    let mint_b = ctx.svm.create_token_mint(&maker, 9).unwrap();
    let maker_ata_a = ctx.svm.create_associated_token_account(&mint_a.pubkey(), &maker).unwrap();
    ctx.svm.mint_to(&mint_a.pubkey(), &maker_ata_a, &maker, 1_000_000_000).unwrap();

    let seed: u64 = 99;
    let escrow_pda = ctx.svm.get_pda(
        &[b"escrow", maker.pubkey().as_ref(), &seed.to_le_bytes()],
        &program_id,
    );
    let vault = get_associated_token_address(&escrow_pda, &mint_a.pubkey());

    println!("📋 Testing: Can we pass accounts in any order?");
    println!("─────────────────────────────────────────────────\n");

    // With manual approach, ORDER MATTERS!
    // But with anchor-litesvm, we can pass accounts as named fields
    // and the ToAccountMetas trait handles the correct ordering!

    // Pass accounts in "random" order - still works!
    let accounts = anchor_escrow::client::accounts::Make {
        system_program: system_program::id(),          // Last in struct
        mint_b: mint_b.pubkey(),                       // Different order
        associated_token_program: spl_associated_token_account::id(),
        maker: maker.pubkey(),                         // First logically
        vault,
        token_program: spl_token::id(),
        maker_ata_a,
        escrow: escrow_pda,
        mint_a: mint_a.pubkey(),
    };

    println!("✓ Accounts defined in non-sequential order");
    println!("✓ ToAccountMetas trait will reorder them correctly\n");

    let make_ix = ctx.program()
        .accounts(accounts)
        .args(anchor_escrow::client::args::Make {
            amount: 1_000_000_000,     // Order doesn't matter here either!
            seed,
            receive: 500_000_000,
        })
        .instruction()
        .unwrap();

    // Execute - if accounts weren't in correct order, this would fail!
    let result = ctx.execute_instruction(make_ix, &[&maker]).unwrap();
    result.assert_success();

    println!("✅ VERIFIED: Account order independence!");
    println!("  - Accounts passed in arbitrary order: ✓");
    println!("  - ToAccountMetas reordered correctly: ✓");
    println!("  - Instruction executed successfully: ✓");
    println!("\n🎉 No need to remember account ordering!\n");
}
