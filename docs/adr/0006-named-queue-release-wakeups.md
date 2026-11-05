# Wake continuous workers after a named queue release

Upstream Graphile Worker merged #653 at
`b4f1b41c7abd747586879bf68a46a5a22897fa96`. Completing or failing a named
job must wake idle fetchers after persistence releases the queue lock. Insert
notifications alone cannot drain a preloaded backlog. Rust's continuous direct
slots also exhausted their initial signals, even though direct persistence is
awaited. Batchers can acknowledge enqueueing before the queue is unlocked.

Each worker now shares a private QueueWakeup with its runners and batchers.
Successful named outcomes accumulate a saturating count and one pending wakeup.
The continuous fetch loop consumes that hint, fans out another direct poll, or
pulses all of its LocalQueues before signalling their consumers. Retaining the
count preserves refetch-delay abort thresholds across coalesced batches.
Unnamed outcomes, failed persistence and mere batch enqueueing do not wake the
worker. A partially persisted failure batch counts only successful outcomes;
closed-channel direct fallbacks follow the same rule.

Hints own no jobs, allocate no unbounded event queue, spawn no detached tasks,
and never send PostgreSQL notifications. Another process still needs its own
poll or notification. LocalQueue pulses retain configured refetch delays,
claim filters, pending-return coordination and the terminal Released state.
Shutdown is selected before pending wakeups; normal slot shutdown checks remain
in force. A late completion can record a harmless unused hint after shutdown.
No released-worker assertion or restart is introduced.

Public options and hook source variants remain unchanged. Direct release hints
are reported as Polling (an immediate database poll), and local hints as
LocalQueue activity. Run-once retains its own fallible continuation and drain
rules and does not start a continuous wakeup consumer. The original error,
batching/backpressure policy and retry backoff remain unchanged.

No migration is necessary: this is process-local scheduling after existing SQL
commits. Applied revisions 1–22, private row types, retirement markers, advisory
locks and PostgreSQL 12 minimum remain intact. Existing root MIT attribution to
Benjie Gillam is retained. This ports the behavior using Rust runtime primitives;
it does not introduce the Node worker-pool API or its JavaScript lifecycle.
