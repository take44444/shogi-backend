use async_trait::async_trait;
use openidconnect::{
    AccessTokenHash, AuthorizationCode, ClientId, ClientSecret, CsrfToken, EndpointMaybeSet,
    EndpointNotSet, EndpointSet, IssuerUrl, Nonce, OAuth2TokenResponse, PkceCodeChallenge,
    PkceCodeVerifier, RedirectUrl, Scope, TokenResponse,
    core::{CoreAuthenticationFlow, CoreClient, CoreProviderMetadata},
    reqwest,
};
use std::time::Duration;
use user_application::{DisplayName, Error, LoginAttempt, VerifiedIdentity};

pub const GOOGLE_ISSUER: &str = "https://accounts.google.com";
pub struct Authorization {
    pub url: String,
    pub state: String,
    pub nonce: String,
    pub pkce_verifier: String,
}
#[async_trait]
pub trait IdentityProvider: Send + Sync {
    fn authorize(&self) -> Authorization;
    async fn exchange(
        &self,
        code: String,
        attempt: LoginAttempt,
    ) -> Result<VerifiedIdentity, Error>;
}
type DiscoveredClient = CoreClient<
    EndpointSet,
    EndpointNotSet,
    EndpointNotSet,
    EndpointNotSet,
    EndpointMaybeSet,
    EndpointMaybeSet,
>;
pub struct GoogleProvider {
    client: DiscoveredClient,
    http: reqwest::Client,
    client_id: String,
    client_secret: String,
    callback: String,
}
impl GoogleProvider {
    pub async fn discover(
        client_id: String,
        client_secret: String,
        callback: String,
    ) -> Result<Self, Error> {
        let http = reqwest::Client::builder()
            .redirect(reqwest::redirect::Policy::none())
            .https_only(true)
            .connect_timeout(Duration::from_secs(5))
            .timeout(Duration::from_secs(10))
            .build()
            .map_err(|_| Error::Unavailable)?;
        let metadata = CoreProviderMetadata::discover_async(
            IssuerUrl::new(GOOGLE_ISSUER.to_owned()).map_err(|_| Error::InvalidInput)?,
            &http,
        )
        .await
        .map_err(|_| Error::Unavailable)?;
        if metadata.authorization_endpoint().url().scheme() != "https"
            || metadata
                .token_endpoint()
                .is_none_or(|u| u.url().scheme() != "https")
        {
            return Err(Error::Unavailable);
        }
        let client = CoreClient::from_provider_metadata(
            metadata,
            ClientId::new(client_id.clone()),
            Some(ClientSecret::new(client_secret.clone())),
        )
        .set_redirect_uri(RedirectUrl::new(callback.clone()).map_err(|_| Error::InvalidInput)?);
        Ok(Self {
            client,
            http,
            client_id,
            client_secret,
            callback,
        })
    }
}
#[async_trait]
impl IdentityProvider for GoogleProvider {
    fn authorize(&self) -> Authorization {
        let (challenge, verifier) = PkceCodeChallenge::new_random_sha256();
        let (url, state, nonce) = self
            .client
            .authorize_url(
                CoreAuthenticationFlow::AuthorizationCode,
                CsrfToken::new_random,
                Nonce::new_random,
            )
            .add_scope(Scope::new("profile".into()))
            .set_pkce_challenge(challenge)
            .url();
        Authorization {
            url: url.to_string(),
            state: state.secret().clone(),
            nonce: nonce.secret().clone(),
            pkce_verifier: verifier.secret().clone(),
        }
    }
    async fn exchange(
        &self,
        code: String,
        attempt: LoginAttempt,
    ) -> Result<VerifiedIdentity, Error> {
        let metadata = CoreProviderMetadata::discover_async(
            IssuerUrl::new(GOOGLE_ISSUER.to_owned()).map_err(|_| Error::InvalidInput)?,
            &self.http,
        )
        .await
        .map_err(|_| Error::Unavailable)?;
        let client = CoreClient::from_provider_metadata(
            metadata,
            ClientId::new(self.client_id.clone()),
            Some(ClientSecret::new(self.client_secret.clone())),
        )
        .set_redirect_uri(
            RedirectUrl::new(self.callback.clone()).map_err(|_| Error::InvalidInput)?,
        );
        exchange_code(&client, &self.http, &self.client_id, code, attempt).await
    }
}

async fn exchange_code(
    client: &DiscoveredClient,
    http: &reqwest::Client,
    client_id: &str,
    code: String,
    attempt: LoginAttempt,
) -> Result<VerifiedIdentity, Error> {
    let response = client
        .exchange_code(AuthorizationCode::new(code))
        .map_err(|_| Error::Unavailable)?
        .set_pkce_verifier(PkceCodeVerifier::new(attempt.pkce_verifier))
        .request_async(http)
        .await
        .map_err(|_| Error::Unauthorized)?;
    verify_response(client, client_id, response, attempt.nonce)
}

fn verify_response(
    client: &DiscoveredClient,
    client_id: &str,
    response: openidconnect::core::CoreTokenResponse,
    nonce: String,
) -> Result<VerifiedIdentity, Error> {
    let token = response.id_token().ok_or(Error::Unauthorized)?;
    let verifier = client.id_token_verifier();
    let nonce = Nonce::new(nonce);
    let claims = token
        .claims(&verifier, &nonce)
        .map_err(|_| Error::Unauthorized)?;
    if claims
        .authorized_party()
        .is_some_and(|azp| azp.as_str() != client_id)
        || (claims.audiences().len() > 1 && claims.authorized_party().is_none())
    {
        return Err(Error::Unauthorized);
    }
    if let Some(expected) = claims.access_token_hash() {
        let actual = AccessTokenHash::from_token(
            response.access_token(),
            token.signing_alg().map_err(|_| Error::Unauthorized)?,
            token
                .signing_key(&verifier)
                .map_err(|_| Error::Unauthorized)?,
        )
        .map_err(|_| Error::Unauthorized)?;
        if actual != *expected {
            return Err(Error::Unauthorized);
        }
    }
    if claims.subject().as_str().is_empty() {
        return Err(Error::Unauthorized);
    }
    let name = claims
        .name()
        .and_then(|names| names.get(None))
        .map(|s| s.as_str())
        .unwrap_or("Player");
    Ok(VerifiedIdentity {
        issuer: GOOGLE_ISSUER.to_owned(),
        subject: claims.subject().as_str().to_owned(),
        initial_name: DisplayName::new(name.to_owned())
            .unwrap_or(DisplayName::new("Player".into()).expect("valid static display name")),
    })
}

#[cfg(test)]
mod tests;
