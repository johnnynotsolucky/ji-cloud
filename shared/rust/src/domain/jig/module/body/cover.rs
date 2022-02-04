use crate::domain::jig::module::{
    body::{Body, BodyConvert, BodyExt, StepExt, ThemeId, _groups::design::*},
    ModuleKind,
};
use serde::{de::IntoDeserializer, Deserialize, Serialize};
use std::collections::HashSet;
use std::convert::TryFrom;

/// The body for [`Cover`](crate::domain::jig::module::ModuleKind::Cover) modules.
#[derive(Clone, Serialize, Deserialize, Debug)]
pub struct ModuleData {
    /// The content
    pub content: Option<Content>,
}

// cover always has data
impl Default for ModuleData {
    fn default() -> Self {
        Self {
            content: Some(Content::default()),
        }
    }
}

impl BodyExt<(), Step> for ModuleData {
    fn as_body(&self) -> Body {
        Body::Cover(self.clone())
    }

    fn is_complete(&self) -> bool {
        self.content.is_some()
    }

    fn kind() -> ModuleKind {
        ModuleKind::Cover
    }

    fn new_with_mode_and_theme(_mode: (), theme: ThemeId) -> Self {
        ModuleData {
            content: Some(Content {
                base: BaseContent {
                    theme,
                    ..Default::default()
                },
                ..Default::default()
            }),
        }
    }

    fn mode(&self) -> Option<()> {
        None
    }

    fn requires_choose_mode(&self) -> bool {
        false
    }

    fn set_editor_state_step(&mut self, step: Step) {
        if let Some(content) = self.content.as_mut() {
            content.editor_state.step = step;
        }
    }
    fn set_editor_state_steps_completed(&mut self, steps_completed: HashSet<Step>) {
        if let Some(content) = self.content.as_mut() {
            content.editor_state.steps_completed = steps_completed;
        }
    }

    fn get_editor_state_step(&self) -> Option<Step> {
        self.content
            .as_ref()
            .map(|content| content.editor_state.step)
    }

    fn get_editor_state_steps_completed(&self) -> Option<HashSet<Step>> {
        self.content
            .as_ref()
            .map(|content| content.editor_state.steps_completed.clone())
    }

    fn set_theme(&mut self, theme_id: ThemeId) {
        if let Some(content) = self.content.as_mut() {
            content.base.theme = theme_id;
        }
    }

    fn get_theme(&self) -> Option<ThemeId> {
        self.content.as_ref().map(|content| content.base.theme)
    }
}

impl BodyConvert for ModuleData {}

impl TryFrom<Body> for ModuleData {
    type Error = &'static str;

    fn try_from(body: Body) -> Result<Self, Self::Error> {
        match body {
            Body::Cover(data) => Ok(data),
            _ => Err("cannot convert body to cover!"),
        }
    }
}

/// The body for [`Cover`](crate::domain::jig::module::ModuleKind::Cover) modules.
#[derive(Default, Clone, Serialize, Deserialize, Debug)]
pub struct Content {
    /// The editor state
    pub editor_state: EditorState,

    /// The base content for all design modules
    pub base: BaseContent,
}

/// Editor state
#[derive(Default, Clone, Serialize, Deserialize, Debug)]
pub struct EditorState {
    /// the current step
    pub step: Step,

    /// the completed steps
    pub steps_completed: HashSet<Step>,
}

// TODO Currently there exists some Cover modules with an editor_state which has the step set to
// Four, or Four in the steps_completed field. The workaround here is to tell serde to make use of
// the custom Deserialize implementation (and Serialize) by using itself as a remote type.
// See https://serde.rs/remote-derive.html

/// The Steps
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(remote = "Step")]
pub enum Step {
    /// Step 1
    One,
    /// Step 2
    Two,
    /// Step 3
    Three,
}

impl<'de> Deserialize<'de> for Step {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: serde::Deserializer<'de>,
    {
        let value = String::deserialize(deserializer)?;
        if value == "Four" {
            Ok(Self::Three)
        } else {
            Step::deserialize(value.into_deserializer())
        }
    }
}

impl Serialize for Step {
    fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: serde::Serializer,
    {
        Step::serialize(&self, serializer)
    }
}

impl Default for Step {
    fn default() -> Self {
        Self::One
    }
}

impl StepExt for Step {
    fn next(&self) -> Option<Self> {
        match self {
            Self::One => Some(Self::Two),
            Self::Two => Some(Self::Three),
            Self::Three => None,
        }
    }

    fn as_number(&self) -> usize {
        match self {
            Self::One => 1,
            Self::Two => 2,
            Self::Three => 3,
        }
    }

    fn label(&self) -> &'static str {
        const STR_DESIGN: &'static str = "Design";
        const STR_CONTENT: &'static str = "Content";
        const STR_PREVIEW: &'static str = "Preview";

        match self {
            Self::One => STR_DESIGN,
            Self::Two => STR_CONTENT,
            Self::Three => STR_PREVIEW,
        }
    }

    fn get_list() -> Vec<Self> {
        vec![Self::One, Self::Two, Self::Three]
    }
    fn get_preview() -> Self {
        Self::Three
    }
}
