# Link survey

Inline links can contain [nested parentheses](https://example.test/docs/(v1)/intro)
and titles such as [the guide](https://example.test/guide "API guide").

Escaped destinations remain readable: [escaped](https://example.test/a\(b\)).

Reference links are useful in long documents: [the same guide][guide].

Autolinks include <https://example.test/auto> and <mailto:team@example.test>.

[guide]: https://example.test/guide

The **Nested destination** cases above should remain links rather than plain text.
