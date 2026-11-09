# margin-code's fixture vault

`vault/` is a vault written by margin-code's own code: two code reviews in a thread it made, with every kind of
comment. `CONTENTS.md` (margin's own) says what each file shows. Ledger's contract test,
`src-tauri/src/tests_margin_fixture.rs`, loads it and checks that Ledger reads each file as margin-code means it, and
never rewrites it.

Both are copied verbatim from [openledger-nl/margin](https://github.com/openledger-nl/margin),
`code/test/fixtures/` at commit `37681b8`. Don't edit them here: they are margin-code's output, and the test is only
worth something while they stay that.

The files are LF, byte for byte; `.gitattributes` keeps git from converting them on checkout.

## Refreshing it

When margin-code writes its fixture differently (`pnpm fixture` in margin's `code/`), copy it again from a margin
checkout and name the new commit above:

```bash
rm -r src-tauri/tests/fixtures/margin/vault
cp -r ../margin/code/test/fixtures/vault ../margin/code/test/fixtures/CONTENTS.md src-tauri/tests/fixtures/margin/
cargo test --manifest-path src-tauri/Cargo.toml tests_margin_fixture
```

Then bring the test's assertions in line with the new `CONTENTS.md`.
