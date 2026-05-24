# Unicorn Ending Timer

## Goal

Add an "ending timer" to the Unicorn display that counts down the last 10 minutes of a calendar event, paired with the existing T-20m countdown to the next event's start. The ending timer uses a different icon from the regular countdown so the user can tell at a glance whether the display is "time until event starts" or "time until event ends."

## Behavior

The unicorn currently receives `CalendarEventUpcoming { description, start_at, end_at }` at T-20m before an event's start (HA fires a webhook to `POST /calendar` on the hub). The hub's `Unicorn` actor responds by sending a single countdown to the device.

After this change, every event also gets an **ending timer** fired at `end_at - 10m`, **unless** the next event arriving from HA chains directly to this one (i.e., `next.start_at == this.end_at`), in which case the ending timer is suppressed and only the next event's countdown is shown.

### Walkthroughs

**Case 1 — gap between events** (A 9:00–9:15, B 9:20–9:30):

| Time | Event | What is sent to the device |
|------|-------|----------------------------|
| 8:40 | HA: A | Countdown to 9:00 |
| 9:00 | HA: B | (nothing immediate; B's countdown is deferred) |
| 9:05 | wake-up | Ending timer (target 9:15), then Countdown to 9:20 |
| 9:20 | wake-up | Ending timer (target 9:30) |

The unicorn device's animation queue holds the ending timer and the deferred countdown; the ending plays for 10:00 → 0:00, then the countdown to B plays 5:00 → 0:00.

**Case 2 — chained events** (A 10:00–10:15, B 10:15–10:30):

| Time | Event | What is sent to the device |
|------|-------|----------------------------|
| 9:40 | HA: A | Countdown to 10:00 |
| 9:55 | HA: B | Countdown to 10:15 (A's ending is suppressed) |
| 10:20 | wake-up | Ending timer (target 10:30) |

## Architecture

All new logic lives in the `Unicorn` actor (`src/unicorn.rs`). The HA actor and the broker message shape are unchanged.

### State

```rust
enum ScheduledAction {
    SendCountdown { target: DateTime<Local> },
    SendEnding    { target: DateTime<Local> },
}

struct ScheduledItem {
    at: DateTime<Local>,
    action: ScheduledAction,
}

// Added to Unicorn:
schedule: Vec<ScheduledItem>,        // sorted by (at, action_rank)
wake_task: Option<JoinHandle<()>>,   // sleeps until schedule[0].at, then sends WakeUp
```

A new self-message variant `WakeUp` is delivered by the wake task. The schedule is the entire mutable state for ending-timer logic — there is no separate `last_event_*` field; the latest event's end time is "the `target` of whichever `SendEnding` is in the schedule."

### Sort key

The schedule is kept sorted by `(at, action_rank)` where `SendEnding` has rank `0` and `SendCountdown` has rank `1`. This guarantees the ending fires before the paired countdown at the same `at`, as a property of the data rather than of the insertion order.

### Operations

**`process_schedule()`** — the only place that touches `wake_task`:

1. Pop every item where `at <= now`, execute each in order (HTTP GET to device).
2. Drop `wake_task` if present. If the schedule is non-empty, spawn a new tokio task that sleeps until `schedule[0].at` and then sends `WakeUp` to the actor.

**On `WakeUp`:** call `process_schedule()`.

**On `CalendarEventUpcoming { start_at, end_at }`:**

1. Scan `schedule` for a `SendEnding { target }` where `target == start_at` (chained case):
   - **Found** → remove it. Enqueue `SendCountdown(start_at)` at `now`.
   - **Not found** → look for the `SendEnding` in `schedule` with the largest `at` (the most distant scheduled ending):
     - **Exists** → enqueue `SendCountdown(start_at)` at that item's `at` (rides along with the ending).
     - **None** → enqueue `SendCountdown(start_at)` at `now`.
2. Enqueue `SendEnding(end_at)` at `max(now, max(start_at, end_at - 10m))`. The extra `start_at` clamp ensures the ending timer never fires before the event has begun — relevant for events shorter than 10 minutes and for late deliveries.
3. Re-sort the schedule by the sort key above.
4. Call `process_schedule()`.

### Device API

The device's `/countdown` route requires an `icon` query parameter, one of `calendar`, `flag`, or `timer`. Missing or unknown values raise an error on the device.

- `SendCountdown` → `/countdown?timestamp=<target>&icon=calendar`
- `SendEnding` → `/countdown?timestamp=<target>&icon=flag`

(Other broker handlers use `icon=timer`: `StartCountdown`, `StartTimestampCountdown`.)

`CALENDAR`, `FLAG`, and `TIMER` bitmaps live near `CLOCK` in `unicorn/src/main.py`. The `countdown_animation` function takes the icon bytearray as a required argument. The hub uses `timestamp` (not `seconds`) so queue-delay on the device doesn't drift the displayed value.

## What's deliberately out of scope

- **Different priorities for ending vs. countdown** on the device side. Both stay at priority 2; the hub's ordered sends and the device's FIFO-within-priority queue are enough.
- **A new broker message variant.** The behavior is fully encapsulated in the `Unicorn` actor's reaction to the existing `CalendarEventUpcoming`.
- **HA-side changes.** HA continues to fire only the T-20m-before-start webhook.
- **Final icon bitmap.** A placeholder is fine; the visual will be iterated separately.
