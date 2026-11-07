//! `contacts add|list|remove`: local labels, never sent anywhere.

use std::fmt::Write as _;

use serde_json::{Value, json};

use crate::{
    ContactsCmd,
    context::Ctx,
    error::{Error, Result},
    refs,
    types::Contact,
};

fn render(contacts: &[Contact]) -> String {
    if contacts.is_empty() {
        return "No contacts. Add one: clave contacts add <address> <label>\n".to_owned();
    }
    let width = contacts.iter().map(|c| c.label.len()).max().unwrap_or(0);
    contacts.iter().fold(String::new(), |mut out, c| {
        let _ = writeln!(out, "{:<width$}  {}", c.label, c.address);
        out
    })
}

pub(crate) fn run(ctx: &Ctx, cmd: ContactsCmd) -> Result<()> {
    let mut client = ctx.client()?;
    let raw: Value = match cmd {
        ContactsCmd::List => client.call("contacts.list", Value::Null)?,
        ContactsCmd::Add { address, label } => {
            if !refs::is_address(&address) {
                return Err(Error::usage(format!(
                    "\"{address}\" is not an address (0x and 64 hex digits)"
                )));
            }
            client.call(
                "contacts.put",
                json!({ "address": address, "label": label.to_ascii_lowercase() }),
            )?
        }
        ContactsCmd::Remove { reference } => {
            let contacts: Vec<Contact> = client.call("contacts.list", Value::Null)?;
            let contact = refs::resolve_contact(&contacts, &reference)?;
            client.call("contacts.remove", json!({ "address": contact.address }))?
        }
    };
    let contacts: Vec<Contact> = serde_json::from_value(raw.clone()).unwrap_or_default();
    ctx.emit(&raw, || render(&contacts));
    Ok(())
}
