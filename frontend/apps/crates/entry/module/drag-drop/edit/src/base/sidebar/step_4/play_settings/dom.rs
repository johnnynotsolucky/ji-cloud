use dominator::{clone, Dom};
use futures_signals::signal::SignalExt;
use std::rc::Rc;

use super::state::PlaySettingsState;
use shared::domain::module::body::drag_drop::{Hint, Next};

use components::module::_common::edit::settings::prelude::*;

pub fn render(state: Rc<PlaySettingsState>) -> Dom {
    render_settings(Rc::new(ModuleSettings {
        lines: vec![
            ModuleSettingsLine::new(
                LineKind::Next,
                vec![
                    Some(
                        SettingsButtonBuilder::new(
                            SettingsButtonKind::ContinueAll,
                            clone!(state => move || {
                                state.base.play_settings.next.signal_ref(|curr| {
                                    std::mem::discriminant(curr) == std::mem::discriminant(&Next::PlaceAll)
                                })
                            }),
                        )
                        .on_click(clone!(state => move || state.set_next(Next::PlaceAll)))
                        .build()
                    ),
                    Some(
                        SettingsButtonBuilder::new(
                            SettingsButtonKind::ContinueClick,
                            clone!(state => move || {
                                state.base.play_settings.next.signal_ref(|curr| {
                                    std::mem::discriminant(curr) == std::mem::discriminant(&Next::ClickContinue)
                                })
                            }),
                        )
                        .on_click(clone!(state => move || state.set_next(Next::ClickContinue)))
                        .build()
                    ),
                ],
            ),
            ModuleSettingsLine::new(
                LineKind::TimeLimit,
                vec![
                    Some(
                        SettingsButtonBuilder::new(
                            SettingsButtonKind::TimeLimitOff,
                            clone!(state => move || {
                                state.base.play_settings.has_time_limit
                                    .signal()
                                    .map(|flag| !flag)
                            }),
                        )
                        .on_click(clone!(state => move || state.set_has_time_limit(false)))
                        .build()
                    ),
                    Some(
                        SettingsButtonBuilder::new(
                            SettingsButtonKind::TimeLimit,
                            clone!(state => move || {
                                state.base.play_settings.has_time_limit
                                    .signal()
                            }),
                        )
                        .value(SettingsValue::new(
                            state.base.play_settings.time_limit.get(),
                            clone!(state => move |value| {
                                state.set_time_limit(value);
                            }),
                        ))
                        .on_click(clone!(state => move || state.set_has_time_limit(true)))
                        .build()
                    ),
                ],
            ),
            ModuleSettingsLine::new(
                LineKind::Hint,
                vec![
                    Some(
                        SettingsButtonBuilder::new(
                            SettingsButtonKind::HighlightOff,
                            clone!(state => move || {
                                state.base.play_settings.hint.signal_ref(|curr| {
                                    *curr == Hint::None
                                })
                            }),
                        )
                        .on_click(clone!(state => move || state.set_hint(Hint::None)))
                        .build()
                    ),
                    Some(
                        SettingsButtonBuilder::new(
                            SettingsButtonKind::Highlight,
                            clone!(state => move || {
                                state.base.play_settings.hint.signal_ref(|curr| {
                                    *curr == Hint::Highlight
                                })
                            }),
                        )
                        .on_click(clone!(state => move || state.set_hint(Hint::Highlight)))
                        .build()
                    ),
                ],
            ),
            // Note - not including scoring until player settings is resolved
        ],
    }))
}
