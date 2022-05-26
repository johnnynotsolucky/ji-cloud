use super::state::*;
use shared::{
    api::endpoints::{self, ApiEndpoint},
    domain::{
        jig::JigFocus,
        module::{
            body::{BodyExt, ModeExt, StepExt},
            LiteModule, ModuleCreateRequest, ModuleId, ModuleKind,
        },
        CreateResponse,
    },
    error::EmptyError,
};
use utils::{
    iframe::{IframeAction, IframeMessageExt, ModuleToJigEditorMessage},
    prelude::*,
};

impl PostPreview {
    pub fn next(&self) {
        let msg = IframeAction::new(ModuleToJigEditorMessage::Next);

        if msg.try_post_message_to_editor().is_err() {
            log::info!("Couldn't post message to top... redirect!");

            let route: String = Route::Asset(AssetRoute::Edit(AssetEditRoute::Jig(
                self.jig_id,
                JigFocus::Modules, // only module focused jigs are should be here
                JigEditRoute::Landing,
            )))
            .into();
            dominator::routing::go_to_url(&route);
        }
    }

    pub fn publish(&self) {
        let msg = IframeAction::new(ModuleToJigEditorMessage::Publish);

        if msg.try_post_message_to_editor().is_err() {
            log::info!("Couldn't post message to top... redirect!");

            let route: String = Route::Asset(AssetRoute::Edit(AssetEditRoute::Jig(
                self.jig_id,
                JigFocus::Modules, // only module focused jigs are should be here
                JigEditRoute::Landing,
            )))
            .into();
            dominator::routing::go_to_url(&route);
        }
    }

    pub fn duplicate_module<RawData, Mode, Step>(&self, target_kind: ModuleKind, raw_data: RawData)
    where
        RawData: BodyExt<Mode, Step> + 'static,
        Mode: ModeExt + 'static,
        Step: StepExt + 'static,
    {
        let target_body = raw_data.convert_to_body(target_kind).unwrap_ji();

        let req = ModuleCreateRequest {
            body: target_body,
            parent_id: self.jig_id.into(),
        };

        let jig_id = self.jig_id;

        self.loader.load(async move {
            let res = api_with_auth::<CreateResponse<ModuleId>, EmptyError, ModuleCreateRequest>(
                endpoints::module::Create::PATH,
                endpoints::module::Create::METHOD,
                Some(req),
            )
            .await;

            match res {
                Ok(res) => {
                    let module_id = res.id;

                    let module = LiteModule {
                        id: module_id,
                        kind: target_kind,
                        is_complete: raw_data.is_complete(),
                    };

                    let msg = IframeAction::new(ModuleToJigEditorMessage::AppendModule(module));

                    if msg.try_post_message_to_editor().is_err() {
                        log::info!("Couldn't post message to parent... redirect!");
                        let route: String = Route::Asset(AssetRoute::Edit(AssetEditRoute::Jig(
                            jig_id,
                            JigFocus::Modules, // only module focused jigs are should be here
                            JigEditRoute::Module(module_id),
                        )))
                        .into();
                        dominator::routing::go_to_url(&route);
                    }
                }
                Err(_) => {
                    log::error!("request to create module failed!");
                }
            }
        });
    }
}
