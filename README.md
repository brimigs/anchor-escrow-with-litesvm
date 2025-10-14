# Anchor Escrow Program LiteSVM Example

This repository is an example of how to test an anchor program with the litesvm testing library. The test file is heavily commented to help explain how to correctly write these tests.

This example uses an escrow program to showcase using both the `litesvm` crate and the `litesvm-token` crate.

For full litesvm documentation, go to [litesvm.com](https://www.litesvm.com/)

To run litesvm tests:

```shell
anchor build
cargo test
```

These tests cover:

- Litesvm test environment setup
- Building anchor program instructions
- Building and sending transactions
- Creating and funding accounts
- Creating token accounts and ATAs
- Creating mint accounts
- Minting tokens
- Transferring tokens
