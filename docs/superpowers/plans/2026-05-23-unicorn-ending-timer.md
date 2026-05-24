# Unicorn Ending Timer Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Add an "ending timer" to the Unicorn display that counts down the last 10 minutes of a calendar event (with a different icon), suppressed when the next event chains directly to it.

**Architecture:** All scheduling state lives in the `Unicorn` actor (`src/unicorn.rs`) as a sorted `Vec<ScheduledItem>` plus a single tokio wake-up task. A pure helper `add_calendar_event(...)` mutates the schedule; `process_schedule(...)` pops due items, sends HTTP requests to the device, and re-arms the wake task. The device's `/countdown` route is extended with an optional `?icon=ending` query parameter.

**Tech Stack:** Rust (kameo actors, tokio, reqwest, chrono), MicroPython (microdot HTTP, picographics).

**Spec:** `docs/superpowers/specs/2026-05-23-unicorn-ending-timer-design.md`

---

## File Structure

**Created:** none.

**Modified:**
- `unicorn/src/main.py` — add `ENDING` icon bitmap, parameterize `countdown_animation(timestamp, icon)`, extend `/countdown` route to read `?icon=ending`.
- `src/unicorn.rs` — add `ScheduledAction`, `ScheduledItem`, pure helpers (`action_rank`, `sort_schedule`, `add_calendar_event`, `pop_due`), `WakeUp` self-message, `process_schedule` method, schedule state on `Unicorn` struct, rewritten `CalendarEventUpcoming` handler, and `#[cfg(test)]` unit tests for the pure helpers.

No new files. No new broker message variants. No HA-side changes.

---

## Task 1: Device — add ENDING icon placeholder and parameterize countdown_animation

**Files:**
- Modify: `unicorn/src/main.py` (icon definitions, `countdown_animation`, `/countdown` route)

- [ ] **Step 1: Add the ENDING placeholder icon next to CALENDAR and CLOCK**

Insert just after the `CLOCK = bytearray([...])` block (~line 748). Use a distinct placeholder so it's visually obvious it's a different icon during testing (a diagonal line is enough for now):

```python
ENDING = bytearray([
    0b00001111,0b11111000,
    0b00001100,0b00011000,
    0b00001010,0b00101000,
    0b00001001,0b01001000,
    0b00001000,0b10001000,
    0b00001001,0b01001000,
    0b00001010,0b00101000,
    0b00001100,0b00011000,
    0b00001111,0b11111000,
    0b00000000,0b00000000, # Ending (placeholder)
])
```

- [ ] **Step 2: Parameterize `countdown_animation` to accept an icon**

Change the signature at the top of `countdown_animation` (search for `async def countdown_animation(timestamp):`) to:

```python
async def countdown_animation(timestamp, icon=CALENDAR):
```

Replace the two `draw_icon(graphics, CALENDAR, 0, y)` calls inside the function with:

```python
draw_icon(graphics, icon, 0, y)
```

And in the `except AnimationInterrupt:` block at the bottom of `countdown_animation` (search for `enqueue_animation(countdown_animation(timestamp), priority=2)`), update the re-enqueue to pass the icon through:

```python
except AnimationInterrupt:
    enqueue_animation(countdown_animation(timestamp, icon), priority=2)
    raise
```

- [ ] **Step 3: Extend the `/countdown` route to read the `icon` query arg**

Find the route handler:

```python
@server.route("/countdown", methods=["GET"])
async def countdown(request):
    if 'seconds' in request.args:
        seconds = int(request.args['seconds'])
        timestamp = time.time() + seconds
    else:
        while True:
            try:
                ntptime.settime()
                break
            except:
                time.sleep(1)
                pass
        timestamp = int(request.args['timestamp'])

    enqueue_animation(countdown_animation(timestamp), priority=2)
    return 'started'
```

Change the last `enqueue_animation` line to look up the icon:

```python
    icon = ENDING if request.args.get('icon') == 'ending' else CALENDAR
    enqueue_animation(countdown_animation(timestamp, icon), priority=2)
    return 'started'
```

- [ ] **Step 4: Verify Python syntax is valid**

Run from the repo root:

```
python -m py_compile unicorn/src/main.py
```

Expected: no output (success). If you get a `SyntaxError`, fix it before continuing.

Note: this compile-checks the Python source but does **not** execute it — the file imports MicroPython-only modules (`galactic`, `picographics`, `phew`, etc.) that don't exist on a regular host Python. Running the file directly will fail with `ModuleNotFoundError`; that's expected. Full functional testing happens on the device.

- [ ] **Step 5: Commit**

```bash
git add unicorn/src/main.py
git commit -m "Added ENDING icon and icon param to countdown animation"
```

---

## Task 2: Hub — add ScheduledAction / ScheduledItem types and sort helpers (with tests)

**Files:**
- Modify: `src/unicorn.rs` (add types and helpers above the `impl Actor` block; add `#[cfg(test)] mod tests` at the bottom)

- [ ] **Step 1: Add the type definitions**

Insert near the top of `src/unicorn.rs`, just after the `use` statements:

```rust
#[derive(Debug, Clone, PartialEq, Eq)]
enum ScheduledAction {
    SendCountdown { target: chrono::DateTime<chrono::Local> },
    SendEnding { target: chrono::DateTime<chrono::Local> },
}

#[derive(Debug, Clone, PartialEq, Eq)]
struct ScheduledItem {
    at: chrono::DateTime<chrono::Local>,
    action: ScheduledAction,
}

fn action_rank(action: &ScheduledAction) -> u8 {
    match action {
        ScheduledAction::SendEnding { .. } => 0,
        ScheduledAction::SendCountdown { .. } => 1,
    }
}

fn sort_schedule(schedule: &mut Vec<ScheduledItem>) {
    schedule.sort_by_key(|item| (item.at, action_rank(&item.action)));
}
```

- [ ] **Step 2: Add a unit test for the sort key**

Append to the bottom of `src/unicorn.rs`:

```rust
#[cfg(test)]
mod tests {
    use super::*;
    use chrono::{DateTime, Local, TimeZone};

    fn t(year: i32, month: u32, day: u32, hour: u32, minute: u32) -> DateTime<Local> {
        Local.with_ymd_and_hms(year, month, day, hour, minute, 0).unwrap()
    }

    #[test]
    fn ending_sorts_before_countdown_at_same_time() {
        let same_at = t(2026, 5, 23, 9, 5);
        let mut schedule = vec![
            ScheduledItem {
                at: same_at,
                action: ScheduledAction::SendCountdown { target: t(2026, 5, 23, 9, 20) },
            },
            ScheduledItem {
                at: same_at,
                action: ScheduledAction::SendEnding { target: t(2026, 5, 23, 9, 15) },
            },
        ];

        sort_schedule(&mut schedule);

        assert!(matches!(schedule[0].action, ScheduledAction::SendEnding { .. }));
        assert!(matches!(schedule[1].action, ScheduledAction::SendCountdown { .. }));
    }

    #[test]
    fn earlier_at_sorts_first() {
        let mut schedule = vec![
            ScheduledItem {
                at: t(2026, 5, 23, 9, 20),
                action: ScheduledAction::SendEnding { target: t(2026, 5, 23, 9, 30) },
            },
            ScheduledItem {
                at: t(2026, 5, 23, 9, 5),
                action: ScheduledAction::SendCountdown { target: t(2026, 5, 23, 9, 20) },
            },
        ];

        sort_schedule(&mut schedule);

        assert_eq!(schedule[0].at, t(2026, 5, 23, 9, 5));
        assert_eq!(schedule[1].at, t(2026, 5, 23, 9, 20));
    }
}
```

- [ ] **Step 3: Run the tests and verify they pass**

```
cargo test --lib unicorn::tests
```

Expected output includes:

```
test unicorn::tests::ending_sorts_before_countdown_at_same_time ... ok
test unicorn::tests::earlier_at_sorts_first ... ok
```

If tests fail, fix the implementation and re-run before continuing.

- [ ] **Step 4: Commit**

```bash
git add src/unicorn.rs
git commit -m "Added ScheduledAction/ScheduledItem types and sort helper"
```

---

## Task 3: Hub — implement `add_calendar_event` pure helper (with tests)

**Files:**
- Modify: `src/unicorn.rs` (add helper below `sort_schedule`; add tests inside `mod tests`)

- [ ] **Step 1: Add the helper function**

Insert immediately below `sort_schedule` (still above the `impl Actor` block):

```rust
fn add_calendar_event(
    schedule: &mut Vec<ScheduledItem>,
    start_at: chrono::DateTime<chrono::Local>,
    end_at: chrono::DateTime<chrono::Local>,
    now: chrono::DateTime<chrono::Local>,
) {
    let chained_idx = schedule.iter().position(|item| {
        if let ScheduledAction::SendEnding { target } = &item.action {
            *target == start_at
        } else {
            false
        }
    });

    let countdown_at = if let Some(idx) = chained_idx {
        schedule.remove(idx);
        now
    } else {
        let latest_ending_at = schedule
            .iter()
            .filter_map(|item| match item.action {
                ScheduledAction::SendEnding { .. } => Some(item.at),
                _ => None,
            })
            .max();
        latest_ending_at.unwrap_or(now)
    };

    schedule.push(ScheduledItem {
        at: countdown_at,
        action: ScheduledAction::SendCountdown { target: start_at },
    });

    let ending_at = std::cmp::max(now, end_at - chrono::Duration::minutes(10));
    schedule.push(ScheduledItem {
        at: ending_at,
        action: ScheduledAction::SendEnding { target: end_at },
    });

    sort_schedule(schedule);
}
```

- [ ] **Step 2: Add unit tests for all four cases**

Append inside `mod tests` (after the existing tests, before the closing `}`):

```rust
    #[test]
    fn first_event_schedules_immediate_countdown_and_future_ending() {
        // Event A 9:00-9:15, now is 8:40 (T-20m).
        let now = t(2026, 5, 23, 8, 40);
        let mut schedule = vec![];

        add_calendar_event(&mut schedule, t(2026, 5, 23, 9, 0), t(2026, 5, 23, 9, 15), now);

        assert_eq!(schedule.len(), 2);
        assert_eq!(schedule[0].at, now);
        assert_eq!(
            schedule[0].action,
            ScheduledAction::SendCountdown { target: t(2026, 5, 23, 9, 0) }
        );
        assert_eq!(schedule[1].at, t(2026, 5, 23, 9, 5));
        assert_eq!(
            schedule[1].action,
            ScheduledAction::SendEnding { target: t(2026, 5, 23, 9, 15) }
        );
    }

    #[test]
    fn gap_case_defers_countdown_to_ride_with_prior_ending() {
        // A 9:00-9:15 already on schedule (added at 8:40). Now event B 9:20-9:30 arrives at 9:00.
        let now_b = t(2026, 5, 23, 9, 0);
        let mut schedule = vec![];
        add_calendar_event(
            &mut schedule,
            t(2026, 5, 23, 9, 0),
            t(2026, 5, 23, 9, 15),
            t(2026, 5, 23, 8, 40),
        );

        add_calendar_event(&mut schedule, t(2026, 5, 23, 9, 20), t(2026, 5, 23, 9, 30), now_b);

        // Expected, in (at, ending-before-countdown) order:
        //   8:40  SendCountdown(9:00)
        //   9:05  SendEnding(9:15)
        //   9:05  SendCountdown(9:20)
        //   9:20  SendEnding(9:30)
        assert_eq!(schedule.len(), 4);
        assert_eq!(schedule[0].action, ScheduledAction::SendCountdown { target: t(2026, 5, 23, 9, 0) });
        assert_eq!(schedule[1].at, t(2026, 5, 23, 9, 5));
        assert_eq!(schedule[1].action, ScheduledAction::SendEnding { target: t(2026, 5, 23, 9, 15) });
        assert_eq!(schedule[2].at, t(2026, 5, 23, 9, 5));
        assert_eq!(schedule[2].action, ScheduledAction::SendCountdown { target: t(2026, 5, 23, 9, 20) });
        assert_eq!(schedule[3].at, t(2026, 5, 23, 9, 20));
        assert_eq!(schedule[3].action, ScheduledAction::SendEnding { target: t(2026, 5, 23, 9, 30) });
    }

    #[test]
    fn chained_case_removes_prior_ending_and_sends_countdown_immediately() {
        // A 10:00-10:15 already on schedule (added at 9:40). Event B 10:15-10:30 arrives at 9:55.
        let now_b = t(2026, 5, 23, 9, 55);
        let mut schedule = vec![];
        add_calendar_event(
            &mut schedule,
            t(2026, 5, 23, 10, 0),
            t(2026, 5, 23, 10, 15),
            t(2026, 5, 23, 9, 40),
        );

        add_calendar_event(&mut schedule, t(2026, 5, 23, 10, 15), t(2026, 5, 23, 10, 30), now_b);

        // A's SendEnding(target=10:15) should have been removed. Expected:
        //   9:40   SendCountdown(10:00)   (from A)
        //   9:55   SendCountdown(10:15)   (from B, immediate)
        //   10:20  SendEnding(10:30)      (from B)
        assert_eq!(schedule.len(), 3);
        assert_eq!(schedule[0].at, t(2026, 5, 23, 9, 40));
        assert_eq!(schedule[0].action, ScheduledAction::SendCountdown { target: t(2026, 5, 23, 10, 0) });
        assert_eq!(schedule[1].at, now_b);
        assert_eq!(schedule[1].action, ScheduledAction::SendCountdown { target: t(2026, 5, 23, 10, 15) });
        assert_eq!(schedule[2].at, t(2026, 5, 23, 10, 20));
        assert_eq!(schedule[2].action, ScheduledAction::SendEnding { target: t(2026, 5, 23, 10, 30) });
    }

    #[test]
    fn past_ending_window_clamps_to_now() {
        // Event arrives less than 10m before its end (degenerate / late delivery).
        let now = t(2026, 5, 23, 9, 10);
        let mut schedule = vec![];

        add_calendar_event(&mut schedule, t(2026, 5, 23, 9, 0), t(2026, 5, 23, 9, 15), now);

        // SendEnding's `at` would be 9:05 but is clamped to now (9:10).
        let ending = schedule
            .iter()
            .find(|item| matches!(item.action, ScheduledAction::SendEnding { .. }))
            .unwrap();
        assert_eq!(ending.at, now);
    }
```

- [ ] **Step 3: Run the tests and verify they pass**

```
cargo test --lib unicorn::tests
```

Expected: all six tests pass (two from Task 2, four from this task).

- [ ] **Step 4: Commit**

```bash
git add src/unicorn.rs
git commit -m "Added add_calendar_event helper with tests"
```

---

## Task 4: Hub — implement `pop_due` pure helper (with tests)

**Files:**
- Modify: `src/unicorn.rs`

- [ ] **Step 1: Add the helper**

Append just below `add_calendar_event`:

```rust
fn pop_due(
    schedule: &mut Vec<ScheduledItem>,
    now: chrono::DateTime<chrono::Local>,
) -> Vec<ScheduledItem> {
    let split = schedule
        .iter()
        .position(|item| item.at > now)
        .unwrap_or(schedule.len());
    schedule.drain(..split).collect()
}
```

- [ ] **Step 2: Add tests inside `mod tests`**

Append to `mod tests`:

```rust
    #[test]
    fn pop_due_returns_items_at_or_before_now_and_removes_them() {
        let now = t(2026, 5, 23, 9, 5);
        let mut schedule = vec![
            ScheduledItem {
                at: t(2026, 5, 23, 9, 0),
                action: ScheduledAction::SendCountdown { target: t(2026, 5, 23, 9, 0) },
            },
            ScheduledItem {
                at: t(2026, 5, 23, 9, 5),
                action: ScheduledAction::SendEnding { target: t(2026, 5, 23, 9, 15) },
            },
            ScheduledItem {
                at: t(2026, 5, 23, 9, 5),
                action: ScheduledAction::SendCountdown { target: t(2026, 5, 23, 9, 20) },
            },
            ScheduledItem {
                at: t(2026, 5, 23, 9, 20),
                action: ScheduledAction::SendEnding { target: t(2026, 5, 23, 9, 30) },
            },
        ];

        let due = pop_due(&mut schedule, now);

        assert_eq!(due.len(), 3);
        assert_eq!(schedule.len(), 1);
        assert_eq!(schedule[0].at, t(2026, 5, 23, 9, 20));
    }

    #[test]
    fn pop_due_returns_empty_when_nothing_due() {
        let now = t(2026, 5, 23, 9, 0);
        let mut schedule = vec![ScheduledItem {
            at: t(2026, 5, 23, 9, 5),
            action: ScheduledAction::SendEnding { target: t(2026, 5, 23, 9, 15) },
        }];

        let due = pop_due(&mut schedule, now);

        assert!(due.is_empty());
        assert_eq!(schedule.len(), 1);
    }
```

- [ ] **Step 3: Run the tests**

```
cargo test --lib unicorn::tests
```

Expected: all eight tests pass.

- [ ] **Step 4: Commit**

```bash
git add src/unicorn.rs
git commit -m "Added pop_due helper with tests"
```

---

## Task 5: Hub — add schedule state and WakeUp self-message to the actor

**Files:**
- Modify: `src/unicorn.rs` (struct fields, on_start initializer, new WakeUp message)

- [ ] **Step 1: Add `schedule` and `wake_task` fields to the `Unicorn` struct**

Find the struct (top of file):

```rust
pub struct Unicorn {
    client: reqwest::Client,
    base_url: reqwest::Url,
}
```

Replace it with:

```rust
pub struct Unicorn {
    client: reqwest::Client,
    base_url: reqwest::Url,
    schedule: Vec<ScheduledItem>,
    wake_task: Option<tokio::task::JoinHandle<()>>,
}
```

- [ ] **Step 2: Initialize the new fields in `on_start`**

Find the `Ok(Self { ... })` at the end of `on_start` and update it:

```rust
        Ok(Self {
            client: reqwest::Client::new(),
            base_url: reqwest::Url::parse(&std::env::var("UNICORN_BASE_URL").unwrap()).unwrap(),
            schedule: Vec::new(),
            wake_task: None,
        })
```

- [ ] **Step 3: Define the `WakeUp` self-message and its handler**

Insert just below the existing `impl Message<crate::BrokerMessage> for Unicorn { ... }` block:

```rust
#[derive(Debug)]
pub struct WakeUp;

impl Message<WakeUp> for Unicorn {
    type Reply = ();

    async fn handle(
        &mut self,
        _: WakeUp,
        context: &mut Context<Self, Self::Reply>,
    ) -> Self::Reply {
        self.process_schedule(context.actor_ref()).await;
    }
}
```

This won't compile yet — `process_schedule` is added in Task 6. That's fine; we'll commit at the end of Task 6.

- [ ] **Step 4: Verify the project still builds with the existing handlers**

We need to confirm only the new `process_schedule` reference is the missing piece. Run:

```
cargo check
```

Expected: one error, "no method named `process_schedule` found for struct `Unicorn`" (or similar). If you see other errors, fix them before moving on.

- [ ] **Step 5: Do NOT commit yet**

Task 5 leaves the build broken intentionally; Task 6 completes the actor changes and commits both together.

---

## Task 6: Hub — implement `process_schedule` method and wire it into the calendar handler

**Files:**
- Modify: `src/unicorn.rs` (add `impl Unicorn { ... }` block, replace `CalendarEventUpcoming` arm)

- [ ] **Step 1: Add the `impl Unicorn` block with `process_schedule` and `send_action`**

Insert just after the `WakeUp` message impl (still inside the same file, before `#[cfg(test)] mod tests`):

```rust
impl Unicorn {
    async fn process_schedule(&mut self, actor_ref: ActorRef<Self>) {
        let now = chrono::Local::now();
        let due = pop_due(&mut self.schedule, now);
        for item in due {
            self.send_action(&item.action).await;
        }

        if let Some(handle) = self.wake_task.take() {
            handle.abort();
        }

        if let Some(next) = self.schedule.first() {
            let next_at = next.at;
            let handle = tokio::spawn(async move {
                let now = chrono::Local::now();
                let sleep_duration = (next_at - now)
                    .to_std()
                    .unwrap_or(std::time::Duration::ZERO);
                tokio::time::sleep(sleep_duration).await;
                let _ = actor_ref.tell(WakeUp).await;
            });
            self.wake_task = Some(handle);
        }
    }

    async fn send_action(&self, action: &ScheduledAction) {
        match action {
            ScheduledAction::SendCountdown { target } => {
                tracing::info!("unicorn schedule: send countdown to {:?}", target);
                self.client
                    .get(self.base_url.join("/countdown").unwrap())
                    .query(&[("timestamp", target.timestamp().to_string())])
                    .send()
                    .await
                    .unwrap()
                    .error_for_status()
                    .unwrap();
            }
            ScheduledAction::SendEnding { target } => {
                tracing::info!("unicorn schedule: send ending to {:?}", target);
                self.client
                    .get(self.base_url.join("/countdown").unwrap())
                    .query(&[
                        ("timestamp", target.timestamp().to_string()),
                        ("icon", "ending".to_string()),
                    ])
                    .send()
                    .await
                    .unwrap()
                    .error_for_status()
                    .unwrap();
            }
        }
    }
}
```

- [ ] **Step 2: Replace the `CalendarEventUpcoming` arm to drive the schedule**

In `impl Message<crate::BrokerMessage> for Unicorn`, find:

```rust
            crate::BrokerMessage::CalendarEventUpcoming(event) => {
                tracing::info!("unicorn message: {:?}", event);

                let now = chrono::Local::now();

                let seconds = (event.start_at - now).num_seconds();

                self.client
                    .get(self.base_url.join("/countdown").unwrap())
                    .query(&[("seconds", seconds.to_string())])
                    .send()
                    .await
                    .unwrap()
                    .error_for_status()
                    .unwrap();
            }
```

Replace it with:

```rust
            crate::BrokerMessage::CalendarEventUpcoming(event) => {
                tracing::info!("unicorn message: {:?}", event);

                let now = chrono::Local::now();
                add_calendar_event(&mut self.schedule, event.start_at, event.end_at, now);
                self.process_schedule(context.actor_ref()).await;
            }
```

- [ ] **Step 3: Rename `_context` to `context` in the `BrokerMessage` handler signature**

In the same `impl Message<crate::BrokerMessage> for Unicorn` block, change:

```rust
    async fn handle(
        &mut self,
        message: crate::BrokerMessage,
        _context: &mut Context<Self, Self::Reply>,
    ) -> Self::Reply {
```

to:

```rust
    async fn handle(
        &mut self,
        message: crate::BrokerMessage,
        context: &mut Context<Self, Self::Reply>,
    ) -> Self::Reply {
```

- [ ] **Step 4: Build and run all tests**

```
cargo check
cargo test --lib unicorn::tests
```

Expected: `cargo check` succeeds with no errors. All eight unit tests pass.

- [ ] **Step 5: Commit**

```bash
git add src/unicorn.rs
git commit -m "Wired calendar events through schedule with ending timer"
```

---

## Task 7: Manual end-to-end verification

**Files:** none modified.

This task confirms the integrated behavior against the two spec walkthroughs by hitting the hub's `/calendar` endpoint directly (bypassing HA).

- [x] **Step 1: Start the hub and device**

Start the Rust hub. From the repo root, with `UNICORN_BASE_URL` pointing at the unicorn device (e.g., `http://unicorn.local`):

```
UNICORN_BASE_URL=http://unicorn.local cargo run
```

Confirm the unicorn device is reachable by opening its IP in a browser; you should see the inbox animation by default.

- [x] **Step 2: Verify the chained case (case 2 — no ending timer)**

Pick two timestamps a couple of minutes apart so you can watch the behavior live. Example: if it's currently 14:00 local, use A=14:03–14:08 and B=14:08–14:13. Send A first:

```bash
curl -X POST http://localhost:9001/calendar \
  -H 'Content-Type: application/json' \
  -d '{"summary":"A","all_day":false,"start":"2026-05-23T14:03:00-05:00","end":"2026-05-23T14:08:00-05:00"}'
```

Within a few seconds, the unicorn should show a countdown to A's start with the **calendar** icon.

Now send B (which chains directly to A — same start/end timestamp):

```bash
curl -X POST http://localhost:9001/calendar \
  -H 'Content-Type: application/json' \
  -d '{"summary":"B","all_day":false,"start":"2026-05-23T14:08:00-05:00","end":"2026-05-23T14:13:00-05:00"}'
```

Expected: a second countdown enters the device's queue. When A's countdown finishes, the device transitions straight to B's countdown — **no ending icon should ever appear**.

Adjust the timestamps in the curl bodies to match your current local time and timezone offset.

- [x] **Step 3: Verify the gap case (case 1 — ending timer between)**

Restart the hub to clear state, then send two events with a gap. Example sequence (adjust timestamps for "now + a few minutes"):

```bash
# Event A: now+1m to now+4m
curl -X POST http://localhost:9001/calendar \
  -H 'Content-Type: application/json' \
  -d '{"summary":"A","all_day":false,"start":"2026-05-23T14:01:00-05:00","end":"2026-05-23T14:04:00-05:00"}'

# Event B: now+6m to now+9m  (gap from 14:04 to 14:06)
curl -X POST http://localhost:9001/calendar \
  -H 'Content-Type: application/json' \
  -d '{"summary":"B","all_day":false,"start":"2026-05-23T14:06:00-05:00","end":"2026-05-23T14:09:00-05:00"}'
```

Tip: with the spec's 10-minute ending lead, use shorter events than the spec examples so testing doesn't take 20+ minutes. The lead is `chrono::Duration::minutes(10)` in `add_calendar_event` — for this manual test you may temporarily change it to `Duration::minutes(1)` to compress timing, then revert and rebuild.

Expected behavior with a 1-minute lead (or wait it out at 10 minutes):
1. Countdown to A's start (calendar icon).
2. At A.end - lead: ending timer (**ending icon**) counts down to A's end.
3. Without interruption, the device transitions to the countdown for B's start (calendar icon).
4. At B.end - lead: ending timer (**ending icon**) counts down to B's end.

If you changed the lead for testing, revert it now:

```rust
let ending_at = std::cmp::max(now, end_at - chrono::Duration::minutes(10));
```

And rebuild:

```
cargo build
```

- [x] **Step 4: Commit (only if you made temporary edits and reverted them)**

If you reverted the lead value, `git status` should be clean. If anything else changed during testing (logging, etc.) decide whether to keep or discard.

---

## Self-Review

Spec coverage:
- "Behavior" (case 1 and case 2 walkthroughs): Tasks 3 (pure-logic tests) + 7 (end-to-end manual).
- "State" (`schedule`, `wake_task`, no `last_event_*`): Tasks 2, 5.
- "Sort key" (ending before countdown at same `at`): Task 2.
- "process_schedule" semantics: Task 6.
- "On `CalendarEventUpcoming`" — all four branches: Task 3 tests cover the three insertion branches; clamp-to-now is also Task 3.
- "Device API" (`/countdown?icon=ending`, timestamp param): Task 1 (device) and Task 6 (hub `send_action`).
- "Out of scope" (no new broker message, no HA changes, placeholder icon): respected — no broker message added, no HA changes, ENDING bitmap is a placeholder.

Placeholder scan: no "TBD" / "implement later" / "similar to" remain. Every code step shows the full code to add or change.

Type / name consistency: `ScheduledAction`, `ScheduledItem`, `action_rank`, `sort_schedule`, `add_calendar_event`, `pop_due`, `process_schedule`, `send_action`, `WakeUp` — all spelled consistently across Tasks 2–6.
