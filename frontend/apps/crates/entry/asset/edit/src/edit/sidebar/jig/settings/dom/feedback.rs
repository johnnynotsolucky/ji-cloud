use core::hash::Hash;
use std::{collections::HashSet, rc::Rc};

use components::audio::mixer::{AudioHandle, AudioPath, AUDIO_MIXER};
use dominator::{clone, html, Dom};

use crate::edit::sidebar::jig::settings::{
    actions::{set_active_popup, update_jig_settings},
    dom::STR_BACK_TO_SETTINGS,
    state::{ActiveSettingsPopup, FeedbackTab},
};
use futures_signals::signal::{Mutable, MutableLockMut, SignalExt};
use shared::domain::jig::{AudioFeedbackNegative, AudioFeedbackPositive};
use utils::{asset::JigAudioExt, events};

use super::super::state::JigSettings;

const STR_CORRECT: &str = "Correct answer";
const STR_MISTAKE: &str = "Mistake";

impl JigSettings {
    pub fn render_feedback(self: &Rc<Self>, tab: FeedbackTab) -> Dom {
        let state = self;
        html!("jig-audio-body", {
            .prop("slot", "overlay")
            .prop("kind", "feedback")
            .children(&mut [
                html!("label", {
                    .prop("slot", "correct-mistake")
                    .child(html!("input", {
                        .prop("name", "correct-mistake")
                        .prop("type", "radio")
                        .prop("checked", tab == FeedbackTab::Positive)
                        .event(clone!(state => move |_:events::Input| {
                            state.active_popup.set(Some(ActiveSettingsPopup::Feedback(FeedbackTab::Positive)));
                        }))
                    }))
                    .text(STR_CORRECT)
                }),
                html!("label", {
                    .prop("slot", "correct-mistake")
                    .child(html!("input", {
                        .prop("name", "correct-mistake")
                        .prop("type", "radio")
                        .prop("checked", tab == FeedbackTab::Negative)
                        .event(clone!(state => move |_:events::Input| {
                            state.active_popup.set(Some(ActiveSettingsPopup::Feedback(FeedbackTab::Negative)));
                        }))
                    }))
                    .text(STR_MISTAKE)
                }),
                html!("button-rect", {
                    .prop("kind", "text")
                    .prop("slot", "back")
                    .prop("color", "blue")
                    .child(html!("fa-icon", {.prop("icon", "fa-light fa-chevron-left")}))
                    .text(STR_BACK_TO_SETTINGS)
                    .event(clone!(state => move|_: events::Click| {
                        set_active_popup(Rc::clone(&state), ActiveSettingsPopup::Main);
                    }))
                }),
                html!("button-icon", {
                    .prop("icon", "x")
                    .prop("slot", "close")
                    .event(clone!(state => move |_:events::Click| {
                        state.active_popup.set(None);
                    }))
                }),
                // html!("input-search", {
                //     .prop("slot", "search")
                // }),
            ])
            .apply(|dom| {
                match tab {
                    FeedbackTab::Positive => {
                        let audio_handles: Vec<Mutable<Option<AudioHandle>>> = AudioFeedbackPositive::variants().iter().map(|_| Mutable::new(None)).collect();
                        let audio_handles = Rc::new(audio_handles);

                        dom.children(AudioFeedbackPositive::variants().iter().enumerate().map(clone!(state, audio_handles => move|(index, option)| {
                            state.feedback_line(state.jig.feedback_positive.clone(), option, audio_handles.clone(), index)
                        })).collect::<Vec<Dom>>())
                        .after_removed(clone!(audio_handles => move |_| {
                            for audio_handle in audio_handles.iter() {
                                match &*audio_handle.lock_ref() {
                                    None => {},
                                    Some(audio_handle) => {
                                        audio_handle.pause();
                                    },
                                }
                            }
                        }))
                    },
                    FeedbackTab::Negative => {
                        let audio_handles: Vec<Mutable<Option<AudioHandle>>> = AudioFeedbackNegative::variants().iter().map(|_| Mutable::new(None)).collect();
                        let audio_handles = Rc::new(audio_handles);

                        dom.children(AudioFeedbackNegative::variants().iter().enumerate().map(clone!(state, audio_handles => move|(index, option)| {
                            state.feedback_line(state.jig.feedback_negative.clone(), option, audio_handles.clone(), index)
                        })).collect::<Vec<Dom>>())
                        .after_removed(clone!(audio_handles => move |_| {
                            for audio_handle in audio_handles.iter() {
                                match &*audio_handle.lock_ref() {
                                    None => {},
                                    Some(audio_handle) => {
                                        audio_handle.pause();
                                    },
                                }
                            }
                        }))
                    },
                }
            })
        })
    }

    fn feedback_line<'a, T>(
        self: &Rc<Self>,
        list: Mutable<HashSet<T>>,
        option: &T,
        audio_handles: Rc<Vec<Mutable<Option<AudioHandle>>>>,
        index: usize,
    ) -> Dom
    where
        T: Hash + Eq + Clone + JigAudioExt + Into<AudioPath<'a>> + std::fmt::Debug + 'static,
    {
        let state = self;

        let audio_handle = &audio_handles[index];

        html!("jig-audio-line", {
            .prop("slot", "lines")
            .prop("label", option.display_name())
            .prop_signal("playing", audio_handle.signal_ref(|x| x.is_some()))
            .children(&mut [
                html!("input", {
                    .prop("slot", "checkbox")
                    .prop("type", "checkbox")
                    .prop_signal("checked", list.signal_cloned().map(clone!(option => move|list| {
                        list.contains(&option)
                    })))
                    .event(clone!(state, option => move |_ :events::Click| {
                        let mut list = list.lock_mut();
                        match list.contains(&option) {
                            true => list.remove(&option),
                            false => list.insert(option.clone()),
                        };
                        drop(list);
                        update_jig_settings(Rc::clone(&state));
                    }))
                }),
                html!("jig-audio-play-pause", {
                    .prop("slot", "play-pause")
                    .prop_signal("mode", audio_handle.signal_ref(|audio_handle| {
                        match audio_handle {
                            Some(_) => "pause",
                            None => "play",
                        }
                    }))
                    .event(clone!(option, audio_handles => move |_ :events::Click| {
                        let on_ended = clone!(audio_handles => move|| {
                            audio_handles[index].set(None);
                        });

                        let mut audio_handles = audio_handles.iter().map(|x| x.lock_mut()).collect::<Vec<MutableLockMut<Option<AudioHandle>>>>();

                        match *audio_handles[index] {
                            Some(_) => *audio_handles[index] = None,
                            None => {
                                audio_handles = audio_handles.into_iter().map(|mut o| {
                                    *o = None;
                                    o
                                }).collect();

                                let path:AudioPath = option.clone().into();

                                let handle = AUDIO_MIXER.with(move |mixer| mixer.play_on_ended(path, false, on_ended));
                                *audio_handles[index] = Some(handle);
                            },
                        };
                    }))
                }),
            ])
        })
    }
}
