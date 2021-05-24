use components::module::edit::DomRenderable;
use dominator::{html, Dom, clone};
use std::rc::Rc;
use super::state::*;
use components::{backgrounds, stickers, traces};
use futures_signals::{
    signal_vec::SignalVecExt,
    signal::SignalExt
};

impl DomRenderable for Main {
    fn render(state: Rc<Main>) -> Dom {
        html!("empty-fragment", {
            .property("slot", "main")
            .children_signal_vec(
                state.phase.signal_cloned().map(clone!(state => move |phase| {
                    match phase {
                        Phase::Layout => {
                            vec![
                                backgrounds::dom::render(state.base.backgrounds.clone(), None),
                                stickers::dom::render(state.base.stickers.clone(), None)
                            ]
                        },
                        Phase::Trace(traces) => {
                            let raw_backgrounds = state.base.backgrounds.to_raw();
                            let raw_stickers = state.base.stickers.to_raw();

                            vec![
                                backgrounds::dom::render_raw(&raw_backgrounds),
                                stickers::dom::render_raw(&raw_stickers),
                                traces::edit::dom::render(traces)
                            ]
                        }
                    }
                }))
                .to_signal_vec()
            )
        })
    }
}
