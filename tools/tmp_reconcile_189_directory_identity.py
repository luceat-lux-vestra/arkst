from pathlib import Path


def replace_once(text: str, old: str, new: str, label: str) -> str:
    count = text.count(old)
    if count != 1:
        raise SystemExit(f"{label}: expected one anchor, got {count}")
    return text.replace(old, new)


manifest_path = Path("docs/compatibility/quarkdown/FILESYSTEM_PROJECT_DATA_RESOURCES_AUDIT_MANIFEST.tsv")
lines = manifest_path.read_text().splitlines()
out = []
seen = set()
for line in lines:
    if line.startswith("owned\tbuiltin:.listfiles\t"):
        row = line.split("\t")
        if len(row) != 24 or row[17] != "PARTIAL":
            raise SystemExit("listfiles row shape/status drift")
        row[8] = replace_once(
            row[8],
            "Arkst infers only non-empty logical directories from immutable source/asset path prefixes",
            "Arkst derives logical directories from immutable source/asset path prefixes plus explicit logical directory identities",
            "listfiles nested normalization",
        )
        row[9] = replace_once(
            row[9],
            "empty directories are not representable in the current VirtualProject, and host permission failures remain outside the bounded subset.",
            "explicit empty directories are representable in VirtualProject, while host permission failures remain outside the bounded subset.",
            "listfiles missing/empty behavior",
        )
        row[14] = replace_once(
            row[14],
            "files/assets use canonical VirtualPath identity and non-empty directory identity is inferred from descendants.",
            "files/assets use canonical VirtualPath identity; directory identity is explicit or inferred from descendants.",
            "listfiles identity wording",
        )
        row[18] = replace_once(
            row[18],
            "Source-relative non-empty logical directories support",
            "Source-relative logical directories, including explicit empty directories, support",
            "listfiles supported subset",
        )
        if "empty-directory" in row[19]:
            raise SystemExit("listfiles empty-directory incorrectly remains unresolved")
        if not all(token in row[21] for token in ("#189", "#191", "POLICY_DIVERGENCE:global-read")):
            raise SystemExit("listfiles follow-up ownership drift")
        seen.add("listfiles")
        out.append("\t".join(row))
    elif line.startswith("owned\tbuiltin:.filename\t"):
        row = line.split("\t")
        if len(row) != 24 or row[17] != "PARTIAL":
            raise SystemExit("filename row shape/status drift")
        row[13] = replace_once(
            row[13],
            "VirtualProject logical source/asset existence",
            "VirtualProject logical source/asset/directory existence",
            "filename implementation path",
        )
        row[16] += "; crates/arkst-project/tests/logical_directory_listing_189.rs directory-identity metadata evidence"
        row[18] = replace_once(
            row[18],
            "Existing source or asset resources resolve source-relative through VirtualProject",
            "Existing source, asset, or logical-directory resources resolve source-relative through VirtualProject",
            "filename supported subset",
        )
        if row[21] != "POLICY_DIVERGENCE:global-read;#191":
            raise SystemExit("filename follow-up ownership drift")
        seen.add("filename")
        out.append("\t".join(row))
    else:
        out.append(line)
if seen != {"listfiles", "filename"}:
    raise SystemExit(f"missing canonical rows: {seen}")
manifest_path.write_text("\n".join(out) + "\n")

# Research text must describe metadata identity consistently for .filename.
path = Path("docs/research/quarkdown-project-data-189.md")
text = path.read_text()
text = replace_once(
    text,
    "bytes. `VirtualProjectResourceProvider` accepts both source and asset stores, so\na binary asset remains a valid file identity and no UTF-8 conversion is\nperformed merely to obtain its name.",
    "bytes. `VirtualProjectResourceProvider` accepts source, asset, and logical-directory\nidentity, so binary assets and explicit or inferred directories remain valid\nidentity targets without UTF-8 conversion merely to obtain a name.",
    "research filename identity scope",
)
path.write_text(text)

# Canonical narrative must not contradict the shared metadata contract.
path = Path("docs/compatibility/quarkdown/FILESYSTEM_PROJECT_DATA_RESOURCES_AUDIT.md")
text = path.read_text()
text = replace_once(
    text,
    "`.filename` resolves existing source/asset logical metadata without\n   reading resource bytes;",
    "`.filename` resolves existing source/asset/directory logical metadata without\n   reading resource bytes;",
    "audit filename metadata scope",
)
path.write_text(text)

# Permanent guard pins the absence of the stale pre-empty-directory claims.
path = Path("crates/arkst-core/tests/filesystem_project_data_resources_audit.rs")
text = path.read_text()
anchor = '''    assert!(listfiles[18].contains("empty-directory"));
    assert!(listfiles[19].contains("last-modified"));
'''
replacement = '''    assert!(listfiles[18].contains("empty-directory"));
    assert!(!listfiles[8].contains("non-empty"));
    assert!(!listfiles[9].contains("not representable"));
    assert!(!listfiles[14].contains("non-empty"));
    assert!(!listfiles[18].contains("non-empty"));
    assert!(listfiles[19].contains("last-modified"));
'''
text = replace_once(text, anchor, replacement, "listfiles permanent stale-claim guard")
anchor = '''    assert!(filename[18].contains("binary assets"));
    assert!(filename[19].contains("global-read"));
'''
replacement = '''    assert!(filename[18].contains("binary assets"));
    assert!(filename[13].contains("directory"));
    assert!(filename[18].contains("logical-directory"));
    assert!(filename[19].contains("global-read"));
'''
text = replace_once(text, anchor, replacement, "filename directory identity guard")
path.write_text(text)
