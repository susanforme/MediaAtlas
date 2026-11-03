# Tagged beta publication

`release.py` checks versions, inspects native payloads, stages each target and aggregates the complete required target set. `publish.py` performs the final publication gates and supports rebuilding a tag that already has a Release.

The workflow checks out its event commit, cancels older runs in the same repository/tag concurrency group, and resolves the live GitHub tag before scheduling builds. Annotated tags are dereferenced to their actual commit. The publisher repeats that check before each release mutation and before publishing. A run whose tag moved stops without continuing uploads or publication.

Before changing GitHub, `publish.py publish` regenerates the aggregate from all verified target directories and compares the complete output set byte for byte, including manifests, notices and checksums. A changed file, missing required platform, unknown asset, stale provenance or failed test record blocks publication.

For an existing Release, the publisher reuses its ID and first changes it to a draft. It replaces every expected asset, then checks the exact remote filename set, upload state, size and GitHub SHA-256 digest before making the Release public again. GitHub has no atomic multi-asset replacement API; hiding the Release during replacement prevents publishing mixed package generations. Failed uploads or remote verification leave a draft that a later successful run can rebuild. Unexpected existing assets fail closed and are never automatically deleted. The script does not move tags or delete Releases.

Local checks (no release mutation):

```sh
python -m unittest discover -s scripts/release -p 'test_*.py' -v
python scripts/release/publish.py check-current --tag "$RELEASE_TAG" --commit "$RELEASE_COMMIT" --repository "$RELEASE_REPOSITORY"
```

The workflow alone invokes the mutating `publish` command after its complete native build matrix and artifact checks. Development verification does not execute that command against GitHub.
