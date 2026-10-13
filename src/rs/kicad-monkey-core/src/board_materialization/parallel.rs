//! Running independent producer stages side by side.
//!
//! Only stages that read the source and return a value run concurrently;
//! emission stays sequential, so output is byte-identical whatever the
//! scheduling. On `wasm32`, which has no threads here, stages run in order.

/// Run `background` alongside `foreground` and return both results.
pub(crate) fn join<F, B, RF, RB>(foreground: F, background: B) -> (RF, RB)
where
    F: FnOnce() -> RF,
    B: FnOnce() -> RB + Send,
    RB: Send,
{
    #[cfg(not(target_arch = "wasm32"))]
    {
        std::thread::scope(|scope| {
            let handle = scope.spawn(background);
            let foreground = foreground();
            let background = handle
                .join()
                .unwrap_or_else(|panic| std::panic::resume_unwind(panic));
            (foreground, background)
        })
    }
    #[cfg(target_arch = "wasm32")]
    {
        let background = background();
        (foreground(), background)
    }
}
