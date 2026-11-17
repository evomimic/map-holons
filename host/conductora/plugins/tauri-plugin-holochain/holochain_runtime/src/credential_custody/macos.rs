//! Explicit data-protection Keychain access; never use the legacy default keychain.
use super::{denied, PassphraseStore};
use core_foundation::{base::TCFType, data::CFData, dictionary::CFDictionary, string::CFString};
use security_framework::{
    access_control::{ProtectionMode, SecAccessControl},
    os::macos::code_signing::{Flags, SecCode, SecRequirement},
    passwords::{generic_password, PasswordOptions},
};
use security_framework_sys::{
    base::errSecItemNotFound,
    item::{kSecUseAuthenticationUI, kSecValueData},
    keychain_item::SecItemAdd,
};
use zeroize::Zeroizing;

// Public Security.framework symbol not exposed by the locked Rust bindings.
extern "C" {
    static kSecUseAuthenticationUIFail: core_foundation::string::CFStringRef;
}

const BUNDLE_ID: &str = "com.map-holons.tauri.dev";
const SERVICE: &str = "map-holons.native-conductor";
const ACCOUNT: &str = "primary-installation-passphrase-v1";

pub(super) struct AppKeychain {
    group: &'static str,
}

fn admission_requirement(group: &str) -> crate::Result<SecRequirement> {
    let prefix = group
        .strip_suffix(BUNDLE_ID)
        .ok_or_else(|| denied("Custody access group does not match the packaged application"))?;
    if prefix.len() < 2
        || !prefix.ends_with('.')
        || !group.bytes().all(|b| b.is_ascii_alphanumeric() || b == b'.' || b == b'-')
    {
        return Err(denied("Invalid compiled custody access group"));
    }
    format!("anchor apple generic and identifier \"{BUNDLE_ID}\" and entitlement[\"com.apple.application-identifier\"] = \"{group}\" and ! entitlement[\"com.apple.security.get-task-allow\"] exists")
        .parse().map_err(|_| denied("Invalid packaged custody signing requirement"))
}

impl AppKeychain {
    pub(super) fn for_current_process() -> crate::Result<Self> {
        let group = option_env!("MAP_KEYCHAIN_ACCESS_GROUP")
            .ok_or_else(|| denied("Protected package has no compiled Keychain access group"))?;
        let requirement = admission_requirement(group)?;
        SecCode::for_self(Flags::NONE)
            .and_then(|code| code.check_validity(Flags::STRICT_VALIDATE, &requirement))
            .map_err(|_| denied("Protected custody requires the approved signed/provisioned package without debugger entitlement"))?;
        Ok(Self { group })
    }

    #[allow(deprecated)] // Isolated binding gap: PasswordOptions exposes query attributes.
    fn options(&self) -> crate::Result<PasswordOptions> {
        let mut options = PasswordOptions::new_generic_password(SERVICE, ACCOUNT);
        options.use_protected_keychain();
        options.set_access_group(self.group);
        options.set_access_synchronized(Some(false));
        // No new authentication/confirmation prompt is selected by this provider.
        // A locked Keychain or inaccessible group is an error, never a fallback.
        unsafe {
            options.query.push((
                CFString::wrap_under_get_rule(kSecUseAuthenticationUI),
                CFString::wrap_under_get_rule(kSecUseAuthenticationUIFail).into_CFType(),
            ));
        }
        Ok(options)
    }
}

impl PassphraseStore for AppKeychain {
    fn read(&self) -> crate::Result<Option<Zeroizing<Vec<u8>>>> {
        match generic_password(self.options()?) {
            Ok(secret) => Ok(Some(Zeroizing::new(secret))),
            Err(error) if error.code() == errSecItemNotFound => Ok(None),
            Err(error) => {
                Err(denied(format!("Protected credential unavailable (OSStatus {})", error.code())))
            }
        }
    }

    #[allow(deprecated)]
    fn insert(&self, secret: &[u8]) -> crate::Result<()> {
        let mut options = self.options()?;
        options.set_access_control(
            SecAccessControl::create_with_protection(
                Some(ProtectionMode::AccessibleWhenUnlockedThisDeviceOnly),
                0,
            )
            .map_err(|_| denied("Creating device-local custody access control"))?,
        );
        // Do not use set_generic_password_options: it updates duplicate items,
        // which could replace the credential protecting an existing installation.
        unsafe {
            options.query.push((
                CFString::wrap_under_get_rule(kSecValueData),
                CFData::from_buffer(secret).into_CFType(),
            ));
        }
        let attributes = CFDictionary::from_CFType_pairs(&options.query);
        let status = unsafe { SecItemAdd(attributes.as_concrete_TypeRef(), std::ptr::null_mut()) };
        if status != 0 {
            return Err(denied(format!(
                "Protected credential creation refused (OSStatus {status})"
            )));
        }
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn foreign_or_injected_groups_are_rejected_before_keychain_access() {
        for group in [
            "",
            "TEAM.other.application",
            "TEAM.\"com.map-holons.tauri.dev",
            "com.map-holons.tauri.dev",
        ] {
            assert!(admission_requirement(group).is_err());
        }
        let requirement = admission_requirement("EXAMPLETEAM.com.map-holons.tauri.dev").unwrap();
        assert!(SecCode::for_self(Flags::NONE)
            .and_then(|code| code.check_validity(Flags::NONE, &requirement))
            .is_err());
    }
}
