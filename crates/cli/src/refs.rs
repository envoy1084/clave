//! References: `vaults:ops` / `tokens:ttt` / `contacts:bea` aliases, bare names in the argument's namespace, raw
//! `0x` addresses, and git-style proposal id prefixes.

use crate::{
    error::{Error, Kind, Result},
    types::{Contact, TokenInfo, VaultSummary},
};

/// A raw Aztec address: `0x` and 64 hex digits.
pub(crate) fn is_address(input: &str) -> bool {
    input
        .strip_prefix("0x")
        .is_some_and(|hex| hex.len() == 64 && hex.bytes().all(|b| b.is_ascii_hexdigit()))
}

/// Splits `namespace:name`, checking the namespace is the one the argument expects.
fn in_namespace<'a>(input: &'a str, namespace: &str) -> Result<&'a str> {
    match input.split_once(':') {
        Some((ns, name)) if ns == namespace => Ok(name),
        Some((ns, _)) => Err(Error::usage(format!(
            "\"{input}\": expected a {namespace}: reference, not {ns}:"
        ))),
        None => Ok(input),
    }
}

/// The vault `reference` names, or the profile's default (or only) vault when it is `None`.
pub(crate) fn resolve_vault<'a>(
    vaults: &'a [VaultSummary],
    reference: Option<&str>,
) -> Result<&'a VaultSummary> {
    if vaults.is_empty() {
        return Err(
            Error::new(Kind::NotReady, "This profile has no vaults yet.")
                .hint("Found one: tecpan lobby open, or join one: tecpan lobby join <invite>"),
        );
    }
    let Some(reference) = reference else {
        if let Some(vault) = vaults.iter().find(|v| v.is_default) {
            return Ok(vault);
        }
        if let [only] = vaults {
            return Ok(only);
        }
        return Err(Error::usage("This profile has several vaults; pick one.")
            .hint("tecpan -V <alias|address> …, or select a default: tecpan vault use <ref>"));
    };
    let name = in_namespace(reference, "vaults")?;
    if name.starts_with("0x") {
        let lower = name.to_ascii_lowercase();
        let matches: Vec<&VaultSummary> = vaults
            .iter()
            .filter(|v| v.address.to_ascii_lowercase().starts_with(&lower))
            .collect();
        return one(reference, "vault", &matches, |v| short_hex(&v.address));
    }
    let matches: Vec<&VaultSummary> = vaults
        .iter()
        .filter(|v| {
            v.alias
                .as_deref()
                .is_some_and(|a| a.eq_ignore_ascii_case(name))
                || v.name.eq_ignore_ascii_case(name)
        })
        .collect();
    one(reference, "vault", &matches, |v| short_hex(&v.address))
}

/// A tracked token by symbol (case-insensitive) or address.
pub(crate) fn resolve_token<'a>(tokens: &'a [TokenInfo], reference: &str) -> Result<&'a TokenInfo> {
    let name = in_namespace(reference, "tokens")?;
    let matches: Vec<&TokenInfo> = if is_address(name) {
        tokens
            .iter()
            .filter(|t| t.address.eq_ignore_ascii_case(name))
            .collect()
    } else {
        tokens
            .iter()
            .filter(|t| t.symbol.eq_ignore_ascii_case(name))
            .collect()
    };
    one(reference, "token", &matches, |t| short_hex(&t.address)).map_err(|e| {
        if e.message.starts_with("Unknown") {
            e.hint("Track it first: tecpan token add <address>, or list them: tecpan token list")
        } else {
            e
        }
    })
}

/// A recipient: a raw address, a contact label, or a vault member's name.
pub(crate) fn resolve_recipient(
    reference: &str,
    contacts: &[Contact],
    members: &[(String, String)],
) -> Result<String> {
    if is_address(reference) {
        return Ok(reference.to_owned());
    }
    let name = in_namespace(reference, "contacts")?;
    let contact: Vec<&Contact> = contacts
        .iter()
        .filter(|c| c.label.eq_ignore_ascii_case(name))
        .collect();
    if !contact.is_empty() {
        return one(reference, "contact", &contact, |c| short_hex(&c.address))
            .map(|c| c.address.clone());
    }
    if !reference.contains(':') {
        let member: Vec<&(String, String)> = members
            .iter()
            .filter(|(n, _)| n.eq_ignore_ascii_case(name))
            .collect();
        if !member.is_empty() {
            return one(reference, "member", &member, |(_, a)| short_hex(a))
                .map(|(_, a)| a.clone());
        }
    }
    Err(Error::usage(format!("Unknown recipient \"{reference}\"."))
        .hint("Use a 0x address, or add a label first: tecpan contacts add <address> <label>"))
}

/// A contact by label or address.
pub(crate) fn resolve_contact<'a>(contacts: &'a [Contact], reference: &str) -> Result<&'a Contact> {
    let name = in_namespace(reference, "contacts")?;
    let matches: Vec<&Contact> = contacts
        .iter()
        .filter(|c| c.label.eq_ignore_ascii_case(name) || c.address.eq_ignore_ascii_case(name))
        .collect();
    one(reference, "contact", &matches, |c| c.label.clone())
}

/// The proposal id `prefix` uniquely names (with or without `0x`); an exact id always wins.
pub(crate) fn resolve_proposal<'a>(ids: &[&'a str], prefix: &str) -> Result<&'a str> {
    let want = strip_hex(prefix);
    if want.is_empty() || !want.bytes().all(|b| b.is_ascii_hexdigit()) {
        return Err(Error::usage(format!("\"{prefix}\" is not a proposal id")));
    }
    if let Some(exact) = ids.iter().find(|id| strip_hex(id) == want) {
        return Ok(exact);
    }
    let matches: Vec<&'a str> = ids
        .iter()
        .copied()
        .filter(|id| strip_hex(id).starts_with(&want))
        .collect();
    match matches.as_slice() {
        [] => Err(Error::usage(format!("Unknown proposal \"{prefix}\"."))
            .hint("List them: tecpan proposals --status all")),
        [one] => Ok(one),
        many => Err(Error::usage(format!(
            "Ambiguous proposal \"{prefix}\": {}. Use more characters.",
            many.iter()
                .map(|id| short_id(id))
                .collect::<Vec<_>>()
                .join(", ")
        ))),
    }
}

/// A lobby by id prefix, or the only open lobby when `prefix` is `None`.
pub(crate) fn resolve_lobby<'a>(ids: &[&'a str], prefix: Option<&str>) -> Result<&'a str> {
    match prefix {
        Some(prefix) => resolve_proposal(ids, prefix).map_err(|e| Error {
            message: e.message.replace("proposal", "lobby"),
            hint: Some("List them: tecpan lobby list".into()),
            ..e
        }),
        None => match ids {
            [only] => Ok(only),
            [] => Err(Error::usage("No open lobby.")
                .hint("Open one: tecpan lobby open --name <n> -k <k> --tv <t_v>")),
            _ => Err(Error::usage("Several lobbies are open; name one.").hint("tecpan lobby list")),
        },
    }
}

fn one<'a, T>(
    reference: &str,
    what: &str,
    matches: &[&'a T],
    label: impl Fn(&T) -> String,
) -> Result<&'a T> {
    match matches {
        [] => Err(Error::usage(format!("Unknown {what} \"{reference}\"."))),
        [one] => Ok(one),
        many => Err(Error::usage(format!(
            "Ambiguous {what} \"{reference}\": {}.",
            many.iter().map(|m| label(m)).collect::<Vec<_>>().join(", ")
        ))),
    }
}

fn strip_hex(id: &str) -> String {
    id.strip_prefix("0x").unwrap_or(id).to_ascii_lowercase()
}

/// The first six hex digits of an id, as shown in tables (`7f3a1c`).
pub(crate) fn short_id(id: &str) -> String {
    strip_hex(id).chars().take(6).collect()
}

/// `0x2874…5e69`.
pub(crate) fn short_hex(value: &str) -> String {
    let body = value.strip_prefix("0x").unwrap_or(value);
    if body.len() <= 10 {
        return value.to_owned();
    }
    format!("0x{}…{}", &body[..4], &body[body.len() - 4..])
}

#[cfg(test)]
mod tests {
    use super::*;

    fn address(fill: char) -> String {
        format!("0x{}", fill.to_string().repeat(64))
    }

    fn vault(name: &str, alias: Option<&str>, fill: char, is_default: bool) -> VaultSummary {
        VaultSummary {
            address: address(fill),
            name: name.into(),
            alias: alias.map(Into::into),
            is_default,
            ..VaultSummary::default()
        }
    }

    #[test]
    fn vaults_by_alias_name_address_and_default() {
        let vaults = [
            vault("Ops", Some("ops"), 'a', false),
            vault("Treasury", None, 'b', true),
        ];
        assert_eq!(
            resolve_vault(&vaults, Some("vaults:ops")).unwrap().name,
            "Ops"
        );
        assert_eq!(
            resolve_vault(&vaults, Some("treasury")).unwrap().name,
            "Treasury"
        );
        assert_eq!(resolve_vault(&vaults, Some("0xaaaa")).unwrap().name, "Ops");
        assert_eq!(resolve_vault(&vaults, None).unwrap().name, "Treasury");
        assert_eq!(
            resolve_vault(&vaults, Some("tokens:ops"))
                .err()
                .unwrap()
                .exit_code(),
            2
        );
        assert_eq!(
            resolve_vault(&vaults, Some("nope"))
                .err()
                .unwrap()
                .exit_code(),
            2
        );
        let no_default = [vault("A", None, 'a', false), vault("B", None, 'b', false)];
        assert_eq!(
            resolve_vault(&no_default, None).err().unwrap().exit_code(),
            2
        );
        assert_eq!(resolve_vault(&no_default[..1], None).unwrap().name, "A");
        assert_eq!(resolve_vault(&[], None).err().unwrap().exit_code(), 4);
    }

    #[test]
    fn tokens_by_symbol_case_insensitive() {
        let tokens = [TokenInfo {
            address: address('c'),
            symbol: "TTT".into(),
            decimals: 18,
            ..TokenInfo::default()
        }];
        assert_eq!(resolve_token(&tokens, "ttt").unwrap().decimals, 18);
        assert_eq!(resolve_token(&tokens, "tokens:TTT").unwrap().decimals, 18);
        assert!(resolve_token(&tokens, &address('c')).is_ok());
        let unknown = resolve_token(&tokens, "usdc").err().unwrap();
        assert_eq!(unknown.exit_code(), 2);
        assert!(unknown.hint.is_some());
    }

    #[test]
    fn recipients() {
        let contacts = [Contact {
            address: address('d'),
            label: "bea".into(),
        }];
        let members = [("cai".to_owned(), address('e'))];
        assert_eq!(
            resolve_recipient("bea", &contacts, &members).unwrap(),
            address('d')
        );
        assert_eq!(
            resolve_recipient("contacts:bea", &contacts, &members).unwrap(),
            address('d')
        );
        assert_eq!(
            resolve_recipient("Cai", &contacts, &members).unwrap(),
            address('e')
        );
        assert_eq!(
            resolve_recipient(&address('f'), &contacts, &members).unwrap(),
            address('f')
        );
        assert!(resolve_recipient("contacts:cai", &contacts, &members).is_err());
        assert_eq!(
            resolve_recipient("dan", &contacts, &members)
                .err()
                .unwrap()
                .exit_code(),
            2
        );
    }

    #[test]
    fn proposal_prefixes() {
        let ids = ["0x7f3a1c00", "0x7f90d200", "0xc09e4400"];
        assert_eq!(resolve_proposal(&ids, "7f3a").unwrap(), "0x7f3a1c00");
        assert_eq!(resolve_proposal(&ids, "0xC09E").unwrap(), "0xc09e4400");
        let ambiguous = resolve_proposal(&ids, "7f").err().unwrap();
        assert_eq!(ambiguous.exit_code(), 2);
        assert_eq!(
            ambiguous.message,
            "Ambiguous proposal \"7f\": 7f3a1c, 7f90d2. Use more characters."
        );
        assert_eq!(resolve_proposal(&ids, "aa").err().unwrap().exit_code(), 2);
        assert_eq!(resolve_proposal(&ids, "zz").err().unwrap().exit_code(), 2);
        // An exact id wins over longer ids it prefixes.
        assert_eq!(resolve_proposal(&["0xab", "0xabcd"], "ab").unwrap(), "0xab");
    }

    #[test]
    fn lobbies() {
        assert_eq!(resolve_lobby(&["0x01ab"], None).unwrap(), "0x01ab");
        assert_eq!(resolve_lobby(&[], None).err().unwrap().exit_code(), 2);
        let ambiguous = resolve_lobby(&["0x01ab", "0x01cd"], Some("01"))
            .err()
            .unwrap();
        assert!(ambiguous.message.starts_with("Ambiguous lobby"));
    }

    #[test]
    fn short_forms() {
        assert_eq!(short_hex(&address('a')), "0xaaaa…aaaa");
        assert_eq!(short_hex("0x12"), "0x12");
        assert_eq!(short_id("0x7F3A1C9d"), "7f3a1c");
    }
}
