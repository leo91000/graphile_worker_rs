# Keep cron scheduling alive after errors

Cron retries all scheduling errors indefinitely with logged, capped exponential
backoff, matching [Graphile Worker JS](https://github.com/graphile/worker/pull/544).
We chose this over classifying transient database errors because connection
failures vary across drivers; persistent configuration or permission errors also
remain visible in logs and require operator intervention rather than stopping
the worker.

Recovery repeats registration and backfill before normal scheduling, using each
entry's stable identity and existing fill window. Shutdown interrupts recovery,
including database waits and backoff; successfully scheduled ticks reset the delay.
