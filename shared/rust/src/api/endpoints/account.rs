use super::ApiEndpoint;
use crate::domain::billing::{
    CreateSchoolAccountRequest, GetSchoolAccountPath, GetSchoolAccountResponse, SchoolAccountPath,
    SchoolId, SchoolName, SchoolNamePath,
};
use crate::{api::Method, error::EmptyError};

/// Return a list of known school names
pub struct GetSchoolNames;
impl ApiEndpoint for GetSchoolNames {
    type Path = SchoolNamePath;
    type Req = ();
    type Res = Vec<SchoolName>;
    type Err = EmptyError;
    const METHOD: Method = Method::Get;
}

/// Create a new school account
pub struct CreateSchoolAccount;
impl ApiEndpoint for CreateSchoolAccount {
    type Path = SchoolAccountPath;
    type Req = CreateSchoolAccountRequest;
    type Res = SchoolId;
    type Err = EmptyError;
    const METHOD: Method = Method::Post;
}

/// Get a school account
pub struct GetSchoolAccount;
impl ApiEndpoint for GetSchoolAccount {
    type Path = GetSchoolAccountPath;
    type Req = ();
    type Res = GetSchoolAccountResponse;
    type Err = EmptyError;
    const METHOD: Method = Method::Get;
}

/// Update a school account
pub struct UpdateSchoolAccount;
