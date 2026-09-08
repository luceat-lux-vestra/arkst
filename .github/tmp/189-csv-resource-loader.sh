#!/usr/bin/env bash
set -euo pipefail

python3 <<'PY'
from pathlib import Path

def replace_once(path: str, old: str, new: str) -> None:
    p = Path(path)
    text = p.read_text()
    count = text.count(old)
    if count != 1:
        raise SystemExit(f"{path}: expected one anchor, got {count}: {old[:80]!r}")
    p.write_text(text.replace(old, new))

replace_once(
    'crates/arkst-engine/src/lib.rs',
    'pub mod builtins;\npub mod evaluator;\n',
    'pub mod builtins;\npub mod csv_resource;\npub mod evaluator;\n',
)

adapter = Path('crates/arkst-core/src/engine_adapter.rs')
text = adapter.read_text()
if not text.endswith('\n}\n'):
    raise SystemExit('engine_adapter.rs module terminator changed')
test = r'''

    #[test]
    fn csv_resource_loader_reuses_virtual_project_source_relative_text_and_boundaries() {
        let project = VirtualProjectBuilder::new()
            .entry("docs/main.qd")
            .expect("valid entry")
            .add_source("docs/main.qd", "root")
            .expect("valid source")
            .add_source("docs/parts/child.qd", "child")
            .expect("valid source")
            .add_asset(
                "docs/parts/data/table.csv",
                b"name,note\r\nalpha,\"one,two\"\r\n".to_vec(),
            )
            .expect("valid CSV asset")
            .add_asset("docs/parts/data/invalid.csv", vec![0xff, 0xfe])
            .expect("valid binary asset")
            .build()
            .expect("valid project");
        let child = source_id(&project, "docs/parts/child.qd");
        let provider = VirtualProjectResourceProvider::new(&project);

        let csv = arkst_engine::csv_resource::load_csv_resource(
            &provider,
            child,
            "data/./table.csv",
        )
        .expect("nested source-relative CSV resolves");
        assert_eq!(csv.path, "docs/parts/data/table.csv");
        assert_eq!(csv.headers, ["name", "note"]);
        assert_eq!(
            csv.rows,
            [vec!["alpha".to_string(), "one,two".to_string()]]
        );

        assert!(matches!(
            arkst_engine::csv_resource::load_csv_resource(
                &provider,
                child,
                "data/invalid.csv"
            ),
            Err(arkst_engine::csv_resource::CsvResourceError::Resource(
                ResourceAccessError::InvalidUtf8 { path, .. }
            )) if path == "docs/parts/data/invalid.csv"
        ));
        assert!(matches!(
            arkst_engine::csv_resource::load_csv_resource(&provider, child, "/tmp/x.csv"),
            Err(arkst_engine::csv_resource::CsvResourceError::Resource(
                ResourceAccessError::UnsupportedReference { .. }
            ))
        ));
        assert!(matches!(
            arkst_engine::csv_resource::load_csv_resource(
                &provider,
                child,
                "../../../outside.csv"
            ),
            Err(arkst_engine::csv_resource::CsvResourceError::Resource(
                ResourceAccessError::Boundary { .. }
            ))
        ));
    }
'''
text = text[:-3] + test + '}\n'
adapter.write_text(text)

research = Path('docs/research/quarkdown-project-data-189.md')
text = research.read_text()
old_heading = '## Implemented bounded slices: `.filename` and `.listfiles`'
if text.count(old_heading) != 1:
    raise SystemExit('research heading anchor changed')
text = text.replace(
    old_heading,
    '## Implemented bounded slices: `.filename`, `.listfiles`, and the CSV resource prerequisite',
    1,
)
old = 'This record pins the bounded #189 implementation slices against Quarkdown v2.5.1 at\n`107ec3a9482f10d6f90d7580f8409b46a719d18e`. The remaining #189 families `.csv` and `.bibliography` are intentionally not claimed here.'
new = 'This record pins the bounded #189 implementation slices against Quarkdown v2.5.1 at\n`107ec3a9482f10d6f90d7580f8409b46a719d18e`. The language-level `.csv` table producer and `.bibliography` output family remain intentionally unclaimed; this record now also covers the internal CSV resource/parser prerequisite consumed by #183.'
if text.count(old) != 1:
    raise SystemExit('research intro anchor changed')
text = text.replace(old, new, 1)
appendix = '''

## Bounded CSV resource/parser prerequisite

Pinned Quarkdown `Data.kt` calls `csvReader().open(file)` and
`readAllWithHeaderAsSequence()` before any table construction. The pinned stdlib
build uses `com.github.doyaaaaaken:kotlin-csv-jvm:1.10.0`. Its default reader is
UTF-8 with comma delimiter, `"` quote/escape, `skipEmptyLine=false`, duplicate
header rejection, and ERROR behavior for both insufficient and excess fields.
The parser recognizes LF, CR/CRLF, U+2028, U+2029, and U+0085 record terminators;
terminators inside quoted fields remain cell data, doubled quotes decode to one
quote, and a leading U+FEFF is ignored at a record parser start.

`arkst-engine::csv_resource::load_csv_resource` is the #189-owned platform-neutral
prerequisite only. It obtains validated text exclusively through
`ResourceProvider::read_text`, retains the provider's canonical logical path,
and returns raw ordered headers/rows. It deliberately does **not** trim headers
or cells: pinned Quarkdown applies `trim()` later while creating `Table::Cell`.
Likewise PLAIN/MARKDOWN cell transformation, caption/ref binding, the Table node,
and renderer behavior remain #183-owned rather than being duplicated here.

The bounded parser proves the pinned default header/row shape, quoted comma and
multiline fields, doubled quotes, empty/trailing fields, duplicate-header and
row-width failures, BOM handling, and the pinned line-terminator set. It fails
closed on malformed quote states instead of promising bug-for-bug exception text
or edge behavior not independently established. `ResourceProvider::read_text`
continues to reject invalid UTF-8; Java decoder replacement behavior is therefore
not claimed as parity. Those residual parser/decoder details remain #189 debt,
while the public `.csv` callable/table producer remains `UNSUPPORTED` until #183
consumes this prerequisite.

No dependency, `Cargo.lock`, host filesystem, cwd, native path, timestamp,
process, environment, or network capability is introduced. The same loader can
consume any future WASM provider implementing the existing text contract, but
public native/WASM resource binding parity remains #191 and is not claimed here.
'''
if '## Bounded CSV resource/parser prerequisite' in text:
    raise SystemExit('research CSV section already present')
research.write_text(text.rstrip() + appendix + '\n')

manifest = Path('docs/compatibility/quarkdown/FILESYSTEM_PROJECT_DATA_RESOURCES_AUDIT_MANIFEST.tsv')
lines = manifest.read_text().splitlines()
matches = [i for i, line in enumerate(lines) if line.startswith('owned\tbuiltin:.csv\t')]
if len(matches) != 1:
    raise SystemExit(f'expected one csv manifest row, got {len(matches)}')
i = matches[0]
fields = lines[i].split('\t')
if len(fields) != 24 or fields[17] != 'UNSUPPORTED' or fields[21] != '#189;#183':
    raise SystemExit(f'unexpected pre-change csv row: {fields}')
fields[13] = 'Parser call; no evaluator csv dispatch; arkst-engine::csv_resource::load_csv_resource consumes ResourceProvider::read_text and parses raw ordered headers/rows; no IR table producer; #183 is the table consumer; no Markdown/WASM/CLI language implementation.'
fields[14] = 'The internal parsed resource retains the provider canonical logical path plus raw ordered header/cell strings; it is not a typed table value and exposes no host identity.'
fields[15] = 'No host FS enters core/engine; VirtualProject text resolution remains source-relative and project-bounded; invalid UTF-8 fails closed through the existing provider rather than inheriting Java decoder replacement behavior.'
fields[16] = 'crates/arkst-engine/tests/csv_resource_contract.rs; crates/arkst-engine/src/csv_resource.rs; crates/arkst-core/src/engine_adapter.rs nested source-relative/invalid-UTF8/boundary unit evidence; docs/research/quarkdown-project-data-189.md; current stdlib handoff in STDLIB_BUILTINS_AUDIT_MANIFEST.tsv'
fields[18] = 'The internal #189 resource prerequisite source-relative reads validated UTF-8 CSV through ResourceProvider and parses pinned default raw header/row syntax, quoted/multiline fields, doubled quotes, BOM/terminators, and deterministic duplicate-header/row-width failures. No public .csv callable/table behavior is represented.'
fields[19] = 'No public .csv evaluator dispatch, PLAIN/MARKDOWN mode transform, trim-at-table-stage behavior, caption/ref binding, Table producer/output, Java UTF-8 replacement parity, exact upstream exception text, or malformed-edge bug-for-bug parity is represented.'
fields[20] = 'The bounded #189 resource/parser prerequisite is present, but residual decoder/parser parity remains #189; table producer/output and mode/caption/ref consumption remain #183.'
fields[22] = 'Owned data-file loader prerequisite is bounded and platform-neutral; canonical language support stays UNSUPPORTED until the #183 table producer consumes it.'
fields[23] = 'The loader itself consumes the existing in-memory ResourceProvider text contract and adds no host API; public WASM resource binding/end-to-end parity remains #191 outside this row, and no .csv WASM language surface exists.'
lines[i] = '\t'.join(fields)
manifest.write_text('\n'.join(lines) + '\n')

audit = Path('docs/compatibility/quarkdown/FILESYSTEM_PROJECT_DATA_RESOURCES_AUDIT.md')
text = audit.read_text()
old = '- `.csv`, `.bibliography`, and `.env` remain `UNSUPPORTED`; `.filename` and `.listfiles` are bounded `PARTIAL` file-identity/directory-listing slices. The manifest states each absent contract and assigns its bounded\n  follow-up; absence is not inferred merely from a missing high-level test.'
new = '- `.csv`, `.bibliography`, and `.env` remain `UNSUPPORTED`; `.filename` and `.listfiles` are bounded `PARTIAL` file-identity/directory-listing slices. `.csv` now has a #189-owned internal source-relative CSV resource/parser prerequisite, but no evaluator dispatch, mode/caption/ref consumption, Table node, or output claim; those language/table layers remain absent under #183. The manifest states each absent contract and assigns its bounded\n  follow-up; absence is not inferred merely from a missing high-level test.'
if text.count(old) != 1:
    raise SystemExit('audit status anchor changed')
text = text.replace(old, new, 1)
old = '   `ResourceProvider` plus the separate `LoadableLibraryProvider`. `.read` and `.json`\n   read text; `.filename` resolves existing source/asset logical metadata without'
new = '   `ResourceProvider` plus the separate `LoadableLibraryProvider`. `.read` and `.json`\n   read text; the internal #189 CSV prerequisite reuses the same `read_text` authority and parses raw ordered headers/rows without constructing a Table; `.filename` resolves existing source/asset logical metadata without'
if text.count(old) != 1:
    raise SystemExit('audit resource-model anchor changed')
text = text.replace(old, new, 1)
old = 'NAME parity remains fail-closed rather than approximated. Data-format errors\nand inaccessible-host-file distinctions are likewise not silently borrowed\nfrom the host and remain bounded follow-up work.'
new = 'NAME parity remains fail-closed rather than approximated. The internal CSV prerequisite now gives malformed quotes, duplicate headers, and row-width mismatches deterministic typed failures over validated UTF-8 provider text; exact upstream exception text, Java decoder replacement behavior, and malformed-edge bug-for-bug parity remain bounded #189 work. Inaccessible-host-file distinctions are likewise not silently borrowed\nfrom the host and remain bounded follow-up work.'
if text.count(old) != 1:
    raise SystemExit('audit boundary anchor changed')
text = text.replace(old, new, 1)
audit.write_text(text)

audit_test = Path('crates/arkst-core/tests/filesystem_project_data_resources_audit.rs')
text = audit_test.read_text()
anchor = '    assert!(!filename[9].contains("may still return a name"));\n'
addition = '''    let csv = row(&rows, "builtin:.csv");
    assert_eq!(csv[17], "UNSUPPORTED");
    assert!(csv[13].contains("csv_resource::load_csv_resource"));
    assert!(csv[14].contains("raw ordered header/cell"));
    assert!(csv[16].contains("csv_resource_contract.rs"));
    assert!(csv[18].contains("internal #189 resource prerequisite"));
    assert!(csv[18].contains("No public .csv callable/table behavior"));
    assert!(csv[19].contains("No public .csv evaluator dispatch"));
    assert!(csv[19].contains("Java UTF-8 replacement parity"));
    assert!(csv[20].contains("#189"));
    assert!(csv[20].contains("#183"));
    assert_eq!(csv[21], "#189;#183");
'''
if text.count(anchor) != 1:
    raise SystemExit('audit-test csv insertion anchor changed')
text = text.replace(anchor, anchor + addition, 1)
audit_test.write_text(text)

reconciliation = Path('crates/arkst-core/tests/quarkdown_v251_reconciliation.rs')
text = reconciliation.read_text()
anchor = '    assert_eq!(listfiles[21], "#189;POLICY_DIVERGENCE:global-read;#191");\n\n'
addition = '''    let csv = resource_row("builtin:.csv");
    assert_eq!(csv[17], "UNSUPPORTED");
    assert!(csv[13].contains("csv_resource::load_csv_resource"));
    assert!(csv[16].contains("csv_resource_contract.rs"));
    assert!(csv[18].contains("internal #189 resource prerequisite"));
    assert!(csv[19].contains("No public .csv evaluator dispatch"));
    assert_eq!(csv[21], "#189;#183");
    assert!(PROJECT_DATA_RESEARCH.contains("Bounded CSV resource/parser prerequisite"));
    assert!(PROJECT_DATA_RESEARCH.contains("kotlin-csv-jvm:1.10.0"));
    assert!(PROJECT_DATA_RESEARCH.contains("does **not** trim headers"));

'''
if text.count(anchor) != 1:
    raise SystemExit('reconciliation csv insertion anchor changed')
text = text.replace(anchor, anchor + addition, 1)
reconciliation.write_text(text)
PY

cargo fmt --all
cargo fmt --all --check
git diff --check
test -z "$(git diff -- Cargo.lock)"
! grep -Eq 'std::fs|std::path|PathBuf|std::env|std::process|TcpStream|UdpSocket' crates/arkst-engine/src/csv_resource.rs

cargo test --locked -p arkst-engine --test csv_resource_contract
cargo test --locked -p arkst-engine
cargo test --locked -p arkst-core --lib
cargo test --locked -p arkst-core --test filesystem_project_data_resources_audit
cargo test --locked -p arkst-core --test quarkdown_v251_reconciliation
cargo clippy --locked --workspace --all-targets --all-features -- -D warnings

test -z "$(git diff -- Cargo.lock)"
rm .github/workflows/tmp-189-csv-resource-loader.yml
rm .github/tmp/189-csv-resource-loader.sh

actual="$(git diff origin/main --name-only | sort)"
expected="$(printf '%s\n' \
  crates/arkst-core/src/engine_adapter.rs \
  crates/arkst-core/tests/filesystem_project_data_resources_audit.rs \
  crates/arkst-core/tests/quarkdown_v251_reconciliation.rs \
  crates/arkst-engine/src/csv_resource.rs \
  crates/arkst-engine/src/lib.rs \
  crates/arkst-engine/tests/csv_resource_contract.rs \
  docs/compatibility/quarkdown/FILESYSTEM_PROJECT_DATA_RESOURCES_AUDIT.md \
  docs/compatibility/quarkdown/FILESYSTEM_PROJECT_DATA_RESOURCES_AUDIT_MANIFEST.tsv \
  docs/research/quarkdown-project-data-189.md | sort)"
test "$actual" = "$expected"

git add -A
git diff --cached --check
git config user.name github-actions[bot]
git config user.email 41898282+github-actions[bot]@users.noreply.github.com
git commit -m 'compat(resource): complete CSV loader proof tree' -m 'Refs #189'
git push origin HEAD:compat/189-csv-resource-loader
