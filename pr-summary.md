# Support deserializing sequences from non-contiguous elements

Fixes #177. This work is mostly a port of https://github.com/RReverser/serde-xml-rs/pull/143.

The sequence deserializer had been modified to read all XML events until the end of the current depth. Unused events are kept in a buffer which is re-read by the parent deserializer.

The buffer will be trimmed when the top-level deserializer moves on to avoid excess memory usage. Events which have been consumed by sequence deserializers in the middle of the buffer are replaced with tombstones to avoid unnecessary shuffling.

A few considerations:

- `Deserializer` type signature has an unnecessary type param `R: BufRead`, kept to avoid an API change. But this could be removed.
- Reading of out-of-order elements currently cannot be configured or disabled. This has some consequences:
    - Some callers may have previously relied on errors produced by non-contiguous elements, particularly for tuples.
    - This may be a memory usage concern for large documents.

I've made a few other changes which are closely related, but not strictly necessary. I could probably revert them if desired:

- The internal `Deserializer::peek` method now returns `Result<Event>` rather than `Result<Option<Event>>`, because it was never producing a value of `None`.