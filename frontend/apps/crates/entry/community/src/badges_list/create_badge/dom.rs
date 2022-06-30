use std::rc::Rc;

use dominator::{clone, html, with_node, Dom};
use futures_signals::signal::not;
use utils::events;
use web_sys::{File, HtmlInputElement, HtmlTextAreaElement, Url};

use super::CreateBadge;

pub const STR_BADGE_NAME: &str = "Your badge name";
pub const STR_DESCRIPTION: &str = "Description";

impl CreateBadge {
    pub fn render(self: Rc<Self>) -> Dom {
        let state = self;

        html!("community-create-badge", {
            .property("slot", "popup")
            .children(&mut [
                html!("input-wrapper", {
                    .property("label", STR_BADGE_NAME)
                    .property("slot", "name")
                    .child(html!("input" => HtmlInputElement, {
                        .with_node!(input => {
                            .event(clone!(state => move |_: events::Input| {
                                let value = input.value();
                                state.name.set(Some(value));
                            }))
                        })
                    }))
                }),
                html!("input-wrapper", {
                    .property("label", STR_DESCRIPTION)
                    .property("slot", "description")
                    .child(html!("textarea" => HtmlTextAreaElement, {
                        .with_node!(input => {
                            .event(clone!(state => move |_: events::Input| {
                                let value = input.value();
                                state.description.set(Some(value));
                            }))
                        })
                    }))
                }),
                html!("button-rect", {
                    .text("Create")
                    .property("slot", "submit")
                    .property_signal("disabled", not(state.can_save_signal()))
                    .event(clone!(state => move |_: events::Click| {
                        state.save_badges();
                    }))
                }),
                html!("fa-button", {
                    .property("slot", "close")
                    .property("icon", "fa-regular fa-xmark")
                    .property_signal("disabled", state.loader.is_loading())
                    .event(clone!(state => move |_: events::Click| {
                        state.badge_list_state.create_popup_open.set(false);
                    }))
                }),
            ])
            .child(html!("input-file", {
                .property("slot", "image")
                .property("accept", "image/*")
                .event(clone!(state => move |e: events::CustomFile| {
                    let file = e.file();
                    state.image.set(Some(file));
                }))
                .child_signal(state.image.signal_ref(|image| {
                    Some(match image {
                        Some(image) => {
                            let object_url = file_to_object_url(image);
                            html!("img", {
                                .style("overflow", "hidden")
                                .style("max-width", "100%")
                                .style("max-height", "100%")
                                .property("src", &object_url)
                            })
                        },
                        None => {
                            html!("fa-icon", {
                                .property("icon", "fa-light fa-cloud-arrow-up")
                            })
                        }
                    })
                }))
            }))
        })
    }
}

pub fn file_to_object_url(file: &File) -> String {
    Url::create_object_url_with_blob(file).unwrap()
}
