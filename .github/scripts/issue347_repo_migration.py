from pathlib import Path
import re


def replace_once(path, old, new):
    p = Path(path)
    s = p.read_text()
    n = s.count(old)
    if n != 1:
        raise SystemExit(f"{path}: expected 1 occurrence, got {n}: {old[:100]!r}")
    p.write_text(s.replace(old, new, 1))


def find_matching_paren(text, open_index):
    depth = 0
    i = open_index
    state = "code"
    raw_hashes = 0
    while i < len(text):
        c = text[i]
        n = text[i + 1] if i + 1 < len(text) else ""
        if state == "code":
            if c == '"':
                # Rust raw string r###"..."###
                j = i - 1
                hashes = 0
                while j >= 0 and text[j] == '#':
                    hashes += 1
                    j -= 1
                if j >= 0 and text[j] == 'r':
                    state = "raw"
                    raw_hashes = hashes
                else:
                    state = "string"
            elif c == "'":
                state = "char"
            elif c == '/' and n == '/':
                state = "line_comment"
                i += 1
            elif c == '/' and n == '*':
                state = "block_comment"
                i += 1
            elif c == '(':
                depth += 1
            elif c == ')':
                depth -= 1
                if depth == 0:
                    return i
        elif state == "string":
            if c == '\\':
                i += 1
            elif c == '"':
                state = "code"
        elif state == "char":
            if c == '\\':
                i += 1
            elif c == "'":
                state = "code"
        elif state == "raw":
            if c == '"' and text.startswith('#' * raw_hashes, i + 1):
                i += raw_hashes
                state = "code"
        elif state == "line_comment":
            if c == '\n':
                state = "code"
        elif state == "block_comment":
            if c == '*' and n == '/':
                state = "code"
                i += 1
        i += 1
    raise SystemExit("unbalanced parenthesis while migrating Typst backend call")


def add_target_argument(text, method, expected_at_least=0):
    needle = f".{method}("
    cursor = 0
    changed = 0
    while True:
        pos = text.find(needle, cursor)
        if pos < 0:
            break
        open_index = pos + len(needle) - 1
        close_index = find_matching_paren(text, open_index)
        args = text[open_index + 1:close_index]
        if "TypstTarget::" not in args:
            text = text[:close_index] + ", TypstTarget::Pdf" + text[close_index:]
            changed += 1
            cursor = close_index + len(", TypstTarget::Pdf") + 1
        else:
            cursor = close_index + 1
    if changed < expected_at_least:
        raise SystemExit(f"expected at least {expected_at_least} {method} migrations, got {changed}")
    return text, changed


def ensure_target_import(text):
    if "TypstTarget" in text:
        return text
    marker = "use arkst_typst::"
    idx = text.find(marker)
    if idx < 0:
        raise SystemExit("file uses Typst backend but has no arkst_typst import anchor")
    line_end = text.find("\n", idx)
    return text[:line_end + 1] + "use arkst_typst::TypstTarget;\n" + text[line_end + 1:]

# Keep a compatibility accessor derived from the typed artifact set; this is
# not a parallel optional output field.
replace_once(
    "crates/arkst-typst/src/backend.rs",
    '''impl TypstOutput {
    pub fn single_artifact(&self) -> Option<&TypstArtifact> {''',
    '''impl TypstOutput {
    /// Borrow the single PDF artifact when this result is a PDF compilation.
    pub fn pdf(&self) -> Option<&[u8]> {
        (self.target == TypstTarget::Pdf)
            .then(|| self.single_artifact())
            .flatten()
            .map(|artifact| artifact.bytes.as_slice())
    }

    pub fn single_artifact(&self) -> Option<&TypstArtifact> {''',
)

files = [
    "crates/arkst-typst-subprocess/src/lib.rs",
    "crates/arkst-typst-subprocess/tests/backend_integration.rs",
    "crates/arkst-typst-subprocess/tests/code_callouts_integration.rs",
    "crates/arkst-typst-subprocess/examples/measure_subprocess.rs",
    "crates/arkst-typst-inprocess/tests/auto_page_break.rs",
    "crates/arkst-typst-inprocess/tests/backend_integration.rs",
    "crates/arkst-typst-inprocess/tests/backend_parity.rs",
    "crates/arkst-typst-inprocess/examples/measure_inprocess.rs",
    "crates/arkst-cli/src/commands.rs",
]

for path in files:
    p = Path(path)
    text = p.read_text()
    original = text
    text, compile_changes = add_target_argument(text, "compile")
    text, source_map_changes = add_target_argument(text, "compile_with_source_map")
    # Migrate only TypstOutput field reads. BackendObservation.pdf and unrelated
    # domain fields are intentionally untouched.
    text = re.sub(r"\boutput\.pdf\b(?!\()", "output.pdf()", text)
    text = re.sub(r"\btypst_output\.pdf\b(?!\()", "typst_output.pdf()", text)
    if compile_changes or source_map_changes or text != original:
        text = ensure_target_import(text)
    p.write_text(text)
