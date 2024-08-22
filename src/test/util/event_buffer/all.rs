use time::OffsetDateTime;

use crate::test::util::Event;

pub(crate) struct All<'a, T> {
    pub(super) inner: &'a super::EventBufferInner<T>,
}

impl<'a, T> All<'a, T> {
    pub(crate) fn filter_map<R>(&self, f: impl Fn(&T) -> Option<R>) -> Vec<R> {
        self.inner
            .events
            .lock()
            .unwrap()
            .data
            .iter()
            .map(|(ev, _)| ev)
            .filter_map(f)
            .collect()
    }
}

impl<'a, T: Clone> All<'a, T> {
    /// Returns a list of current events and a future to await for more being received.
    pub(crate) fn watch(&self) -> (Vec<T>, tokio::sync::futures::Notified) {
        // The `Notify` must be created *before* reading the events to ensure any added
        // events trigger notifications.
        let notify = self.inner.event_received.notified();
        let events = self
            .inner
            .events
            .lock()
            .unwrap()
            .data
            .iter()
            .map(|(ev, _)| ev)
            .cloned()
            .collect();
        (events, notify)
    }

    /// Returns a list of current events.
    pub(crate) fn get(&self) -> Vec<T> {
        self.watch().0
    }

    pub(crate) fn get_timed(&self) -> Vec<(T, OffsetDateTime)> {
        self.inner.events.lock().unwrap().data.clone()
    }
}

impl<'a> All<'a, Event> {}

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
