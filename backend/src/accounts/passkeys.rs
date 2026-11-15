//! WebAuthn configuration and the option tweaks Tessera needs.

use webauthn_rs::prelude::{
    CreationChallengeResponse, CredentialID, DiscoverableAuthentication, PasskeyRegistration,
    RequestChallengeResponse, Uuid, Webauthn, WebauthnBuilder, WebauthnError,
};
use webauthn_rs_proto::ResidentKeyRequirement;

use crate::config::Config;

pub fn build_webauthn(config: &Config) -> Result<Webauthn, WebauthnError> {
    WebauthnBuilder::new(&config.webauthn_rp_id, &config.public_url)?
        .rp_name(&config.webauthn_rp_name)
        .build()
}

/// Starts a passkey registration that requires a discoverable credential, so
/// sign-in works with a single tap and no username field.
pub fn start_registration(
    webauthn: &Webauthn,
    user_id: Uuid,
    display_name: &str,
    exclude: Vec<CredentialID>,
) -> Result<(CreationChallengeResponse, PasskeyRegistration), WebauthnError> {
    let exclude = (!exclude.is_empty()).then_some(exclude);
    let (mut options, state) =
        webauthn.start_passkey_registration(user_id, display_name, display_name, exclude)?;
    if let Some(selection) = options.public_key.authenticator_selection.as_mut() {
        selection.resident_key = Some(ResidentKeyRequirement::Required);
        selection.require_resident_key = true;
    }
    Ok((options, state))
}

/// Starts a username-less sign-in. webauthn-rs defaults to autofill
/// ("conditional") mediation; Tessera uses an explicit button instead.
pub fn start_login(
    webauthn: &Webauthn,
) -> Result<(RequestChallengeResponse, DiscoverableAuthentication), WebauthnError> {
    let (mut options, state) = webauthn.start_discoverable_authentication()?;
    options.mediation = None;
    Ok((options, state))
}

#[cfg(test)]
mod tests {
    use super::*;

    fn webauthn() -> Webauthn {
        let config = Config::from_lookup(|key| {
            (key == "TESSERA_PUBLIC_URL").then(|| "https://tessera.example.com".to_owned())
        })
        .unwrap();
        build_webauthn(&config).unwrap()
    }

    #[test]
    fn registration_requires_resident_key_and_user_verification() {
        let (options, _) =
            start_registration(&webauthn(), Uuid::new_v4(), "Alice", vec![]).unwrap();
        let json = serde_json::to_value(&options).unwrap();
        let selection = &json["publicKey"]["authenticatorSelection"];

        assert_eq!(selection["residentKey"], "required");
        assert_eq!(selection["requireResidentKey"], true);
        assert_eq!(selection["userVerification"], "required");
        assert_eq!(json["publicKey"]["rp"]["id"], "tessera.example.com");
        assert_eq!(json["publicKey"]["user"]["displayName"], "Alice");
    }

    #[test]
    fn registration_excludes_existing_credentials() {
        let (options, _) =
            start_registration(&webauthn(), Uuid::new_v4(), "Alice", vec![vec![1, 2, 3]]).unwrap();
        let json = serde_json::to_value(&options).unwrap();
        assert_eq!(
            json["publicKey"]["excludeCredentials"]
                .as_array()
                .map(Vec::len),
            Some(1)
        );
    }

    #[test]
    fn login_uses_modal_mediation_and_no_allow_list() {
        let (options, _) = start_login(&webauthn()).unwrap();
        let json = serde_json::to_value(&options).unwrap();

        assert!(json.get("mediation").is_none_or(serde_json::Value::is_null));
        let allow = &json["publicKey"]["allowCredentials"];
        assert!(allow.is_null() || allow.as_array().is_some_and(Vec::is_empty));
        assert_eq!(json["publicKey"]["userVerification"], "required");
    }
}
