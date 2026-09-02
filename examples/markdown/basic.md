# Arkst Markdown example

This document exercises the Markdown path that Arkst currently lowers through Typst.

## Text and inline structure

Plain text can contain *emphasis*, **strong text**, ~~strikethrough~~, and `inline code`.

A normal link points to the [Arkst repository](https://github.com/luceat-lux-vestra/arkst), and an autolink works too: <https://example.com>.

> Blockquotes preserve nested Markdown structure.
>
> - including lists
> - and **inline formatting**

## Lists

1. Ordered lists preserve their starting order.
2. Items may contain nested content.

- Unordered lists work too.
  - Nested list items are retained.

## Code

```rust
fn main() {
    println!("hello from Arkst");
}
```

    let indented = "code block";

---

This line ends with a hard break.  
This text follows it.

This source line has a soft break
before this continuation.
