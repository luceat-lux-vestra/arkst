# Metadata normalization

UTF-8 주변 문맥과 함께 [direct metadata link](https://example.test/a&amp;b "t&ouml;tle")를 둔다.

Reference metadata도 [같은 목적지][metadata-ref]로 유지한다.

[metadata-ref]: https://example.test/r&#x26;s "제목 &amp; 확인"

```rust extra&amp;metadata
fn metadata_fixture() {}
```

이 문서는 링크와 fenced-code info가 일반 Markdown 문맥 안에서 함께
동작하는지 확인한다.
