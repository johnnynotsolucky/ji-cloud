use std::rc::Rc;

use components::{
    tooltip::{
        callbacks::TooltipErrorCallbacks,
        dom::render as TooltipDom,
        state::{Anchor, ContentAnchor, MoveStrategy, TooltipData, TooltipError, TooltipTarget, State as TooltipState},
    },
    input::simple_select::SimpleSelect
};
use dominator::{Dom, clone, html, with_node};
use futures_signals::{map_ref, signal::Signal};
use utils::{events, unwrap::UnwrapJiExt};
use web_sys::HtmlElement;

use super::state::AdditionalResourceComponent;

const STR_UPLOAD_FILE: &str = "Upload file";
const STR_ADD_LINK: &str = "Add link";
const STR_SELECT_REQUIRED: &str = "Please select type\nbefore moving on";

impl AdditionalResourceComponent {
    pub fn render(self: Rc<Self>) -> Dom {
        let state = self;

        state.loader.load(clone!(state => async move {
            state.load_resource();
        }));

        html!("jig-edit-publish-resource", {
            .property("slot", "resources")
            .property_signal("label", state.additional_resource.signal_ref(|additional_resource| {
                match additional_resource {
                    Some(additional_resource) => additional_resource.display_name.to_owned(),
                    None => String::new(),
                }
            }))
            .property_signal("resourceType", state.resource_type_name_signal())
            .child(html!("fa-button", {
                .property("slot", "delete")
                .property("icon", "fa-light fa-trash-can")
                .property_signal("disabled", state.additional_resource.signal_ref(|additional_resource| {
                    additional_resource.is_none()
                }))
                .event(clone!(state => move |_: events::Click| {
                    state.delete();
                }))
            }))
        })
    }

    fn resource_type_name_signal(self: &Rc<Self>) -> impl Signal<Item = String> {
        map_ref! {
            let current_resource = self.additional_resource.signal_cloned(),
            let all_resource_types = self.publish_state.resource_types.signal_cloned()
                => move {
                    match current_resource {
                        None => String::new(),
                        Some(current_resource) => {
                            let resource_type = all_resource_types.iter().find(|resource_type| {
                                current_resource.resource_type_id == resource_type.id
                            });

                            match resource_type {
                                None => String::new(),
                                Some(resource_type) => {
                                    resource_type.display_name.to_owned()
                                },
                            }
                        }
                    }
                }
        }
    }
}
