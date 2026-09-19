# Quarkdown `.env` clean-room evidence

Issue: #190

This note records independently authored black-box observations used to bound
Arkst's deterministic environment-input contract. It does not copy or
translate Quarkdown implementation source.

## Reference artifacts

The disposable probe was PR #374 and was closed without merge.

| Target | Official Linux x64 archive SHA-256 |
| --- | --- |
| Quarkdown v2.5.1 | `5751ab608fcb4daa2ec857a3368c029beed5429554ae0bdd95c660b2706269e9` |
| Quarkdown v2.6.0 | `5b015e47c820d06ff6774eb700e60d77bc575819d4a0140cc7f1a115e7ce6dc4` |

The exact official release artifacts were downloaded in the disposable GitHub
Actions probe and verified against those pinned digests before execution.

## Observed contract

Both target versions produced the same relevant behavior:

1. Without explicit process permission, `.env` is denied before variable
   presence matters.
   - normal mode reports/materializes a missing-permission error while the CLI
     process itself exits successfully;
   - strict mode exits with code 72;
   - the diagnostic explicitly identifies the required `process` permission
     and points to `--allow process`.
2. `quarkdown compile --help` exposes repeatable
   `--allow=(...|process|all)` and `--deny=(...|process|all)`.
3. With `--allow process`, a present environment variable evaluates to the
   exact host-provided String value and `.isnone` is false.
4. With `--allow process`, an absent environment variable evaluates to
   `none`; `.isnone` is true and comparison with `none` is true.
5. The allowed present/absent cases succeed in both normal and strict modes.

The probe used a unique present variable with value
`qd190-present-value` and a separate deliberately-unset variable. The
rendered captured value was exactly `qd190-present-value` in both v2.5.1 and
v2.6.0.

## Excluded observations

Two experimental fixture forms were deliberately not used as semantic proof:

- square-bracket text such as `direct:[.env {...}]` remained literal output
  in the tested grammar position;
- an equality probe using a quoted literal produced a false result and was not
  treated as evidence for String equality because the literal form itself was
  not independently established for that value boundary.

The implementation therefore relies only on the validated permission,
present-String, and absent-None observations above.

## Arkst contract derived from the evidence

Arkst does **not** reproduce the upstream ambient process read. Instead it
preserves the observable language contract behind an explicit deterministic
capability boundary:

- `EnvironmentInputs` is an immutable caller-supplied map for one evaluation;
- supplying it authorizes `.env` and defines the complete visible environment;
- a present key returns the exact injected `IrValue::String`;
- an absent key returns `IrValue::None`;
- omitting the snapshot fails closed with source-backed `E3004`;
- an explicit empty snapshot is authorized and every lookup is absent;
- no production core/evaluator environment path calls `std::env` or falls
  back to cwd/process state; test-only oracle harnesses are not runtime
  authority;
- repeated evaluations can use different snapshots without hidden global state;
- public WASM/embedder exposure remains #191-owned.

The ordinary core `compile(...)` path deliberately injects no environment
authority. Hosts that intentionally provide deterministic values use
`compile_with_environment(...)` or the combined explicit capability API.
