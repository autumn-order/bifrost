use eve_esi::model::oauth2::AuthenticationData;

use crate::server::error::Error;

pub fn login_service(
    esi_client: &eve_esi::Client,
    scopes: Vec<String>,
) -> Result<AuthenticationData, Error> {
    let login = esi_client.oauth2().login_url(scopes)?;

    Ok(login)
}
