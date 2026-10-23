# vid-extras

A tool that turns a Jellyfin movie or series folder into a folder that also carries its extras, by discovering candidate videos, downloading them, converting them, and placing them.

## Language

**Extras**:
Videos that accompany a library item: trailers, featurettes, behind-the-scenes, deleted scenes, interviews, shorts, scenes, and uncategorised material.
_Avoid_: bonus content, supplementary material

**Source**:
A medium the user can enable for discovery, as accepted by `--sources`.
_Avoid_: provider, backend

**Source type**:
The medium that actually produced a candidate. A superset of **Source**: KinoCheck and TheTVDB produce candidates without being selectable, and KinoCheck is reached through TMDB rather than chosen directly.
_Avoid_: provider type

**Candidate**:
A video that a source type proposes as an extra for one library item, with its category and, where known, duration.
_Avoid_: result, hit, video

**Discovery**:
The operation that turns a **Discovery query** into candidates plus a **Discovery report**. It resolves identities, queries the enabled sources, and applies duplicate removal and content limits.
_Avoid_: search, crawl, fetch

**Discovery query**:
A normalized request for candidates: title, optional year, library kind, and — for series — optional season and the seasons available on disk.
_Avoid_: request, params

**Discovery report**:
The outcome of one **Discovery**: the candidates, one outcome per source type queried, the number of duplicates removed, and the limits that were applied.
_Avoid_: results, response

**Special**:
A Season 0 episode as defined by TheTVDB, matched to a video candidate and placed in the specials folder.
_Avoid_: bonus episode, extra episode

**Done marker**:
The `done.ext` file that records a library item as finished, so a later run skips it unless forced.
_Avoid_: completion file, lock file
