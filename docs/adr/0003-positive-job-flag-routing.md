# Positive job-flag routing

Accepted flags select jobs by matching any worker-selected routing label;
forbidden flags independently veto jobs carrying any excluded label. Both filters
may coexist so a worker can select Linux work while excluding GPU-tagged work.

This is a routing filter rather than capability validation: matching a Linux
label does not establish support for every other label on a job. Job flags also
carry operational meanings, including resilience during recovery, so treating
every flag as a required capability would conflate separate concepts. The choice
is part of the public filtering contract and must remain consistent across job
claiming paths.

An empty accepted set adds no positive restriction, preserving the behavior of
existing workers. A nonempty set excludes untagged jobs, including both absent
and empty flags. Exclusions always win when a job matches both sets; flags keep
their existing exact-string matching semantics.

Workers retain LocalQueue batching when configured, including with either flag
filter. Each worker-owned local queue stores an immutable filter and applies it
in the batch query before ordering, limits, attempts, or ownership changes.
This replaces the earlier direct-claim fallback so routing does not discard the
configured throughput optimization. Testing a popped cached job would be too
late to preserve attempts and ownership for rejected jobs. Existing TTL and
shutdown cleanup return eligible prefetched jobs using the same claim lifecycle.

Existing public function signatures remain supported. New filtering entry
points share the claim predicate and parameter allocation between single and
batch queries; SQL caching distinguishes both filter-presence states without
caching their values.

For a worker accepting `linux` and forbidding `gpu`:

| Job flags | Eligible |
| --- | --- |
| `linux` | Yes |
| `linux`, `other` | Yes |
| `linux`, `gpu` | No |
| `windows` | No |
| None | No |

Validation covers default and filtered workers, both database drivers, single
and batch claims, continuous runs, `run_once` including named-queue follow-up
claims, filtered and unfiltered LocalQueue caching with one or multiple queues,
shutdown returning filtered cached claims, changing filter values and cached SQL
shapes, and rejected jobs retaining their attempts and unlocked ownership.
