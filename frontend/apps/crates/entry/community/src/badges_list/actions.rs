use std::rc::Rc;

use dominator::clone;
use shared::{api::endpoints, domain::badge::BadgeBrowseQuery};
use utils::prelude::ApiEndpointExt;

use super::BadgesList;

impl BadgesList {
    pub fn load_badges(self: &Rc<Self>) {
        let state = self;

        state.loader.load(clone!(state => async move {
            let req = BadgeBrowseQuery {
                page: Some(state.active_page.get() - 1),
                page_limit: Some(state.items_per_page),
                ..Default::default()
            };

            match endpoints::badge::Browse::api_no_auth(Some(req)).await {
                Ok(res) => {
                    state.badges.set(Some(res.badges));
                    let page_count = page_count(res.total_badge_count as u32, state.items_per_page);
                    state.total_pages.set(page_count);
                },
                Err(_) => todo!(),
            }
        }));
    }
}

fn page_count(total: u32, items_per_page: u32) -> u32 {
    let total = total as f32;
    let items_per_page = items_per_page as f32;
    let page_count = total / items_per_page;
    let page_count = page_count.ceil();
    page_count as u32
}
