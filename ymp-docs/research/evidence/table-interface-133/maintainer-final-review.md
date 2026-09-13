# Maintainer review of the final integration

Production source revision: `8c58f36504d5868566aeac33f7219d18ce563563`.

The integrated table selection change `5dd98cc` is byte-identical to independently accepted
snapshot `5e2506f` (`git diff --stat` is empty). The earlier modal corrections and historical
attribution corrections were reviewed separately and remain in this integration.

The maintainer independently inspected the final small delta `58fe689`:

- Paste reaches an actively edited page filter before the composer fallback. Popup fields keep
  their existing routing, and explicit moves to the composer clear filter-edit state.
- Home resolves a pending rebuild before selecting the first current row, so a saved anchor
  cannot override that explicit key.
- Captured `unknown` values retain their text and receive an unknown sort key; `none` is not
  incorrectly treated as missing information when it can mean an unlimited bound.

No required findings remain. The source passes formatting, strict workspace Clippy and all
498 Rust tests, with two existing ignored fixtures. New regression controls fail against the
prior source, including paste routing and Home after a live update. Release terminal and
installation checks are recorded separately in `../release-043/verification.json`.
