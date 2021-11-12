use super::state::*;
use dominator::{clone, html, Dom, DomBuilder};
use futures_signals::signal::SignalExt;
use std::rc::Rc;
use utils::prelude::*;
use wasm_bindgen::prelude::*;
use web_sys::HtmlElement;

impl<T: ToString + Clone + 'static, P: ToString + 'static, L: ToString + 'static>
    SimpleSelect<T, P, L>
{
    pub fn render(state: Rc<Self>, slot: Option<&str>) -> Dom {
        Self::_render_mixin(
            state,
            slot,
            None::<fn(DomBuilder<HtmlElement>) -> DomBuilder<HtmlElement>>,
        )
    }
    pub fn render_mixin<F>(state: Rc<Self>, slot: Option<&str>, mixin: F) -> Dom
    where
        F: FnOnce(DomBuilder<HtmlElement>) -> DomBuilder<HtmlElement>,
    {
        Self::_render_mixin(state, slot, Some(mixin))
    }

    fn _render_mixin<F>(state: Rc<Self>, slot: Option<&str>, mixin: Option<F>) -> Dom
    where
        F: FnOnce(DomBuilder<HtmlElement>) -> DomBuilder<HtmlElement>,
    {
        html!("input-select", {
            .apply_if(slot.is_some(), |dom| {
                dom.property("slot", slot.unwrap_ji())
            })
            .property_signal("value", state.value.signal_cloned().map(|value| {
                match value {
                    None => JsValue::NULL,
                    Some(value) => JsValue::from_str(&value.to_string())
                }
            }))
            .apply_if(state.label.is_some(), |dom| {
                dom.property("label", state.label.as_ref().unwrap_ji().to_string())
            })
            .apply_if(state.placeholder.is_some(), |dom| {
                dom.property("placeholder", state.placeholder.as_ref().unwrap_ji().to_string())
            })
            .children(state.values.iter().map(clone!(state => move |value| {
                html!("input-select-option", {
                    .text(&value.to_string())
                    .event(clone!(state, value => move |evt:events::CustomSelectedChange| {
                        if evt.selected() {
                            state.value.set(Some(value.clone()));
                            if let Some(on_change) = state.on_change.as_ref() {
                                (on_change) (Some(&value.to_string()));
                            }
                        }
                    }))
                })
            })))
            .apply_if(mixin.is_some(), |dom| {
                dom.apply(mixin.unwrap_ji())
            })
        })
    }
}
