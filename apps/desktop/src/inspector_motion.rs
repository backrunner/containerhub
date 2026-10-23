use crate::*;
use std::time::{Duration, Instant};

pub(super) const COLLAPSE_DURATION: Duration = Duration::from_millis(200);

impl Workspace {
    pub(super) fn collapse_details(&mut self, cx: &mut Context<Self>) {
        if self.selected.is_none() || self.inspector_closing.is_some() {
            return;
        }
        if cx.reduce_motion() {
            self.close_details();
        } else {
            self.inspector_closing = Some(Instant::now());
        }
        cx.notify();
    }

    /// Retain the selected content until the last frame, then release its
    /// subscriptions. No deferred close callback can dismiss a newer selection.
    pub(super) fn inspector_visibility(
        &mut self,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) -> f32 {
        let Some(started) = self.inspector_closing else {
            return 1.;
        };
        let elapsed = started.elapsed();
        if cx.reduce_motion() || elapsed >= COLLAPSE_DURATION {
            self.close_details();
            return 1.;
        }
        window.request_animation_frame();
        let t = elapsed.as_secs_f32() / COLLAPSE_DURATION.as_secs_f32();
        // Smoothstep: same curve as cubic-bezier(1/3, 0, 2/3, 1) in the prototype.
        1. - t * t * (3. - 2. * t)
    }
}
