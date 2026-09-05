# Graphile Worker

Durable background jobs and recurring schedules.

## Language

**Job**:
A unit of work for a named task, with a payload and execution settings.

**Cron entry**:
A recurring schedule for a task, identified independently of that task so that
one task can have multiple schedules.
_Avoid_: Task identifier when referring to the schedule's identity

**Backfill**:
Scheduling missed executions of an already known cron entry within its configured
fill window.
_Avoid_: Replaying every missed execution

**Cron recovery**:
Resuming recurring scheduling after an error, including eligible backfill.
_Avoid_: Job retry, which repeats execution of an existing job

**Unused queue**:
A named queue with no jobs referencing it. A queue can be unused while still locked.
