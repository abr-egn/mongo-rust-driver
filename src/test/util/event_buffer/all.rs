use std::sync::MutexGuard;

use time::OffsetDateTime;

pub(crate) struct All<'a, T> {
    pub(super) events: MutexGuard<'a, super::GenVec<(T, OffsetDateTime)>>,
}

impl<'a, T> All<'a, T> {
    pub(crate) fn filter_map<R>(&self, f: impl Fn(&T) -> Option<R>) -> Vec<R> {
        self.events
            .data
            .iter()
            .map(|(ev, _)| ev)
            .filter_map(f)
            .collect()
    }
}

pub(crate) struct AllMut<'a, T> {
    pub(super) inner: &'a super::EventBufferInner<T>,
}

impl<'a, T> AllMut<'a, T> {
    fn invalidate<R>(&self, f: impl FnOnce(&mut Vec<(T, OffsetDateTime)>) -> R) -> R {
        let mut events = self.inner.events.lock().unwrap();
        events.generation = super::Generation(events.generation.0 + 1);
        let out = f(&mut events.data);
        self.inner.event_received.notify_waiters();
        out
    }

    pub(crate) fn clear(&self) {
        self.invalidate(|data| data.clear());
    }

    pub(crate) fn retain(&self, mut f: impl FnMut(&T) -> bool) {
        self.invalidate(|data| data.retain(|(ev, _)| f(ev)));
    }

    pub(crate) fn push(&self, ev: T) {
        self.inner
            .events
            .lock()
            .unwrap()
            .data
            .push((ev, OffsetDateTime::now_utc()));
        self.inner.event_received.notify_waiters();
    }

    pub(crate) fn take(&self) -> Vec<T> {
        self.invalidate(|evs| evs.drain(..).map(|(ev, _)| ev).collect())
    }
}
