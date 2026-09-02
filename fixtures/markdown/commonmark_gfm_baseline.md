# Arkst Markdown baseline

Setext heading
--------------

A representative paragraph with *emphasis*, **strong**, ~~strikethrough~~,
`inline code`, [a link](https://example.com/docs "Example title"), an
autolink <https://example.com/docs>, and a linkified email reader@example.com.

Escaped punctuation: \*literal asterisks\* and an entity &amp;.

Soft line
break.

Hard line\
break.

> A blockquote with **strong text**.
>
> > A nested blockquote.

- unordered item
  1. nested ordered item
  2. another nested item
- [ ] open task
- [x] completed task

1. ordered item
2. second ordered item

***

```rust extra-info
fn main() {
    println!("hello");
}
```

    indented code block

| Feature | Status | Notes |
| :--- | :---: | ---: |
| CommonMark | ✅ | preserved |
| GFM | ✅ | table and task list |
