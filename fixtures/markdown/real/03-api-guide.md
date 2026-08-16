# API guide

The API accepts a document and produces a stable intermediate result.

| Request | Response | Notes |
| :--- | :---: | ---: |
| `parse` | `Document` | Reads Markdown |
| `compile` | `IrDocument` | Runs evaluation |

## Example request

```rust json
let document = parse(source)?;
let result = compile(document)?;
```

The important fields are:

- `source`
  - retains Unicode text;
  - retains line-ending boundaries.
- `diagnostics`
  - identify unsupported output paths;
  - carry source-backed locations.
