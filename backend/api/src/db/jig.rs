use crate::translate::translate_text;
use anyhow::Context;
use serde_json::value::Value;
use shared::domain::jig::codes::JigCode;
use shared::domain::jig::{AdminJigExport, JigUpdateAdminDataRequest};
use shared::domain::module::StableModuleId;
use shared::domain::playlist::{PlaylistAdminData, PlaylistRating};
use shared::domain::{
    additional_resource::{AdditionalResource, AdditionalResourceId as AddId, ResourceContent},
    asset::{DraftOrLive, OrderBy, PrivacyLevel},
    category::CategoryId,
    jig::{
        AudioBackground, AudioEffects, AudioFeedbackNegative, AudioFeedbackPositive, JigAdminData,
        JigData, JigId, JigPlayerSettings, JigRating, JigResponse, TextDirection,
    },
    meta::{AffiliationId, AgeRangeId, ResourceTypeId as TypeId},
    module::{body::ThemeId, LiteModule, ModuleId, ModuleKind},
    playlist::{PlaylistData, PlaylistId, PlaylistResponse},
    user::{UserId, UserScope},
};
use sqlx::{types::Json, PgConnection, PgPool};
use std::collections::HashMap;
use tracing::{instrument, Instrument};
use uuid::Uuid;

use crate::error;

pub(crate) mod additional_resource;
pub(crate) mod codes;
pub(crate) mod curation;
pub(crate) mod module;
pub(crate) mod report;

pub async fn create(
    pool: &PgPool,
    display_name: &str,
    categories: &[CategoryId],
    age_ranges: &[AgeRangeId],
    affiliations: &[AffiliationId],
    creator_id: UserId,
    language: &str,
    description: &str,
    default_player_settings: &JigPlayerSettings,
) -> Result<JigId, CreateJigError> {
    let mut txn = pool.begin().await?;

    let draft_id = create_jig_data(
        &mut txn,
        display_name,
        categories,
        age_ranges,
        affiliations,
        language,
        description,
        default_player_settings,
        DraftOrLive::Draft,
    )
    .await?;

    let live_id = create_jig_data(
        &mut txn,
        display_name,
        categories,
        age_ranges,
        affiliations,
        language,
        description,
        default_player_settings,
        DraftOrLive::Live,
    )
    .await?;

    let jig = sqlx::query!(
        //language=SQL
        r#"insert into jig (creator_id, author_id, live_id, draft_id) values ($1, $1, $2, $3) returning id"#,
        creator_id.0,
        live_id,
        draft_id,
    )
    .fetch_one(&mut txn)
    .await?;

    sqlx::query!(
        // language=SQL
        r#"
insert into jig_play_count (jig_id, play_count)
values ($1, 0)
        "#,
        jig.id
    )
    .execute(&mut txn)
    .await?;

    txn.commit().await?;

    Ok(JigId(jig.id))
}

pub async fn create_jig_data(
    txn: &mut PgConnection,
    display_name: &str,
    categories: &[CategoryId],
    age_ranges: &[AgeRangeId],
    affiliations: &[AffiliationId],
    language: &str,
    description: &str,
    default_player_settings: &JigPlayerSettings,
    draft_or_live: DraftOrLive,
) -> Result<Uuid, CreateJigError> {
    log::warn!("description: {}", description);

    let jig_data = sqlx::query!(
        // language=SQL
        r#"
insert into jig_data
   (display_name, language, description, direction, scoring, drag_assist, draft_or_live)
values ($1, $2, $3, $4, $5, $6, $7)
returning id
"#,
        display_name,
        language,
        description,
        default_player_settings.direction as i16,
        default_player_settings.scoring,
        default_player_settings.drag_assist,
        draft_or_live as i16,
    )
    .fetch_one(&mut *txn)
    .await?;

    super::recycle_metadata(&mut *txn, "jig_data", jig_data.id, categories).await?;
    super::recycle_metadata(&mut *txn, "jig_data", jig_data.id, age_ranges).await?;
    super::recycle_metadata(&mut *txn, "jig_data", jig_data.id, affiliations).await?;

    Ok(jig_data.id)
}

/// Handle errors for creating a module when posting a Jig
/// This is here because the scope is limited to the above function
pub enum CreateJigError {
    Sqlx(sqlx::Error),
    DefaultModules(serde_json::Error),
    InternalServerError(anyhow::Error),
}

impl From<sqlx::Error> for CreateJigError {
    fn from(e: sqlx::Error) -> Self {
        Self::Sqlx(e)
    }
}

impl From<anyhow::Error> for CreateJigError {
    fn from(e: anyhow::Error) -> Self {
        Self::InternalServerError(e)
    }
}

impl From<serde_json::Error> for CreateJigError {
    fn from(e: serde_json::Error) -> Self {
        Self::DefaultModules(e)
    }
}

#[instrument(skip(pool))]
pub async fn get_one(
    pool: &PgPool,
    id: JigId,
    draft_or_live: DraftOrLive,
    user_id: Option<UserId>,
) -> anyhow::Result<Option<JigResponse>> {
    let res = sqlx::query!( //language=SQL
        r#"
with cte as (
    select id      as "jig_id",
           creator_id,
           author_id,
           liked_count,
           play_count,
           live_up_to_date,
           case
               when $2 = 0 then jig.draft_id
               when $2 = 1 then jig.live_id
               end as "draft_or_live_id",
           published_at,
           rating,
           blocked,
           curated,
           is_premium
    from jig
    left join jig_play_count on jig_play_count.jig_id = jig.id
    left join jig_admin_data "admin" on admin.jig_id = jig.id
    where id = $1
)
select cte.jig_id                                          as "jig_id: JigId",
        display_name,
        creator_id                                          as "creator_id: UserId",
        author_id                                           as "author_id: UserId",
        (select given_name || ' '::text || family_name
        from user_profile
        where user_profile.user_id = author_id)            as "author_name",
        created_at,
        updated_at,
        published_at,
        privacy_level                                       as "privacy_level!: PrivacyLevel",
        language,
        description,
        translated_description                              as "translated_description!: Json<HashMap<String, String>>",
        direction                                           as "direction: TextDirection",
        scoring,
        drag_assist,
        theme                                               as "theme: ThemeId",
        audio_background                                    as "audio_background: AudioBackground",
        liked_count,
        play_count,
        live_up_to_date,
        exists(select 1 from jig_like where jig_id = $1 and user_id = $3)    as "is_liked!",
        locked,
        other_keywords,
        translated_keywords,
        rating                                               as "rating?: JigRating",
        blocked                                              as "blocked",
        curated,
        is_premium                                           as "premium",
        array(select row (unnest(audio_feedback_positive))) as "audio_feedback_positive!: Vec<(AudioFeedbackPositive,)>",
        array(select row (unnest(audio_feedback_negative))) as "audio_feedback_negative!: Vec<(AudioFeedbackNegative,)>",
        array(
                select row (jig_data_module.id, jig_data_module.stable_id, kind, is_complete)
                from jig_data_module
                where jig_data_id = jig_data.id
                order by "index"
        )                                               as "modules!: Vec<(ModuleId, StableModuleId, ModuleKind, bool)>",
        array(select row (category_id)
                from jig_data_category
                where jig_data_id = cte.draft_or_live_id)     as "categories!: Vec<(CategoryId,)>",
        array(select row (affiliation_id)
                from jig_data_affiliation
                where jig_data_id = cte.draft_or_live_id)     as "affiliations!: Vec<(AffiliationId,)>",
        array(select row (age_range_id)
                from jig_data_age_range
                where jig_data_id = cte.draft_or_live_id)     as "age_ranges!: Vec<(AgeRangeId,)>",
        array(
                select row (jdar.id, jdar.display_name, resource_type_id, resource_content)
                from jig_data_additional_resource "jdar"
                where jdar.jig_data_id = cte.draft_or_live_id
    )                                                    as "additional_resource!: Vec<(AddId, String, TypeId, Value)>"
from jig_data
         inner join cte on cte.draft_or_live_id = jig_data.id
"#,
        id.0,
        draft_or_live as i16,
        user_id.map(|x| x.0)
    )
        .fetch_optional(pool).await?;

    let jig = res.map(|row| JigResponse {
        id: row.jig_id,
        published_at: row.published_at,
        creator_id: row.creator_id,
        author_id: row.author_id,
        author_name: row.author_name,
        likes: row.liked_count,
        plays: row.play_count,
        live_up_to_date: row.live_up_to_date,
        is_liked: row.is_liked,
        jig_data: JigData {
            created_at: row.created_at,
            draft_or_live,
            display_name: row.display_name,
            language: row.language,
            modules: row
                .modules
                .into_iter()
                .map(|(id, stable_id, kind, is_complete)| LiteModule {
                    id,
                    stable_id,
                    kind,
                    is_complete,
                })
                .collect(),
            categories: row.categories.into_iter().map(|(it,)| it).collect(),
            last_edited: row.updated_at,
            description: row.description,
            default_player_settings: JigPlayerSettings {
                direction: row.direction,
                scoring: row.scoring,
                drag_assist: row.drag_assist,
            },
            theme: row.theme,
            age_ranges: row.age_ranges.into_iter().map(|(it,)| it).collect(),
            affiliations: row.affiliations.into_iter().map(|(it,)| it).collect(),
            additional_resources: row
                .additional_resource
                .into_iter()
                .map(
                    |(id, display_name, resource_type_id, resource_content)| AdditionalResource {
                        id,
                        display_name,
                        resource_type_id,
                        resource_content: serde_json::from_value::<ResourceContent>(
                            resource_content,
                        )
                        .unwrap(),
                    },
                )
                .collect(),
            audio_background: row.audio_background,
            audio_effects: AudioEffects {
                feedback_positive: row
                    .audio_feedback_positive
                    .into_iter()
                    .map(|(it,)| it)
                    .collect(),
                feedback_negative: row
                    .audio_feedback_negative
                    .into_iter()
                    .map(|(it,)| it)
                    .collect(),
            },
            privacy_level: row.privacy_level,
            locked: row.locked,
            other_keywords: row.other_keywords,
            translated_keywords: row.translated_keywords,
            translated_description: row.translated_description.0,
        },
        admin_data: JigAdminData {
            rating: row.rating,
            blocked: row.blocked,
            curated: row.curated,
            premium: row.premium,
        },
    });

    Ok(jig)
}

#[instrument(skip(db))]
pub async fn get_by_ids(
    db: &PgPool,
    ids: &[JigId],
    draft_or_live: DraftOrLive,
    user_id: Option<UserId>,
) -> sqlx::Result<Vec<JigResponse>> {
    let mut txn = db.begin().await?;

    let jig = sqlx::query!(
        //language=SQL
        r#"
select jig.id                                       as "id!: JigId",
       creator_id                               as "creator_id: UserId",
       author_id                                as "author_id: UserId",
       (select given_name || ' '::text || family_name
        from user_profile
        where user_profile.user_id = author_id) as "author_name",
       live_id                                  as "live_id!",
       draft_id                                 as "draft_id!",
       published_at,
       liked_count                              as "liked_count!",
       live_up_to_date                          as "live_up_to_date!",
       exists(select 1 from jig_like where jig_id = jig.id and user_id = $2) as "is_liked!",
       (
           select play_count
           from jig_play_count
           where jig_play_count.jig_id = jig.id
       )                                        as "play_count!",
       rating                                   as "rating?: JigRating",
       blocked                                  as "blocked!",
       curated                                  as "curated!",
       is_premium                               as "premium!"
from jig
         inner join unnest($1::uuid[])
    with ordinality t(id, ord) using (id)
    inner join jig_admin_data "admin" on admin.jig_id = jig.id
    order by ord asc
    "#,
        &ids.iter().map(|i| i.0).collect::<Vec<Uuid>>(),
        user_id.map(|x| x.0)
    )
    .fetch_all(&mut txn)
    .instrument(tracing::info_span!("query jigs"))
    .await?;

    let jig_data_ids: Vec<Uuid> = match draft_or_live {
        DraftOrLive::Draft => jig.iter().map(|it| it.draft_id).collect(),
        DraftOrLive::Live => jig.iter().map(|it| it.live_id).collect(),
    };

    let jig_data = sqlx::query!(
        //language=SQL
        r#"
select id,
       display_name                                                                  as "display_name!",
       created_at                                                                    as "created_at!",
       updated_at,
       language                                                                      as "language!",
       description                                                                   as "description!",
       translated_description                                                        as "translated_description!: Json<HashMap<String,String>>",
       direction                                                                     as "direction!: TextDirection",
       scoring                                                                       as "scoring!",
       drag_assist                                                                   as "drag_assist!",
       theme                                                                         as "theme!: ThemeId",
       audio_background                                                              as "audio_background!: Option<AudioBackground>",
       array(select row (unnest(audio_feedback_positive)))                           as "audio_feedback_positive!: Vec<(AudioFeedbackPositive,)>",
       array(select row (unnest(audio_feedback_negative)))                           as "audio_feedback_negative!: Vec<(AudioFeedbackNegative,)>",
       array(
                select row (jig_data_module.id, jig_data_module.stable_id, kind, is_complete)
                from jig_data_module
                where jig_data_id = jig_data.id
                order by "index"
       )                                               as "modules!: Vec<(ModuleId, StableModuleId, ModuleKind, bool)>",
       array(select row (category_id)
             from jig_data_category
             where jig_data_id = jig_data.id)     as "categories!: Vec<(CategoryId,)>",
       array(select row (affiliation_id)
             from jig_data_affiliation
             where jig_data_id = jig_data.id)     as "affiliations!: Vec<(AffiliationId,)>",
       array(select row (age_range_id)
             from jig_data_age_range
             where jig_data_id = jig_data.id)     as "age_ranges!: Vec<(AgeRangeId,)>",
       array(
                select row (jdar.id, jdar.display_name, resource_type_id, resource_content)
                from jig_data_additional_resource "jdar"
                where jdar.jig_data_id = jig_data.id
            )                                               as "additional_resource!: Vec<(AddId, String, TypeId, Value)>",
       privacy_level                              as "privacy_level!: PrivacyLevel",
       locked                                     as "locked!",
       other_keywords                             as "other_keywords!",
       translated_keywords                        as "translated_keywords!"
from jig_data
inner join unnest($1::uuid[])
    with ordinality t(id, ord) using (id)
order by ord asc
"#,
        &jig_data_ids
    )
        .fetch_all(&mut txn)
        .instrument(tracing::info_span!("query jig_data"))
        .await?;

    let v = jig
        .into_iter()
        .zip(jig_data.into_iter())
        .map(|(jig_row, jig_data_row)| JigResponse {
            id: jig_row.id,
            published_at: jig_row.published_at,
            creator_id: jig_row.creator_id,
            author_id: jig_row.author_id,
            author_name: jig_row.author_name,
            likes: jig_row.liked_count,
            plays: jig_row.play_count,
            live_up_to_date: jig_row.live_up_to_date,
            is_liked: jig_row.is_liked,
            jig_data: JigData {
                created_at: jig_data_row.created_at,
                draft_or_live,
                display_name: jig_data_row.display_name,
                language: jig_data_row.language,
                modules: jig_data_row
                    .modules
                    .into_iter()
                    .map(|(id, stable_id, kind, is_complete)| LiteModule {
                        id,
                        stable_id,
                        kind,
                        is_complete,
                    })
                    .collect(),
                categories: jig_data_row
                    .categories
                    .into_iter()
                    .map(|(it,)| it)
                    .collect(),
                last_edited: jig_data_row.updated_at,
                description: jig_data_row.description,
                default_player_settings: JigPlayerSettings {
                    direction: jig_data_row.direction,
                    scoring: jig_data_row.scoring,
                    drag_assist: jig_data_row.drag_assist,
                },
                theme: jig_data_row.theme,
                age_ranges: jig_data_row
                    .age_ranges
                    .into_iter()
                    .map(|(it,)| it)
                    .collect(),
                affiliations: jig_data_row
                    .affiliations
                    .into_iter()
                    .map(|(it,)| it)
                    .collect(),
                additional_resources: jig_data_row
                    .additional_resource
                    .into_iter()
                    .map(|(id, display_name, resource_type_id, resource_content)| {
                        AdditionalResource {
                            id,
                            display_name,
                            resource_type_id,
                            resource_content: serde_json::from_value::<ResourceContent>(
                                resource_content,
                            )
                            .unwrap(),
                        }
                    })
                    .collect(),
                audio_background: jig_data_row.audio_background,
                audio_effects: AudioEffects {
                    feedback_positive: jig_data_row
                        .audio_feedback_positive
                        .into_iter()
                        .map(|(it,)| it)
                        .collect(),
                    feedback_negative: jig_data_row
                        .audio_feedback_negative
                        .into_iter()
                        .map(|(it,)| it)
                        .collect(),
                },
                privacy_level: jig_data_row.privacy_level,
                locked: jig_data_row.locked,
                other_keywords: jig_data_row.other_keywords,
                translated_keywords: jig_data_row.translated_keywords,
                translated_description: jig_data_row.translated_description.0,
            },
            admin_data: JigAdminData {
                rating: jig_row.rating,
                blocked: jig_row.blocked,
                curated: jig_row.curated,
                premium: jig_row.premium,
            },
        })
        .collect();

    txn.rollback().await?;

    Ok(v)
}

#[instrument(skip(db))]
pub async fn browse(
    db: &sqlx::Pool<sqlx::Postgres>,
    author_id: Option<UserId>,
    draft_or_live: Option<DraftOrLive>,
    privacy_level: Vec<PrivacyLevel>,
    blocked: Option<bool>,
    page: i32,
    page_limit: u32,
    resource_types: Vec<Uuid>,
    order_by: Option<OrderBy>,
    user_id: Option<UserId>,
) -> sqlx::Result<Vec<JigResponse>> {
    let mut txn = db.begin().await?;

    let privacy_level: Vec<i16> = privacy_level.iter().map(|x| *x as i16).collect();

    let jig_data = sqlx::query!(
    //language=SQL
    r#"
with cte as (
    select array_agg(jd.id)
    from jig_data "jd"
          inner join jig on (draft_id = jd.id or (live_id = jd.id and jd.last_synced_at is not null and published_at is not null))
          left join jig_admin_data "admin" on admin.jig_id = jig.id
          left join jig_data_additional_resource "resource" on jd.id = resource.jig_data_id
    where (author_id = $1 or $1 is null)
        and (jd.draft_or_live = $2 or $2 is null)
        and (blocked = $3 or $3 is null)
        and (jd.privacy_level = any($4) or $4 = array[]::smallint[])
        and (resource.resource_type_id = any($5) or $5 = array[]::uuid[])
    group by updated_at, created_at, jig.published_at, admin.jig_id
    order by case when $6 = 0 then created_at
        when $6 = 1 then published_at
        else coalesce(updated_at, created_at)
  end desc, jig_id
),
cte1 as (
    select * from unnest(array((select cte.array_agg[1] from cte))) with ordinality t(id
   , ord) order by ord
)
select jig.id                                              as "jig_id: JigId",
    privacy_level                                       as "privacy_level: PrivacyLevel",
    creator_id                                          as "creator_id?: UserId",
    author_id                                           as "author_id?: UserId",
    (select given_name || ' '::text || family_name
        from user_profile
     where user_profile.user_id = author_id)            as "author_name",
    created_at,
    updated_at,
    published_at,
    liked_count,
    live_up_to_date,
    exists(select 1 from jig_like where jig_id = jig.id and user_id = $9) as "is_liked!",
    (
         select play_count
         from jig_play_count
         where jig_play_count.jig_id = jig.id
    )                                                   as "play_count!",
   display_name                                                                  as "display_name!",
   language                                                                      as "language!",
   description                                                                   as "description!",
   translated_description                                                        as "translated_description!: Json<HashMap<String,String>>",
   direction                                                                     as "direction!: TextDirection",
   scoring                                                                       as "scoring!",
   drag_assist                                                                   as "drag_assist!",
   theme                                                                         as "theme!: ThemeId",
   audio_background                                                              as "audio_background!: Option<AudioBackground>",
   draft_or_live                                                                 as "draft_or_live!: DraftOrLive",
   array(select row (unnest(audio_feedback_positive)))                           as "audio_feedback_positive!: Vec<(AudioFeedbackPositive,)>",
   array(select row (unnest(audio_feedback_negative)))                           as "audio_feedback_negative!: Vec<(AudioFeedbackNegative,)>",
   array(
           select row (jig_data_module.id, jig_data_module.stable_id, kind, is_complete)
           from jig_data_module
           where jig_data_id = jig_data.id
           order by "index"
    )                                               as "modules!: Vec<(ModuleId, StableModuleId, ModuleKind, bool)>",
   array(select row (category_id)
         from jig_data_category
         where jig_data_id = jig_data.id)     as "categories!: Vec<(CategoryId,)>",
   array(select row (affiliation_id)
         from jig_data_affiliation
         where jig_data_id = jig_data.id)     as "affiliations!: Vec<(AffiliationId,)>",
   array(select row (age_range_id)
         from jig_data_age_range
         where jig_data_id = jig_data.id)     as "age_ranges!: Vec<(AgeRangeId,)>",
   array(
            select row (jdar.id, jdar.display_name, resource_type_id, resource_content)
            from jig_data_additional_resource "jdar"
            where jdar.jig_data_id = jig_data.id
        )                                               as "additional_resource!: Vec<(AddId, String, TypeId, Value)>",
   locked                                     as "locked!",
   other_keywords                             as "other_keywords!",
   translated_keywords                        as "translated_keywords!",
   rating                                     as "rating!: Option<JigRating>",
   blocked                                    as "blocked!",
   curated                                    as "curated!",
   is_premium                                 as "premium!"
from cte1
inner join jig_data on cte1.id = jig_data.id
inner join jig on (
    jig_data.id = jig.draft_id
    or (
        jig_data.id = jig.live_id
        and last_synced_at is not null
        and jig.published_at is not null
    )
)
left join jig_admin_data "admin" on admin.jig_id = jig.id
where ord > (1 * $7 * $8)
order by ord asc
limit $8
"#,
    author_id.map(|x| x.0),
    draft_or_live.map(|it| it as i16),
    blocked,
    &privacy_level[..],
    &resource_types[..],
    order_by.map(|it| it as i32),
    page,
    page_limit as i32,
    user_id.map(|x| x.0)
)
    .fetch_all(&mut txn)
    .instrument(tracing::info_span!("query jig_data"))
    .await?;

    let v: Vec<_> = jig_data
        .into_iter()
        .map(|jig_data_row| JigResponse {
            id: jig_data_row.jig_id,
            published_at: jig_data_row.published_at,
            creator_id: jig_data_row.creator_id,
            author_id: jig_data_row.author_id,
            author_name: jig_data_row.author_name,
            likes: jig_data_row.liked_count,
            plays: jig_data_row.play_count,
            live_up_to_date: jig_data_row.live_up_to_date,
            is_liked: jig_data_row.is_liked,
            jig_data: JigData {
                created_at: jig_data_row.created_at,
                draft_or_live: jig_data_row.draft_or_live,
                display_name: jig_data_row.display_name,
                language: jig_data_row.language,
                modules: jig_data_row
                    .modules
                    .into_iter()
                    .map(|(id, stable_id, kind, is_complete)| LiteModule {
                        id,
                        stable_id,
                        kind,
                        is_complete,
                    })
                    .collect(),
                categories: jig_data_row
                    .categories
                    .into_iter()
                    .map(|(it,)| it)
                    .collect(),
                last_edited: jig_data_row.updated_at,
                description: jig_data_row.description,
                default_player_settings: JigPlayerSettings {
                    direction: jig_data_row.direction,
                    scoring: jig_data_row.scoring,
                    drag_assist: jig_data_row.drag_assist,
                },
                theme: jig_data_row.theme,
                age_ranges: jig_data_row
                    .age_ranges
                    .into_iter()
                    .map(|(it,)| it)
                    .collect(),
                affiliations: jig_data_row
                    .affiliations
                    .into_iter()
                    .map(|(it,)| it)
                    .collect(),
                additional_resources: jig_data_row
                    .additional_resource
                    .into_iter()
                    .map(|(id, display_name, resource_type_id, resource_content)| {
                        AdditionalResource {
                            id,
                            display_name,
                            resource_type_id,
                            resource_content: serde_json::from_value::<ResourceContent>(
                                resource_content,
                            )
                            .unwrap(),
                        }
                    })
                    .collect(),
                audio_background: jig_data_row.audio_background,
                audio_effects: AudioEffects {
                    feedback_positive: jig_data_row
                        .audio_feedback_positive
                        .into_iter()
                        .map(|(it,)| it)
                        .collect(),
                    feedback_negative: jig_data_row
                        .audio_feedback_negative
                        .into_iter()
                        .map(|(it,)| it)
                        .collect(),
                },
                privacy_level: jig_data_row.privacy_level,
                locked: jig_data_row.locked,
                other_keywords: jig_data_row.other_keywords,
                translated_keywords: jig_data_row.translated_keywords,
                translated_description: jig_data_row.translated_description.0,
            },
            admin_data: JigAdminData {
                rating: jig_data_row.rating,
                blocked: jig_data_row.blocked,
                curated: jig_data_row.curated,
                premium: jig_data_row.premium,
            },
        })
        .collect();

    txn.rollback().await?;

    Ok(v)
}

pub async fn update_draft(
    pool: &PgPool,
    api_key: &Option<String>,
    id: JigId,
    display_name: Option<&str>,
    categories: Option<&[CategoryId]>,
    age_ranges: Option<&[AgeRangeId]>,
    affiliations: Option<&[AffiliationId]>,
    language: Option<&str>,
    description: Option<&str>,
    default_player_settings: Option<&JigPlayerSettings>,
    theme: Option<&ThemeId>,
    audio_background: Option<&Option<AudioBackground>>,
    audio_effects: Option<&AudioEffects>,
    privacy_level: Option<PrivacyLevel>,
    other_keywords: Option<String>,
) -> Result<(), error::UpdateWithMetadata> {
    let mut txn = pool.begin().await?;

    let draft_id = sqlx::query!(
        //language=SQL
        r#"
select draft_id from jig join jig_data on jig.draft_id = jig_data.id where jig.id = $1 for update
"#,
        id.0
    )
    .fetch_optional(&mut txn)
    .await?
    .ok_or(error::UpdateWithMetadata::ResourceNotFound)?
    .draft_id;

    // update nullable fields
    if let Some(audio_background) = audio_background {
        sqlx::query!(
            //language=SQL
            r#"
update jig_data
set audio_background = $2,
updated_at = now()
where id = $1 and $2 is distinct from audio_background
            "#,
            draft_id,
            audio_background.map(|it| it as i16),
        )
        .execute(&mut txn)
        .await?;
    }

    // update collection fields, where HashSet<_> maps to an array[] column
    if let Some(audio_effects) = audio_effects {
        sqlx::query!(
            //language=SQL
            r#"
update jig_data
set audio_feedback_positive = $2,
    audio_feedback_negative = $3,
    updated_at = now()
where id = $1 and ($2 <> audio_feedback_positive or $3 <> audio_feedback_negative)
            "#,
            draft_id,
            &audio_effects
                .feedback_positive
                .iter()
                .map(|it| *it as i16)
                .collect::<Vec<_>>(),
            &audio_effects
                .feedback_negative
                .iter()
                .map(|it| *it as i16)
                .collect::<Vec<_>>(),
        )
        .execute(&mut txn)
        .await?;
    }

    if let Some(settings) = default_player_settings {
        sqlx::query!(
            //language=SQL
            r#"
update jig_data
set direction = $2,
    scoring = $3,
    drag_assist = $4,
    updated_at = now()
where id = $1 and
    (($2 is distinct from direction) or
     ($3 is distinct from scoring) or
     ($4 is distinct from drag_assist))
            "#,
            draft_id,
            settings.direction as i16,
            settings.scoring,
            settings.drag_assist,
        )
        .execute(&mut txn)
        .await?;
    }

    if let Some(privacy_level) = privacy_level {
        sqlx::query!(
            //language=SQL
            r#"
update jig_data
set privacy_level = coalesce($2, privacy_level)
where id = $1
  and $2 is distinct from privacy_level
    "#,
            draft_id,
            privacy_level as i16,
        )
        .execute(&mut txn)
        .await?;
    }

    if let Some(description) = description {
        sqlx::query!(
            r#"
update jig_data
set description = $2,
    translated_description = '{}',
    updated_at = now()
where id = $1 and $2 is distinct from description"#,
            draft_id,
            description,
        )
        .execute(&mut txn)
        .await?;
    }

    if let Some(other_keywords) = other_keywords {
        let translate_text = match &api_key {
            Some(key) => translate_text(&other_keywords, "he", "en", key)
                .await
                .context("could not translate text")?,
            None => None,
        };

        sqlx::query!(
            r#"
update jig_data
set other_keywords = $2,
    translated_keywords = (case when ($3::text is not null) then $3::text else (translated_keywords) end),
    updated_at = now()
where id = $1 and $2 is distinct from other_keywords"#,
            draft_id,
            other_keywords,
            translate_text
        )
        .execute(&mut *txn)
        .await?;
    }

    if let Some(display_name) = display_name {
        sqlx::query!(
            r#"
update jig_data
set display_name = $2,
    translated_name = '{}',
    updated_at = now()
where id = $1 and $2 is distinct from display_name"#,
            draft_id,
            display_name,
        )
        .execute(&mut txn)
        .await?;
    }

    // update trivial, not null fields
    sqlx::query!(
        //language=SQL
        r#"
update jig_data
set language         = coalesce($2, language),
    theme            = coalesce($3, theme),
    updated_at = now()
where id = $1
  and (($2::text is not null and $2 is distinct from language) or
       ($3::smallint is not null and $3 is distinct from theme))
"#,
        draft_id,
        language,
        theme.map(|it| *it as i16),
    )
    .execute(&mut txn)
    .await?;

    if let Some(categories) = categories {
        super::recycle_metadata(&mut txn, "jig_data", draft_id, categories)
            .await
            .map_err(super::meta::handle_metadata_err)?;
    }

    if let Some(affiliations) = affiliations {
        super::recycle_metadata(&mut txn, "jig_data", draft_id, affiliations)
            .await
            .map_err(super::meta::handle_metadata_err)?;
    }

    if let Some(age_ranges) = age_ranges {
        super::recycle_metadata(&mut txn, "jig_data", draft_id, age_ranges)
            .await
            .map_err(super::meta::handle_metadata_err)?;
    }

    txn.commit().await?;

    Ok(())
}

pub async fn publish_draft_to_live(db: &PgPool, jig_id: JigId) -> Result<(), error::CloneDraft> {
    let mut txn = db.begin().await?;

    let (draft_id, live_id) = get_draft_and_live_ids(&mut *txn, jig_id)
        .await
        .ok_or(error::CloneDraft::ResourceNotFound)?;

    // let draft = db::jig::get_one(&db, jig_id, DraftOrLive::Draft)
    //     .await?
    //     .ok_or(error::CloneDraft::ResourceNotFound)?; // Not strictly necessary, we already know the JIG exists.

    // let modules = draft.jig_data.modules;
    // Check that modules have been configured on the JIG
    // let has_modules = !modules.is_empty();
    // Check whether the draft's modules all have content
    // let modules_valid = modules
    //     .into_iter()
    //     .filter(|module| !module.is_complete)
    //     .collect::<Vec<LiteModule>>()
    //     .is_empty();

    // If no modules or modules without content, prevent publishing.
    // NOTE: we temporarily allow publishing jig without content
    // since curation also uses this endpoint and some jigs have already been published without content
    // and those jigs have to be curated
    // if !modules_valid || !has_modules {
    //     return Err(error::CloneDraft::IncompleteModules);
    // }

    let new_live_id = clone_data(&mut txn, &draft_id, DraftOrLive::Live, |stable_id| {
        stable_id
    })
    .await?;

    sqlx::query!(
        //language=SQL
        r#"
        update user_asset_data
        set jig_count = jig_count + 1,
        total_asset_count = total_asset_count + 1
        from jig
        where author_id = user_id and
              published_at is null and
              id = $1"#,
        jig_id.0
    )
    .execute(&mut *txn)
    .await?;

    sqlx::query!(
        //language=SQL
        "update jig set live_id = $1, published_at = now() where id = $2",
        new_live_id,
        jig_id.0
    )
    .execute(&mut *txn)
    .await?;

    // should drop all the entries in the metadata tables that FK to the live jig_data row
    sqlx::query!(
        //language=SQL
        r#"
delete from jig_data where id = $1
    "#,
        live_id,
    )
    .execute(&mut *txn)
    .await?;

    log::info!("AOSIJDOAIJSD");

    txn.commit().await?;

    Ok(())
}

pub async fn delete(pool: &PgPool, id: JigId) -> Result<(), error::Delete> {
    let mut txn = pool.begin().await?;

    let (draft_id, live_id) = get_draft_and_live_ids(&mut txn, id)
        .await
        .ok_or(error::Delete::ResourceNotFound)?;

    sqlx::query!(
        //language=SQL
        r#"
    update user_asset_data 
    set jig_count = jig_count - 1,
        total_asset_count = total_asset_count - 1
    from jig
    where author_id = user_id and
          published_at is not null and 
          id = $1"#,
        id.0
    )
    .execute(&mut *txn)
    .await?;

    sqlx::query!(
        //language=SQL
        r#"
with del_data as (
    delete from jig_data
        where id is not distinct from $1 or id is not distinct from $2)
delete
from jig
where id is not distinct from $3

"#,
        draft_id,
        live_id,
        id.0,
    )
    .execute(&mut txn)
    .await?;

    txn.commit().await?;
    Ok(())
}

// `None` here means do not filter.
#[instrument(skip(db))]
pub async fn filtered_count(
    db: &PgPool,
    privacy_level: Vec<PrivacyLevel>,
    blocked: Option<bool>,
    author_id: Option<UserId>,
    draft_or_live: Option<DraftOrLive>,
    resource_types: Vec<Uuid>,
) -> sqlx::Result<(u64, u64)> {
    let privacy_level: Vec<i16> = privacy_level.iter().map(|x| *x as i16).collect();

    let jig_data = sqlx::query!(
        //language=SQL
        r#"
        with cte as (
            select array_agg(jd.id)
            from jig_data "jd"
                  inner join jig on (draft_id = jd.id or (live_id = jd.id and jd.last_synced_at is not null and published_at is not null))
                  left join jig_admin_data "admin" on admin.jig_id = jig.id
                  left join jig_data_additional_resource "resource" on jd.id = resource.jig_data_id
            where (jd.draft_or_live = $1 or $1 is null)
                and (author_id = $2 or $2 is null)
                and (blocked = $3 or $3 is null)
                and (jd.privacy_level = any($4) or $4 = array[]::smallint[])
                and (resource.resource_type_id = any($5) or $5 = array[]::uuid[])
            group by updated_at, created_at, jig.published_at, admin.jig_id, jig_id
        )
            select count(*) as "count!" from unnest(array((select cte.array_agg[1] from cte))) with ordinality t(id
           , ord)
        "#,
        draft_or_live.map(|it| it as i16),
        author_id.map(|it| it.0),
        blocked,
        &privacy_level[..],
        &resource_types[..]
    )
    .fetch_one(db)
    .instrument(tracing::info_span!("count jig_data"))
    .await?;

    let jig = sqlx::query!(
        //language=SQL
        r#"
        with cte as (
            select (array_agg(jig.id))[1]
            from jig
                  inner join jig_data jd on (draft_id = jd.id or (live_id = jd.id and jd.last_synced_at is not null and published_at is not null))
                  left join jig_admin_data "admin" on admin.jig_id = jig.id
                  left join jig_data_additional_resource "resource" on jd.id = resource.jig_data_id
            where (jd.draft_or_live = $1 or $1 is null)
              and (author_id = $2 or $2 is null)
              and (blocked = $3 or $3 is null)
              and (jd.privacy_level = any($4) or $4 = array[]::smallint[])
              and (resource.resource_type_id = any($5) or $5 = array[]::uuid[])
            group by updated_at, created_at, jig.published_at, admin.jig_id, jig_id
        )
            select count(*) as "count!" from unnest(array((select cte.array_agg from cte))) with ordinality t(id
           , ord)
        "#,
        draft_or_live.map(|it| it as i16),
        author_id.map(|it| it.0),
        blocked,
        &privacy_level[..],
        &resource_types[..]
    )
    .fetch_one(db)
    .instrument(tracing::info_span!("count jig"))
    .await?;

    Ok((jig.count as u64, jig_data.count as u64))
}

pub async fn count(db: &PgPool, privacy_level: PrivacyLevel) -> sqlx::Result<u64> {
    sqlx::query!(
        //language=SQL
        r#"
select count(*) as "count!: i64"
from jig_data
inner join jig on jig.live_id = jig_data.id
where (privacy_level = coalesce($1, privacy_level))
"#,
        privacy_level as i16,
    )
    .fetch_one(db)
    .await
    .map(|it| it.count as u64)
}

pub async fn get_draft_and_live_ids(txn: &mut PgConnection, jig_id: JigId) -> Option<(Uuid, Uuid)> {
    sqlx::query!(
        //language=SQL
        r#"
select draft_id, live_id from jig where id = $1
"#,
        jig_id.0
    )
    .fetch_optional(&mut *txn)
    .await
    .ok()?
    .map(|it| (it.draft_id, it.live_id))
}

/// Clones a copy of the jig data and modules
async fn clone_data(
    txn: &mut PgConnection,
    from_data_id: &Uuid,
    draft_or_live: DraftOrLive,
    mut get_stable_module_id: impl FnMut(StableModuleId) -> StableModuleId,
) -> Result<Uuid, error::CloneDraft> {
    let new_id = sqlx::query!(
        //language=SQL
        r#"
insert into jig_data
(display_name, created_at, updated_at, language, last_synced_at, description, theme, audio_background,
 audio_feedback_negative, audio_feedback_positive, direction, scoring, drag_assist, privacy_level, other_keywords, translated_keywords, translated_description)
select display_name,
       created_at,
       updated_at,
       language,
       last_synced_at,
       description,
       theme,
       audio_background,
       audio_feedback_negative,
       audio_feedback_positive,
       direction,
       scoring,
       drag_assist,
       privacy_level,
       other_keywords,
       translated_keywords,
       translated_description::jsonb
from jig_data
where id = $1
returning id
        "#,
        from_data_id,
    )
    .fetch_one(&mut *txn)
    .await?
    .id;

    update_draft_or_live(txn, new_id, draft_or_live).await?;

    let modules = sqlx::query!(
        //language=SQL
        r#"
            select stable_id, "index", kind, is_complete, contents
            from jig_data_module
            where jig_data_id = $1
        "#,
        from_data_id,
    )
    .fetch_all(&mut *txn)
    .await?;

    for module in modules {
        let new_stable_id = get_stable_module_id(StableModuleId(module.stable_id));
        sqlx::query!(
            //language=SQL
            r#"
                insert into jig_data_module
                (stable_id, index, jig_data_id, kind, is_complete, contents)
                values
                ($1, $2, $3, $4, $5, $6)
            "#,
            new_stable_id.0,
            module.index,
            new_id,
            module.kind,
            module.is_complete,
            module.contents,
        )
        .execute(&mut *txn)
        .await?;
    }

    sqlx::query!(
        //language=SQL
        r#"
insert into jig_data_additional_resource(jig_data_id, resource_type_id, display_name, resource_content)
select $2, resource_type_id, display_name, resource_content
from jig_data_additional_resource
where jig_data_id = $1
        "#,
        from_data_id,
        new_id,
    )
    .execute(&mut *txn)
    .await?;

    sqlx::query!(
        //language=SQL
        r#"
insert into jig_data_affiliation(jig_data_id, affiliation_id)
select $2, affiliation_id
from jig_data_affiliation
where jig_data_id = $1
        "#,
        from_data_id,
        new_id,
    )
    .execute(&mut *txn)
    .await?;

    sqlx::query!(
        //language=SQL
        r#"
insert into jig_data_age_range(jig_data_id, age_range_id)
select $2, age_range_id
from jig_data_age_range
where jig_data_id = $1
        "#,
        from_data_id,
        new_id,
    )
    .execute(&mut *txn)
    .await?;

    sqlx::query!(
        //language=SQL
        r#"
insert into jig_data_category(jig_data_id, category_id)
select $2, category_id
from jig_data_category
where jig_data_id = $1
        "#,
        from_data_id,
        new_id,
    )
    .execute(&mut *txn)
    .await?;

    // copy modules

    Ok(new_id)
}

pub async fn clone_jig(
    db: &PgPool,
    parent: JigId,
    user_id: UserId,
) -> Result<JigId, error::CloneDraft> {
    let mut txn = db.begin().await?;

    let (draft_id, live_id) = get_draft_and_live_ids(&mut *txn, parent)
        .await
        .ok_or(error::CloneDraft::ResourceNotFound)?;

    let mut stable_id_map = HashMap::new();

    let new_draft_id = clone_data(&mut txn, &draft_id, DraftOrLive::Draft, |old_stable_id| {
        let new_stable_id: StableModuleId = StableModuleId(Uuid::new_v4());
        stable_id_map.insert(old_stable_id, new_stable_id);
        new_stable_id
    })
    .await?;

    let new_live_id =
        clone_data(
            &mut txn,
            &live_id,
            DraftOrLive::Live,
            |old_stable_id| match stable_id_map.get(&old_stable_id) {
                Some(old_stable_id) => *old_stable_id,
                None => StableModuleId(Uuid::new_v4()),
            },
        )
        .await?;

    let new_jig = sqlx::query!(
        //language=SQL
        r#"
insert into jig (creator_id, author_id, parents, live_id, draft_id)
select creator_id, $2, array_append(parents, $1), $3, $4
from jig
where id = $1
returning id as "id!: JigId"
"#,
        parent.0,
        user_id.0,
        new_live_id,
        new_draft_id,
    )
    .fetch_one(&mut txn)
    .await?;

    sqlx::query!(
        // language=SQL
        r#"
insert into jig_play_count (jig_id, play_count)
values ($1, 0)
        "#,
        new_jig.id.0
    )
    .execute(&mut txn)
    .await?;

    txn.commit().await?;

    Ok(new_jig.id)
}

pub async fn jig_play(db: &PgPool, id: JigId) -> anyhow::Result<()> {
    let mut txn = db.begin().await?;

    let jig = sqlx::query!(
        // language=SQL
        r#"
select published_at  as "published_at?"
from jig
where id = $1
    "#,
        id.0
    )
    .fetch_one(&mut txn)
    .await?;

    //check if jig has been published and playable
    if jig.published_at == None {
        return Err(anyhow::anyhow!("Jig has not been published"));
    };

    //update Jig play count
    sqlx::query!(
        // language=SQL
        r#"
update jig_play_count
set play_count = play_count + 1
where jig_id = $1;
            "#,
        id.0,
    )
    .execute(db)
    .await?;

    txn.commit().await?;

    Ok(())
}

pub async fn update_admin_data(
    pool: &PgPool,
    jig_id: JigId,
    admin_data: JigUpdateAdminDataRequest,
) -> Result<(), error::NotFound> {
    let mut txn = pool.begin().await?;

    let blocked = admin_data.blocked.into_option();

    sqlx::query!(
        // language=SQL
        r#"
update jig_admin_data
set
    rating = coalesce($2, rating),
    blocked = coalesce($3, blocked),
    curated = coalesce($4, curated),
    is_premium = coalesce($5, is_premium)
where jig_id = $1
"#,
        jig_id.0,
        admin_data.rating.into_option() as Option<JigRating>,
        blocked,
        admin_data.curated.into_option(),
        admin_data.premium.into_option(),
    )
    .execute(&mut txn)
    .await?;

    if blocked.is_some() {
        sqlx::query!(
            //language=SQL
            r#"
update jig_data
set updated_at = now()
from jig
where jig.live_id = $1
            "#,
            jig_id.0,
        )
        .execute(&mut txn)
        .await?;
    }

    txn.commit().await?;

    Ok(())
}

pub async fn transfer_jigs(
    pool: &PgPool,
    from: UserId,
    to: UserId,
    jig_ids: &[JigId],
) -> anyhow::Result<()> {
    let mut txn = pool.begin().await?;

    let from_exists = sqlx::query!(
        r#"
select exists (select 1 from "user" where id = $1) as "check_from!"
        "#,
        from.0
    )
    .fetch_one(&mut txn)
    .await?
    .check_from;

    if !from_exists {
        return Err(anyhow::anyhow!("Former creator ID does not exist"));
    };

    let to_exists = sqlx::query!(
        r#"
select exists (select 1 from "user" where id = $1) as "check_to!"
        "#,
        to.0
    )
    .fetch_one(&mut txn)
    .await?
    .check_to;

    if !to_exists {
        return Err(anyhow::anyhow!("Target creator ID does not exist"));
    };

    let mut ids = Vec::new();

    for id in jig_ids {
        let id = id.0;

        ids.push(id)
    }

    let ids = sqlx::query!(
        r#"
update jig
set author_id = $1,
    creator_id = $1
where (creator_id = $2 or author_id = $2)
and id = any($3)
returning live_id
        "#,
        to.0,
        from.0,
        &ids[..]
    )
    .fetch_all(&mut txn)
    .await?;

    sqlx::query!(
        r#"
with new_data as (
    select count(*) as jig_count, author_id from jig where published_at IS NOT NULL and author_id = $1 or author_id = $2 GROUP BY author_id
)
update user_asset_data
    set jig_count = new_data.jig_count,
        total_asset_count = new_data.jig_count + playlist_count + resource_count
from new_data
where user_asset_data.user_id = new_data.author_id;
        "#,
        to.0,
        from.0
    )
    .execute(&mut txn)
    .await?;

    let live_ids: Vec<_> = ids.into_iter().map(|record| record.live_id).collect();

    if live_ids.is_empty() {
        return Err(anyhow::anyhow!("JIG ids not owned by old creator ID"));
    }

    sqlx::query!(
        r#"
    update jig_data
    set last_synced_at = null
    where id = any($1)
            "#,
        &live_ids[..]
    )
    .execute(&mut txn)
    .await?;

    txn.commit().await?;

    Ok(())
}

pub async fn jig_like(db: &PgPool, user_id: UserId, id: JigId) -> anyhow::Result<()> {
    let mut txn = db.begin().await?;

    let jig = sqlx::query!(
        r#"
select author_id    "author_id: UserId",
       published_at  as "published_at?"
from jig
where id = $1
    "#,
        id.0
    )
    .fetch_one(&mut txn)
    .await?;

    //check if Jig is published and likeable
    if jig.published_at == None {
        return Err(anyhow::anyhow!("Jig has not been published"));
    };

    // check if current user is the author
    if jig.author_id == Some(user_id) {
        return Err(anyhow::anyhow!("Cannot like your own jig"));
    };

    // checks if user has already liked the jig
    sqlx::query!(
        // language=SQL
        r#"
insert into jig_like(jig_id, user_id)
values ($1, $2)
            "#,
        id.0,
        user_id.0
    )
    .execute(&mut txn)
    .await
    .map_err(|_| anyhow::anyhow!("Cannot like a jig more than once"))?;

    txn.commit().await?;

    Ok(())
}

pub async fn jig_unlike(db: &PgPool, user_id: UserId, id: JigId) -> anyhow::Result<()> {
    sqlx::query!(
        r#"
delete from jig_like
where jig_id = $1 and user_id = $2
    "#,
        id.0,
        user_id.0
    )
    .execute(db)
    .await
    .map_err(|_| anyhow::anyhow!("Must like jig prior to unlike"))?;

    Ok(())
}

pub async fn get_jig_resources(db: &PgPool) -> Result<Vec<JigId>, error::Delete> {
    let jig_ids = sqlx::query!(
        //language=SQL
        r#"
select id as "id: JigId"
from jig
where jig_focus = 1
limit 150
"#,
    )
    .fetch_all(db)
    .await?
    .into_iter()
    .map(|it| it.id)
    .collect::<Vec<JigId>>();

    println!("inside");

    Ok(jig_ids)
}

pub async fn jig_is_liked(db: &PgPool, user_id: UserId, id: JigId) -> sqlx::Result<bool> {
    let exists = sqlx::query!(
        r#"
select exists (
    select 1
    from jig_like
    where
        jig_id = $1
        and user_id = $2
) as "exists!"
    "#,
        id.0,
        user_id.0
    )
    .fetch_one(db)
    .await?
    .exists;

    Ok(exists)
}

pub async fn list_liked(
    db: &PgPool,
    user_id: UserId,
    page: u32,
    page_limit: u32,
) -> sqlx::Result<Vec<JigId>> {
    let rows = sqlx::query!(
        r#"
        select jig_id
        from jig_like
        where user_id = $1
        order by created_at desc
        offset $2
        limit $3
        
    "#,
        user_id.0,
        (page * page_limit) as i32,
        page_limit as i32,
    )
    .fetch_all(db)
    .await?;

    Ok(rows.into_iter().map(|row| JigId(row.jig_id)).collect())
}

#[instrument(skip(db))]
pub async fn get_jig_playlists(
    db: &sqlx::Pool<sqlx::Postgres>,
    jig_id: JigId,
    user_id: Option<UserId>,
) -> sqlx::Result<Vec<PlaylistResponse>> {
    let mut txn = db.begin().await?;

    let playlist_data = sqlx::query!(
    //language=SQL
    r#"
select playlist.id                                                                as "playlist_id: PlaylistId",
    privacy_level                                                               as "privacy_level: PrivacyLevel",
    creator_id                                                                  as "creator_id?: UserId",
    author_id                                                                   as "author_id?: UserId",
    (select given_name || ' '::text || family_name
     from user_profile
     where user_profile.user_id = author_id)                                     as "author_name",
    published_at,
    likes,
    plays,
    live_up_to_date,
    exists(select 1 from playlist_like where playlist_id = playlist.id and user_id = $2)    as "is_liked!",
    display_name                                                                  as "display_name!",
    updated_at,
    language                                                                      as "language!",
    description                                                                   as "description!",
    translated_description                                                        as "translated_description!: Json<HashMap<String,String>>",
    draft_or_live                                                                 as "draft_or_live!: DraftOrLive",
    other_keywords                                                                as "other_keywords!",
    translated_keywords                                                           as "translated_keywords!",
    rating                                     as "rating!: Option<PlaylistRating>",
    blocked                                    as "blocked!",
    curated                                    as "curated!",
    is_premium                                 as "premium!",
    (
        select row(playlist_data_module.id, playlist_data_module.stable_id, kind, is_complete)
        from playlist_data_module
        where playlist_data_id = playlist_data.id and "index" = 0
        order by "index"
    )                                                   as "cover?: (ModuleId, StableModuleId, ModuleKind, bool)",
    array(select row (category_id)
            from playlist_data_category
            where playlist_data_id = playlist_data.id)     as "categories!: Vec<(CategoryId,)>",
    array(select row (affiliation_id)
            from playlist_data_affiliation
            where playlist_data_id = playlist_data.id)          as "affiliations!: Vec<(AffiliationId,)>",
    array(select row (age_range_id)
            from playlist_data_age_range
            where playlist_data_id = playlist_data.id)          as "age_ranges!: Vec<(AgeRangeId,)>",
    array(select row (id, display_name, resource_type_id, resource_content)
                from playlist_data_resource
                where playlist_data_id = playlist_data.id
          )                                          as "additional_resource!: Vec<(AddId, String, TypeId, Value)>",
    array(
        select row(jig_id)
        from playlist_data_jig
        where playlist_data_jig.playlist_data_id = playlist_data.id
        order by "index"
    )                                                     as "items!: Vec<(JigId,)>"
from playlist_data_jig "pdj"
inner join playlist_data on pdj.playlist_data_id = playlist_data.id
inner join playlist on
        playlist_data.id = playlist.live_id
        and last_synced_at is not null
        and playlist.published_at is not null
left join playlist_admin_data "admin" on admin.playlist_id = playlist.id
where jig_id = $1
order by coalesce(updated_at, created_at) desc
"#,
    jig_id.0,
    user_id.map(|x| x.0)
)
    .fetch_all(&mut txn)
    .instrument(tracing::info_span!("query playlist_data"))
    .await?;

    let v: Vec<_> = playlist_data
        .into_iter()
        .map(|playlist_data_row| PlaylistResponse {
            id: playlist_data_row.playlist_id,
            published_at: playlist_data_row.published_at,
            creator_id: playlist_data_row.creator_id,
            author_id: playlist_data_row.author_id,
            author_name: playlist_data_row.author_name,
            likes: playlist_data_row.likes,
            plays: playlist_data_row.plays,
            live_up_to_date: playlist_data_row.live_up_to_date,
            is_liked: playlist_data_row.is_liked,
            playlist_data: PlaylistData {
                draft_or_live: playlist_data_row.draft_or_live,
                display_name: playlist_data_row.display_name,
                language: playlist_data_row.language,
                cover: playlist_data_row
                    .cover
                    .map(|(id, stable_id, kind, is_complete)| LiteModule {
                        id,
                        stable_id,
                        kind,
                        is_complete,
                    }),
                categories: playlist_data_row
                    .categories
                    .into_iter()
                    .map(|(it,)| it)
                    .collect(),
                last_edited: playlist_data_row.updated_at,
                description: playlist_data_row.description,
                age_ranges: playlist_data_row
                    .age_ranges
                    .into_iter()
                    .map(|(it,)| it)
                    .collect(),
                affiliations: playlist_data_row
                    .affiliations
                    .into_iter()
                    .map(|(it,)| it)
                    .collect(),
                additional_resources: playlist_data_row
                    .additional_resource
                    .into_iter()
                    .map(|(id, display_name, resource_type_id, resource_content)| {
                        AdditionalResource {
                            id,
                            display_name,
                            resource_type_id,
                            resource_content: serde_json::from_value::<ResourceContent>(
                                resource_content,
                            )
                            .unwrap(),
                        }
                    })
                    .collect(),
                privacy_level: playlist_data_row.privacy_level,
                other_keywords: playlist_data_row.other_keywords,
                translated_keywords: playlist_data_row.translated_keywords,
                translated_description: playlist_data_row.translated_description.0,
                items: playlist_data_row
                    .items
                    .into_iter()
                    .map(|(it,)| it)
                    .collect(),
            },
            admin_data: PlaylistAdminData {
                rating: playlist_data_row.rating,
                blocked: playlist_data_row.blocked,
                curated: playlist_data_row.curated,
                premium: playlist_data_row.premium,
            },
        })
        .collect();

    txn.rollback().await?;

    Ok(v)
}

pub async fn is_logged_in(db: &PgPool, user_id: UserId) -> Result<(), error::Auth> {
    sqlx::query!(
        r#"
select exists(select 1 from user_scope where user_id = $1) as "authed!"
"#,
        user_id.0,
    )
    .fetch_one(db)
    .await?
    .authed;

    Ok(())
}

pub async fn authz(db: &PgPool, user_id: UserId, jig_id: Option<JigId>) -> Result<(), error::Auth> {
    let authed = match jig_id {
        None => {
            sqlx::query!(
                r#"
select exists(select 1 from user_scope where user_id = $1 and scope = any($2)) as "authed!"
"#,
                user_id.0,
                &[
                    UserScope::Admin as i16,
                    UserScope::AdminAsset as i16,
                    UserScope::ManageSelfAsset as i16,
                ][..],
            )
            .fetch_one(db)
            .await?
            .authed
        }
        Some(id) => {
            sqlx::query!(
                //language=SQL
                r#"
select exists (
    select 1 from user_scope where user_id = $1 and scope = any($2)
) or (
    exists (select 1 from user_scope where user_id = $1 and scope = $3) and
    not exists (select 1 from jig where jig.id = $4 and jig.author_id <> $1)
) as "authed!"
"#,
                user_id.0,
                &[UserScope::Admin as i16, UserScope::AdminAsset as i16,][..],
                UserScope::ManageSelfAsset as i16,
                id.0
            )
            .fetch_one(db)
            .await?
            .authed
        }
    };

    if !authed {
        return Err(error::Auth::Forbidden);
    }

    Ok(())
}

pub async fn is_users_code(db: &PgPool, user_id: UserId, code: JigCode) -> Result<(), error::Auth> {
    let authed = sqlx::query!(
        //language=SQL
        r#"
            select exists (
                select 1 from jig_code where creator_id = $1 and code = $2
            ) as "authed!"
            
        "#,
        user_id.0,
        code.0
    )
    .fetch_one(db)
    .await?
    .authed;

    if !authed {
        return Err(error::Auth::Forbidden);
    }

    Ok(())
}

async fn update_draft_or_live(
    conn: &mut PgConnection,
    jig_data_id: Uuid,
    draft_or_live: DraftOrLive,
) -> sqlx::Result<()> {
    sqlx::query!(
        //language=SQL
        r#"
update jig_data
set draft_or_live = $2
where id = $1
            "#,
        jig_data_id,
        draft_or_live as i16
    )
    .execute(&mut *conn)
    .await?;

    Ok(())
}

pub async fn jigs_export(db: &sqlx::PgPool) -> anyhow::Result<Vec<AdminJigExport>> {
    let rows = sqlx::query!(
        //language=SQL
        r#"
        with cte as (
            select array_agg(jd.id)
            from jig_data "jd"
                  inner join jig on (draft_id = jd.id or (live_id = jd.id and jd.last_synced_at is not null and published_at is not null))
                  left join jig_admin_data "admin" on admin.jig_id = jig.id
                  left join jig_data_additional_resource "resource" on jd.id = resource.jig_data_id
            where (jd.draft_or_live = $1)
            group by updated_at, created_at, jig.published_at, admin.jig_id
        ),
        cte1 as (
            select * from unnest(array((select cte.array_agg[1] from cte))) with ordinality t(id
           , ord) order by ord
        )
        select jig.id                                           as "jig_id: JigId",
            privacy_level                                       as "privacy_level: PrivacyLevel",
            creator_id                                          as "creator_id?: UserId",
            author_id                                           as "author_id?: UserId",
            (select given_name || ' '::text || family_name
                from user_profile
             where user_profile.user_id = author_id)            as "author_name",
            created_at,
            published_at,
            liked_count,
            exists(select 1 from jig_like where jig_id = jig.id) as "is_liked!",
            (
                 select play_count
                 from jig_play_count
                 where jig_play_count.jig_id = jig.id
            )                                                   as "play_count!",
    
            display_name                                        as "display_name!",
            language                                            as "language!",
            description                                         as "description!",
            rating                                              as "rating!: Option<JigRating>",
            blocked                                             as "blocked!",
            curated                                             as "curated!",
            is_premium                                          as "premium!"
        from cte1
        inner join jig_data on cte1.id = jig_data.id
        inner join jig on (
            jig_data.id = jig.draft_id
            or (
                jig_data.id = jig.live_id
                and last_synced_at is not null
                and jig.published_at is not null
            )
        )
        left join jig_admin_data "admin" on admin.jig_id = jig.id
        "#,
        DraftOrLive::Live as i16
    )
        .fetch_all(db)
        .instrument(tracing::info_span!("query jig_data for export"))
        .await?;

    Ok(rows
        .into_iter()
        .map(|row| AdminJigExport {
            id: row.jig_id,
            description: row.description,
            display_name: row.display_name,
            premium: row.premium,
            blocked: row.blocked,
            author_id: row.author_id,
            author_name: row.author_name,
            likes: row.liked_count,
            plays: row.play_count,
            rating: row.rating,
            privacy_level: row.privacy_level,
            created_at: row.created_at,
            published_at: row.published_at,
            language: row.language,
        })
        .collect())
}
