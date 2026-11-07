//! Order-preserving parallel work over a slice.

use std::sync::atomic::{AtomicUsize, Ordering};
use std::sync::Arc;

/// `work` applied to each of `items`, returned in the order of `items`, on up to `threads`
/// threads. Each thread starts from `state()` and uses the config generation of the calling
/// thread, so results do not depend on which thread handled an item.
pub(crate) fn map_in_order<T: Sync, S, R: Send>(
    items: &[T],
    threads: usize,
    state: impl Fn() -> S + Sync,
    work: impl Fn(&mut S, &T) -> R + Sync,
) -> Vec<R> {
    if items.is_empty() {
        return Vec::new();
    }
    let next = AtomicUsize::new(0);
    let config = crate::config::get();
    let mut slots: Vec<Option<R>> = std::iter::repeat_with(|| None).take(items.len()).collect();
    std::thread::scope(|scope| {
        let workers: Vec<_> = (0..threads.clamp(1, items.len()))
            .map(|_| {
                scope.spawn(|| {
                    let _config = crate::config::pin(Arc::clone(&config));
                    let mut state = state();
                    let mut done = Vec::new();
                    loop {
                        let index = next.fetch_add(1, Ordering::Relaxed);
                        let Some(item) = items.get(index) else {
                            break;
                        };
                        done.push((index, work(&mut state, item)));
                    }
                    done
                })
            })
            .collect();
        for worker in workers {
            let done = worker
                .join()
                .unwrap_or_else(|panic| std::panic::resume_unwind(panic));
            for (index, result) in done {
                slots[index] = Some(result);
            }
        }
    });
    slots
        .into_iter()
        .map(|slot| slot.expect("every item is worked on"))
        .collect()
}

/// The threads parallel work uses: one per available processor.
pub(crate) fn threads() -> usize {
    std::thread::available_parallelism().map_or(1, usize::from)
}
