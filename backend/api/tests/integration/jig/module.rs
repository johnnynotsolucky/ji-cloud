use http::StatusCode;

use macros::test_service;
use serde_json::json;
use shared::domain::{
    asset::{AssetId, AssetType},
    jig::JigId,
    module::{body::memory, ModuleBody, ModuleCreateRequest, ModuleKind, ModuleUpdateRequest},
};
use sqlx::postgres::{PgConnectOptions, PgPoolOptions};
use uuid::Uuid;

use crate::{
    fixture::Fixture,
    helpers::{setup_service, LoginExt},
};

#[test_service(
    setup = "setup_service",
    fixtures("Fixture::MetaKinds", "Fixture::User", "Fixture::Jig")
)]
async fn get_live(port: u16) -> anyhow::Result<()> {
    let name = "get_live";

    let client = reqwest::Client::new();

    let resp = client
        .get(&format!(
            "http://0.0.0.0:{}/v1/{}/module/live/a6b24970-1dd7-11ec-8426-57136b411853",
            port,
            AssetType::Jig
        ))
        .login()
        .send()
        .await?
        .error_for_status()?;

    assert_eq!(resp.status(), StatusCode::OK);

    let body: serde_json::Value = resp.json().await?;

    insta::assert_json_snapshot!(format!("{}",name), body, {".**.updated_at" => "[timestamp]"});

    Ok(())
}

#[test_service(
    setup = "setup_service",
    fixtures("Fixture::MetaKinds", "Fixture::User", "Fixture::Jig")
)]
async fn create_default(port: u16) -> anyhow::Result<()> {
    let name = "create_default";

    let client = reqwest::Client::new();

    let resp = client
        .post(&format!("http://0.0.0.0:{}/v1/module/draft", port))
        .json(&ModuleCreateRequest {
            parent_id: AssetId::JigId(JigId(Uuid::parse_str(
                "0cc084bc-7c83-11eb-9f77-e3218dffb008",
            )?)),
            body: ModuleBody::new(ModuleKind::Cover),
        })
        .login()
        .send()
        .await?
        .error_for_status()?;

    assert_eq!(resp.status(), StatusCode::CREATED);

    let body: serde_json::Value = resp.json().await?;

    insta::assert_json_snapshot!(format!("{}-1",name), body, {
        ".**.id" => "[id]",
        ".**.created_at" => "[created_at]",
        ".**.updated_at" => "[updated_at]"});

    let id = body
        .get("id")
        .expect("has id")
        .as_str()
        .expect("convert to str");

    let resp = client
        .get(&format!(
            "http://0.0.0.0:{}/v1/{}/module/draft/{}",
            port,
            AssetType::Jig,
            id
        ))
        .query(&[("q", "unique")])
        .login()
        .send()
        .await?
        .error_for_status()?;

    let body: serde_json::Value = resp.json().await?;

    insta::assert_json_snapshot!(format!("{}-2",name), body, {
        ".**.id" => "[id]",
        ".**.stable_id" => "[stable_id]",
        ".**.created_at" => "[created_at]",
        ".**.updated_at" => "[updated_at]"});

    Ok(())
}

#[test_service(
    setup = "setup_service",
    fixtures("Fixture::MetaKinds", "Fixture::User", "Fixture::Jig")
)]
async fn update_empty(port: u16) -> anyhow::Result<()> {
    let name = "update_empty";

    let client = reqwest::Client::new();

    let resp = client
        .patch(&format!(
            "http://0.0.0.0:{}/v1/module/draft/a6b24a42-1dd7-11ec-8426-a7165f9281a2",
            port,
        ))
        .json(&ModuleUpdateRequest {
            parent_id: AssetId::JigId(JigId(uuid::Uuid::parse_str(
                "0cc084bc-7c83-11eb-9f77-e3218dffb008",
            )?)),

            is_complete: None,
            body: None,
            index: None,
        })
        .login()
        .send()
        .await?
        .error_for_status()?;

    assert_eq!(resp.status(), StatusCode::NO_CONTENT);

    let resp = client
        .get(&format!(
            "http://0.0.0.0:{}/v1/{}/module/draft/a6b24a42-1dd7-11ec-8426-a7165f9281a2",
            port,
            AssetType::Jig
        ))
        .login()
        .send()
        .await?
        .error_for_status()?;

    assert_eq!(resp.status(), StatusCode::OK);

    let body: serde_json::Value = resp.json().await?;

    insta::assert_json_snapshot!(format!("{}",name), body, {".**.updated_at" => "[timestamp]"});

    Ok(())
}

#[test_service(
    setup = "setup_service",
    fixtures("Fixture::MetaKinds", "Fixture::User", "Fixture::Jig")
)]
async fn update_contents(port: u16) -> anyhow::Result<()> {
    let name = "update_contents";

    let client = reqwest::Client::new();

    let resp = client
        .patch(&format!(
            "http://0.0.0.0:{}/v1/module/draft/a6b24a06-1dd7-11ec-8426-635a3a7ea572",
            port
        ))
        .login()
        .json(&ModuleUpdateRequest {
            parent_id: AssetId::JigId(JigId(uuid::Uuid::parse_str(
                "0cc084bc-7c83-11eb-9f77-e3218dffb008",
            )?)),
            body: Some(ModuleBody::MemoryGame(memory::ModuleData {
                content: Some(memory::Content {
                    ..memory::Content::default()
                }),
            })),
            is_complete: Some(true),
            index: None,
        })
        .send()
        .await?
        .error_for_status()?;

    assert_eq!(resp.status(), StatusCode::NO_CONTENT);

    let resp = client
        .get(&format!(
            "http://0.0.0.0:{}/v1/{}/module/draft/a6b24a06-1dd7-11ec-8426-635a3a7ea572",
            port,
            AssetType::Jig
        ))
        .login()
        .send()
        .await?
        .error_for_status()?;

    assert_eq!(resp.status(), StatusCode::OK);

    let body: serde_json::Value = resp.json().await?;

    insta::assert_json_snapshot!(format!("{}-1",name), body, {".**.updated_at" => "[timestamp]"});

    let resp = client
        .get(&format!(
            "http://0.0.0.0:{}/v1/{}/module/draft/a6b24a06-1dd7-11ec-8426-635a3a7ea572",
            port,
            AssetType::Jig
        ))
        .login()
        .send()
        .await?
        .error_for_status()?;

    assert_eq!(resp.status(), StatusCode::OK);

    let body: serde_json::Value = resp.json().await?;

    insta::assert_json_snapshot!(format!("{}-2",name), body, {".**.updated_at" => "[timestamp]"});

    Ok(())
}

#[test_service(
    setup = "setup_service",
    fixtures(
        "Fixture::MetaKinds",
        "Fixture::User",
        "Fixture::Jig",
        "Fixture::CategoryOrdering"
    )
)]
async fn drag_up_down_modules(port: u16) -> anyhow::Result<()> {
    let name = "drag_up_down_modules";

    let client = reqwest::Client::new();

    let jig_id = "0cc084bc-7c83-11eb-9f77-e3218dffb008".to_string();
    let module_id = "a6b24a42-1dd7-11ec-8426-a7165f9281a2".to_string();

    let resp = client
        .get(&format!("http://0.0.0.0:{}/v1/jig/{jig_id}/draft", port))
        .login()
        .send()
        .await?
        .error_for_status()?;

    let body: serde_json::Value = resp.json().await?;

    insta::assert_json_snapshot!(
        format!("{}-1",name),
        body, {
            ".**.lastEdited" => "[last_edited]",
            ".**.feedbackPositive" => "[audio]",
            ".**.feedbackNegative" => "[audio]",
        }
    );

    let resp = client
        .patch(&format!(
            "http://0.0.0.0:{}/v1/module/draft/{module_id}",
            port
        ))
        .json(&json!({
            "jigId": "0cc084bc-7c83-11eb-9f77-e3218dffb008",
            "index": 2,
        }))
        .login()
        .send()
        .await?
        .error_for_status()?;

    assert_eq!(resp.status(), StatusCode::NO_CONTENT);

    let resp = client
        .get(&format!("http://0.0.0.0:{}/v1/jig/{jig_id}/draft", port))
        .login()
        .send()
        .await?
        .error_for_status()?;

    let body: serde_json::Value = resp.json().await?;

    insta::assert_json_snapshot!(
        format!("{}-2",name),
        body, {
            ".**.lastEdited" => "[last_edited]",
            ".**.feedbackPositive" => "[audio]",
            ".**.feedbackNegative" => "[audio]",
        }
    );

    Ok(())
}
