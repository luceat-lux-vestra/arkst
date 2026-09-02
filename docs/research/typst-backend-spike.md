# Typst Backend Spike

## Objective

Verify that the official Typst compiler can compile a basic document via
subprocess, producing valid PDF output.

## Test

```bash
echo '= Hello, Arkst' > /tmp/arkst-spike.typ
echo 'This is a *paragraph* from the Typst spike.' >> /tmp/arkst-spike.typ
typst compile /tmp/arkst-spike.typ /tmp/arkst-spike.pdf
```

## Result

- **Typst version:** 0.15.1
- **Compile time:** ~1.8s cold start
- **Output:** Valid PDF, version 1.7, 1 page, 11,048 bytes
- **Diagnostics:** None (compilation succeeded)
- **Subprocess overhead:** minimal

## Conclusion

Subprocess backend is viable for M1. The `TypstBackend` trait abstraction
in `arkst-typst` isolates the subprocess call details.

## Next

- Implement proper subprocess backend that captures stdout/stderr
- Parse Typst JSON diagnostic output format
- Evaluate in-process embedding feasibility before v0.1