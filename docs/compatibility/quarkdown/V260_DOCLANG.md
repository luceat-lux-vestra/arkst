# Quarkdown v2.6.0 `.doclang` compatibility

Arkst's Quarkdown v2.6.0 `.doclang` behavior is pinned to clean-room black-box observations from the official Linux x64 v2.6.0 distribution.

- Upstream tag commit: `22f3c1169d0b1356fb51d8f43e1833d3d815aab1`
- Official `quarkdown-linux-x64.zip` SHA-256: `5b015e47c820d06ff6774eb700e60d77bc575819d4a0140cc7f1a115e7ce6dc4`
- Checked-in snapshot SHA-256: `b684f0cd8f6b14537f2857494c967cd73b1a33c9f51938ddcb2fbbcb8525c3a6`
- Snapshot rows: 1,162
- Accepted rows: 1,076
- Rejected rows: 86

The v2.6.0 contract differs from the older Arkst/JDK25-backed compatibility model in two observable ways: the getter returns the English display name, and locale availability is release-specific rather than accepting every structurally valid BCP-47 tag. Unknown or explicitly rejected identifiers fail closed and do not replace the previous document locale.

The existing JDK25 locale snapshot remains in the repository for other pinned JVM-observable compatibility behavior. It is not treated as the v2.6.0 `.doclang` availability/name oracle.

Legacy Java spellings `iw`, `in`, and `ji`, including the independently black-box-checked suffix-preserving forms used by the v2.6.0 probe set, remain accepted and return English display names.

This file records observable compatibility evidence only. It is not derived from Quarkdown implementation source.
