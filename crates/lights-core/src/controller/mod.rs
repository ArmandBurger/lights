// lights - brightness controller for monitors via ddcutil
// Copyright (C) 2026  Armand Burger
//
// This program is free software: you can redistribute it and/or modify
// it under the terms of the GNU General Public License as published by
// the Free Software Foundation, either version 3 of the License, or
// (at your option) any later version.
//
// This program is distributed in the hope that it will be useful,
// but WITHOUT ANY WARRANTY; without even the implied warranty of
// MERCHANTABILITY or FITNESS FOR A PARTICULAR PURPOSE.  See the
// GNU General Public License for more details.
//
// You should have received a copy of the GNU General Public License
// along with this program.  If not, see <https://www.gnu.org/licenses/>.

//! Brightness state shared by the TUI and the desktop UI.
//!
//! Every ddcutil call takes a few hundred milliseconds, so all of them run on
//! worker threads. Input updates the shown value immediately and queues the
//! new target; the target is only written once input on that monitor has
//! been quiet for `DEBOUNCE`, so a burst of input becomes a single write. One
//! writer thread per monitor applies the latest target. A polling thread
//! reads values every `REFRESH`, skipping monitors with a write in flight.
//!
//! The lock syncs every monitor to the selected one's brightness; while
//! locked, every change applies the same value to all monitors.
//!
//! State is guarded by one mutex. `notify` (state changed) and `report`
//! (a message for the user) may be called from any thread and are never
//! called while holding the mutex; the UI marshals them onto its own thread.

mod state;
mod view;

use std::collections::HashMap;
use std::sync::{Arc, Condvar, Mutex, MutexGuard, PoisonError};
use std::thread;
use std::time::Duration;

use crate::backend::Backend;
use crate::brightness;
use crate::grid::Grid;
use crate::monitors::Display;

use state::{Readings, State, bus_of};
pub use view::{MonitorView, View};

const REFRESH: Duration = Duration::from_millis(1500);
/// Quiet input time before a target is written.
const DEBOUNCE: Duration = Duration::from_millis(300);

type Notify = Box<dyn Fn() + Send + Sync>;
type Report = Box<dyn Fn(String) + Send + Sync>;

struct Inner {
    backend: Arc<dyn Backend>,
    notify: Notify,
    report: Report,
    state: Mutex<State>,
    stopped: Mutex<bool>,
    stop: Condvar,
}

/// Cheap to clone; clones share the same state.
#[derive(Clone)]
pub struct Controller {
    inner: Arc<Inner>,
}

impl Controller {
    /// Brightness change per key press or click, in percent.
    pub const STEP: i32 = 5;

    pub fn new(
        backend: Arc<dyn Backend>,
        notify: impl Fn() + Send + Sync + 'static,
        report: impl Fn(String) + Send + Sync + 'static,
    ) -> Self {
        Self {
            inner: Arc::new(Inner {
                backend,
                notify: Box::new(notify),
                report: Box::new(report),
                state: Mutex::default(),
                stopped: Mutex::new(false),
                stop: Condvar::new(),
            }),
        }
    }

    pub fn start(&self) {
        self.reload();
        let inner = self.inner.clone();
        thread::spawn(move || inner.poll_loop());
    }

    /// Re-detect monitors and read their brightness.
    pub fn reload(&self) {
        let inner = self.inner.clone();
        thread::spawn(move || inner.load());
    }

    /// Stop polling and write any still-debouncing targets (blocks).
    pub fn shutdown(&self) {
        *self
            .inner
            .stopped
            .lock()
            .unwrap_or_else(PoisonError::into_inner) = true;
        self.inner.stop.notify_all();

        let pending = {
            let mut st = self.inner.lock();

            for bus in std::mem::take(&mut st.timers) {
                *st.tokens.entry(bus).or_default() += 1;
            }

            st.pending.clone()
        };

        for (bus, value) in pending {
            let _ = self.inner.backend.set(bus, value);
        }
    }

    pub fn view(&self) -> View {
        let st = self.inner.lock();
        let monitors = st
            .displays
            .iter()
            .enumerate()
            .map(|(pos, d)| {
                let bus = bus_of(d);

                MonitorView {
                    display: d.clone(),
                    bus,
                    value: st.shown(bus),
                    busy: st.busy(bus),
                    selected: st.locked || pos == st.selected,
                }
            })
            .collect();
        View {
            detected: st.detected,
            locked: st.locked,
            layout: st.layout,
            monitors,
        }
    }

    pub fn select(&self, delta: i32) {
        {
            let mut st = self.inner.lock();
            let count = st.displays.len() as i32;

            if count == 0 {
                return;
            }

            st.selected = (st.selected as i32 + delta).rem_euclid(count) as usize;
        }

        (self.inner.notify)();
    }

    pub fn select_bus(&self, bus: u32) {
        {
            let mut st = self.inner.lock();

            let Some(pos) = st.buses().iter().position(|&b| b == bus) else {
                return;
            };

            st.selected = pos;
        }

        (self.inner.notify)();
    }

    /// Move by `steps` steps of `STEP`, snapping to multiples of it.
    ///
    /// Applies to `bus` (default: the selected monitor), or to every
    /// monitor while locked.
    pub fn step(&self, steps: i32, bus: Option<u32>) {
        if steps == 0 {
            return;
        }

        let delta = Self::STEP * steps.signum();
        self.inner.change(
            move |mut value| {
                for _ in 0..steps.unsigned_abs() {
                    value = brightness::snap_step(value, delta);
                }

                value
            },
            bus,
        );
    }

    /// A click on one of a monitor's halves: select it and step it.
    pub fn click_step(&self, bus: u32, steps: i32) {
        self.select_bus(bus);
        self.step(steps, Some(bus));
    }

    pub fn set_level(&self, value: i32, bus: Option<u32>) {
        self.inner.change(move |_| value, bus);
    }

    /// Lock (syncing all monitors to the selected one) or unlock.
    pub fn toggle_lock(&self) {
        let message = {
            let mut st = self.inner.lock();

            if st.locked {
                st.locked = false;
                None
            } else if st.displays.is_empty() {
                None
            } else {
                match st.shown(st.selected_bus()) {
                    None => Some("cannot link: brightness unknown".to_owned()),
                    Some(base) => {
                        st.locked = true;

                        for bus in st.buses() {
                            if st.shown(bus) != Some(base) {
                                self.inner.queue(&mut st, bus, base);
                            }
                        }

                        Some(format!("linked: all monitors at {base}%"))
                    }
                }
            }
        };

        if let Some(message) = message {
            (self.inner.report)(message);
        }

        (self.inner.notify)();
    }
}

impl Inner {
    fn lock(&self) -> MutexGuard<'_, State> {
        self.state.lock().unwrap_or_else(PoisonError::into_inner)
    }

    fn change(self: &Arc<Self>, compute: impl Fn(i32) -> i32, bus: Option<u32>) {
        let error = {
            let mut st = self.lock();

            if st.displays.is_empty() {
                return;
            }

            let bus = bus.unwrap_or_else(|| st.selected_bus());

            match st.shown(bus) {
                None => Some(format!("display {}: brightness unknown", st.label(bus))),
                Some(base) => {
                    let target = brightness::clamp(compute(base));
                    let buses = if st.locked { st.buses() } else { vec![bus] };
                    let buses: Vec<u32> = buses
                        .into_iter()
                        .filter(|&b| st.shown(b) != Some(target))
                        .collect();
                    if buses.is_empty() {
                        return;
                    }

                    for b in buses {
                        self.queue(&mut st, b, target);
                    }

                    None
                }
            }
        };

        if let Some(error) = error {
            (self.report)(error);
        }

        (self.notify)();
    }

    /// Queue `value` for `bus` and restart its debounce.
    fn queue(self: &Arc<Self>, st: &mut State, bus: u32, value: i32) {
        st.pending.insert(bus, value);
        let token = st.tokens.entry(bus).or_default();
        *token += 1;
        let token = *token;
        st.timers.insert(bus);
        let inner = self.clone();

        thread::spawn(move || {
            thread::sleep(DEBOUNCE);
            inner.debounce_elapsed(bus, token);
        });
    }

    fn debounce_elapsed(self: &Arc<Self>, bus: u32, token: u64) {
        let start = {
            let mut st = self.lock();

            if st.tokens.get(&bus) != Some(&token) {
                return; // replaced by newer input
            }

            st.timers.remove(&bus);

            if st.writing.contains(&bus) {
                return; // the running writer picks the target up
            }

            // Input that netted out (e.g. + then -) needs no write.
            if st.pending.get(&bus).copied() == st.values.get(&bus).copied().flatten() {
                st.pending.remove(&bus);
                false
            } else {
                let start = st.pending.contains_key(&bus);

                if start {
                    st.writing.insert(bus);
                }

                start
            }
        };

        if start {
            let inner = self.clone();
            thread::spawn(move || inner.write_loop(bus));
        } else {
            (self.notify)();
        }
    }

    fn write_loop(&self, bus: u32) {
        loop {
            let target = {
                let mut st = self.lock();
                // New input arrived during the last write: let its debounce
                // finish; debounce_elapsed starts a new writer.
                let target = st
                    .pending
                    .get(&bus)
                    .copied()
                    .filter(|_| !st.timers.contains(&bus));
                if target.is_none() {
                    st.writing.remove(&bus);
                }

                target
            };

            let Some(target) = target else {
                break;
            };

            if self.backend.set(bus, target).is_err() {
                let label = {
                    let mut st = self.lock();
                    st.pending.remove(&bus);
                    st.writing.remove(&bus);
                    st.label(bus)
                };

                (self.report)(format!("display {label}: cannot adjust brightness"));
                break;
            }

            {
                let mut st = self.lock();
                st.values.insert(bus, Some(target));
                *st.write_gen.entry(bus).or_default() += 1;

                // Drop the target only if no newer one arrived during the write.
                if st.pending.get(&bus) == Some(&target) {
                    st.pending.remove(&bus);
                }
            }

            (self.notify)();
        }

        (self.notify)();
    }

    fn read_all(&self, buses: &[u32]) -> Readings {
        thread::scope(|scope| {
            let reads: Vec<_> = buses
                .iter()
                .map(|&bus| (bus, scope.spawn(move || self.backend.get(bus).ok())))
                .collect();
            reads
                .into_iter()
                .map(|(bus, read)| (bus, read.join().ok().flatten()))
                .collect()
        })
    }

    fn load(&self) {
        let mut displays = match self.backend.detect() {
            Ok(found) => found.into_iter().filter(Display::addressable).collect(),
            Err(e) => {
                (self.report)(e.to_string());
                Vec::new()
            }
        };

        let max = Grid::MAX.cells();

        if displays.len() > max {
            (self.report)(format!("showing {max} of {} monitors", displays.len()));
            displays.truncate(max);
        }

        let buses: Vec<u32> = displays.iter().map(bus_of).collect();
        let values = self.read_all(&buses);

        fn key(ds: &[Display]) -> Vec<(Option<u32>, &str)> {
            ds.iter().map(|d| (d.bus(), d.model.as_str())).collect()
        }

        {
            let mut st = self.lock();

            if key(&displays) != key(&st.displays) {
                st.layout += 1;
            }

            st.displays = displays;
            st.detected = true;

            if st.selected >= st.displays.len() {
                st.selected = 0;
            }

            st.apply_reads(values);
        }

        (self.notify)();
    }

    /// Wait up to `REFRESH`; false once shut down.
    fn wait(&self) -> bool {
        let stopped = self.stopped.lock().unwrap_or_else(PoisonError::into_inner);
        let (stopped, _) = self
            .stop
            .wait_timeout_while(stopped, REFRESH, |stopped| !*stopped)
            .unwrap_or_else(PoisonError::into_inner);
        !*stopped
    }

    fn poll_loop(&self) {
        while self.wait() {
            let gens: HashMap<u32, u64> = {
                let st = self.lock();
                st.buses()
                    .into_iter()
                    .filter(|&b| !st.busy(b))
                    .map(|b| (b, st.write_gen(b)))
                    .collect()
            };

            if gens.is_empty() {
                continue;
            }

            let buses: Vec<u32> = gens.keys().copied().collect();

            let values = self.read_all(&buses);
            {
                let mut st = self.lock();
                // A write that finished while reading makes that reading stale.
                let fresh = values
                    .into_iter()
                    .filter(|(b, _)| st.write_gen(*b) == gens[b])
                    .collect();
                st.apply_reads(fresh);
            }

            (self.notify)();
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::backend::Fake;
    use crate::ddcutil::DdcError;
    use std::sync::atomic::{AtomicUsize, Ordering};
    use std::time::Instant;

    /// A controller over two fake monitors (buses 100, 101 at 20% and 35%)
    /// with the given call delay, loaded and ready.
    fn loaded(delay: Duration) -> (Controller, Arc<Fake>) {
        let fake = Arc::new(Fake::new(2, delay));
        let c = Controller::new(fake.clone(), || {}, |_| {});
        c.inner.load();
        (c, fake)
    }

    fn wait_until(what: impl Fn() -> bool) {
        let deadline = Instant::now() + Duration::from_secs(5);

        while !what() {
            assert!(Instant::now() < deadline, "timed out");
            thread::sleep(Duration::from_millis(10));
        }
    }

    fn idle(c: &Controller) -> bool {
        c.view().monitors.iter().all(|m| !m.busy)
    }

    #[test]
    fn burst_of_input_becomes_one_write() {
        let writes = Arc::new(AtomicUsize::new(0));
        struct Counting(Arc<Fake>, Arc<AtomicUsize>);

        impl Backend for Counting {
            fn detect(&self) -> Result<Vec<Display>, DdcError> {
                self.0.detect()
            }

            fn get(&self, bus: u32) -> Result<i32, DdcError> {
                self.0.get(bus)
            }

            fn set(&self, bus: u32, value: i32) -> Result<i32, DdcError> {
                self.1.fetch_add(1, Ordering::SeqCst);
                self.0.set(bus, value)
            }
        }

        let fake = Arc::new(Fake::new(2, Duration::ZERO));
        let c = Controller::new(
            Arc::new(Counting(fake.clone(), writes.clone())),
            || {},
            |_| {},
        );
        c.inner.load();

        for _ in 0..4 {
            c.step(1, None); // 20 -> 25 -> 30 -> 35 -> 40
        }

        assert_eq!(c.view().monitors[0].value, Some(40));
        wait_until(|| idle(&c));
        assert_eq!(writes.load(Ordering::SeqCst), 1);
        assert_eq!(fake.value(100), Some(40));

        // Input that nets to zero writes nothing.
        c.step(1, None);
        c.step(-1, None);
        wait_until(|| idle(&c));
        assert_eq!(writes.load(Ordering::SeqCst), 1);
    }

    #[test]
    fn lock_syncs_and_steps_together() {
        let (c, fake) = loaded(Duration::ZERO);
        c.select_bus(101); // at 35%
        c.toggle_lock();
        assert!(c.view().locked);
        c.step(1, None);
        wait_until(|| idle(&c));
        assert_eq!(fake.value(100), Some(40));
        assert_eq!(fake.value(101), Some(40));
    }

    #[test]
    fn stepping_past_the_limits_does_nothing() {
        let (c, _) = loaded(Duration::ZERO);
        c.set_level(100, Some(100));
        c.set_level(0, Some(101));
        wait_until(|| idle(&c));
        c.step(1, Some(100));
        c.step(-1, Some(101));
        assert!(idle(&c));

        c.toggle_lock(); // selected monitor is at 100%
        wait_until(|| idle(&c));
        c.step(1, None);
        assert!(idle(&c));
        c.step(-1, None);
        assert!(!idle(&c));
    }

    #[test]
    fn shutdown_writes_pending_targets() {
        let (c, fake) = loaded(Duration::ZERO);
        c.set_level(70, Some(100));
        c.shutdown();
        assert_eq!(fake.value(100), Some(70));
    }

    #[test]
    fn selection_wraps() {
        let (c, _) = loaded(Duration::ZERO);
        c.select(-1);
        assert!(c.view().monitors[1].selected);
        c.select(1);
        assert!(c.view().monitors[0].selected);
    }
}
