# Export provenance

This standalone repository contains a working-file snapshot, not the original
research repository's Git history. Source code and mathematical input/trace
text are preserved byte-for-byte. JSON evidence has historical local paths
normalized to `source-workspace`, `$ORIGINAL_HOME`, or `$ORIGINAL_TMP`; these
are provenance placeholders, not runnable paths in this checkout.

`FILES.json` records exported checksums and, for copied records, the original
source path and checksum. Runtime hash fields inside historical ledgers still
refer to the original artifacts. They do not certify normalized JSON bytes.

The source repository was mathzhuonichi/research. The snapshot was packaged
on 2026-09-21 while the research remained paused. No executable binaries,
credentials, Git metadata, other research topics, or automatic workflows were
exported. No solver or research campaign was launched during publication.
