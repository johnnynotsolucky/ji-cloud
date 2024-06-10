use dominator::{class, clone, html, pseudo, Dom};
use futures_signals::signal::{Signal, SignalExt};
use shared::domain::jig::JigResponse;
use utils::init::user::{get_user_id, is_user_set};

use std::rc::Rc;

use components::{
    page_footer,
    page_header::{PageHeader, PageHeaderConfig, PageLinks},
    player_popup::{PlayerPopup, PreviewPopupCallbacks},
};

use super::state::{Home, HomePageMode};

mod flippable_asset_card;
mod home_sections;
mod iframe;
mod search_section;
use iframe::Iframe;

pub use flippable_asset_card::render_flippable_asset_card;

impl Home {
    pub fn render(self: Rc<Self>, is_search: bool) -> Dom {
        let state = self;
        html!("home-full", {
            .child_signal(state.mode.signal_ref(|mode| {
                // Some(page_header::dom::render(Rc::new(page_header::state::PageHeader::new()), None, Some(PageLinks::from(mode)), true))
                Some(PageHeader::new(PageHeaderConfig {
                    active_page: Some(PageLinks::from(mode)),
                    ..Default::default()
                }).render())
            }))
            .child(search_section::render(state.clone(), is_search))
            .children_signal_vec(state.mode.signal_cloned().map(clone!(state => move |mode| {
                match mode {
                    HomePageMode::Search(_) => vec![],
                    HomePageMode::Home => {
                        let mut divs = vec![];
                        divs.push(state.render_strip("Trending JIGs", state.trending.signal_cloned()));
                        divs.push(state.render_strip("Featured JIGs", state.featured.signal_cloned()));
                        if is_user_set() {
                            divs.push(state.render_strip("My likes", state.liked.signal_cloned()));
                        }
                        divs
                    }
                }
            })).to_signal_vec())
            .child(html!("empty-fragment", {
                .child_signal(state.mode.signal_cloned().map(move |mode| {
                    match mode {
                        HomePageMode::Home => {
                            // Some(home_sections::render(state.clone()))
                            Some(Iframe::new().render())
                        },
                        HomePageMode::Search(search_results) => {
                            Some(search_results.render())
                        },
                    }
                }))
            }))
            .child(page_footer::dom::render(None))
            .child_signal(state.play_asset.signal_cloned().map(clone!(state => move|play_asset| {
                play_asset.map(|asset_id| {
                    let close = clone!(state => move || {
                        state.play_asset.set(None);
                    });
                    PlayerPopup::new_default_player_options_with_jig_quota(
                        asset_id,
                        PreviewPopupCallbacks::new(close)
                    ).render(None)
                })
            })))
        })
    }

    pub fn render_strip(
        self: &Rc<Self>,
        heading: &str,
        jigs_signal: impl Signal<Item = Option<Vec<JigResponse>>> + 'static,
    ) -> Dom {
        let state = self;
        let user_id = get_user_id();
        html!("div", {
            .style("display", "grid")
            .child(html!("h3", {
                .style("color", "#fd7076")
                .style("font-weight", "600")
                .style("font-size", "24px")
                .style("margin", "0")
                .style("padding", "24px")
                .style("padding-bottom", "0")
                .text(heading)
            }))
            .child(html!("div", {
                .style("overflow-x", "auto")
                .style("padding", "24px")
                .style("display", "grid")
                .style("grid-auto-flow", "column")
                .style("justify-content", "start")
                .style("gap", "24px")
                .style("scrollbar-width", "thin")
                .class(class! {
                    .style("scrollbar-color", "transparent transparent")
                    .pseudo!(":hover", {
                        .style("scrollbar-color", "var(--light-gray-1) transparent")
                    })
                })
                .children_signal_vec(jigs_signal.map(clone!(state => move |trending| {
                    match trending {
                        None => vec![html!("progress")],
                        Some(trending) => {
                            trending.into_iter().map(|jig| {
                                let jig_id = jig.id;
                                render_flippable_asset_card(Rc::new(jig.into()), user_id, Box::new(clone!(state => move || {
                                    state.play_asset.set(Some(jig_id.into()));
                                })))
                            }).collect()
                        },
                    }
                })).to_signal_vec())
            }))
        })
    }
}
