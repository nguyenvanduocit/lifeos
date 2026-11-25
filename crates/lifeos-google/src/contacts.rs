//! Google Contacts (the People API): the owner's contacts.

use anyhow::{Context, Result, ensure};
use schemars::JsonSchema;
use serde::Deserialize;
use serde_json::{Map, Value, json};

use crate::Google;
use crate::api::count;

const BASE: &str = "https://people.googleapis.com/v1/";
const FIELDS: &str = "names,emailAddresses,phoneNumbers,organizations,birthdays,biographies,addresses";

#[derive(Debug, Default, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct ContactsSearch {
    /// A name, email, phone number or organization, or the start of one.
    pub query: String,
    /// 1–30; 10 when not given.
    pub max_results: Option<u32>,
}

#[derive(Debug, Default, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct ContactsList {
    /// 1–1000; 100 when not given.
    pub max_results: Option<u32>,
    pub page_token: Option<String>,
}

/// A contact's fields, once for creating and once with the contact to change.
macro_rules! contact_fields {
    ($(#[$m:meta])* pub struct $name:ident { $($head:tt)* }) => {
        $(#[$m])*
        #[derive(Debug, Default, Deserialize, JsonSchema)]
        #[serde(deny_unknown_fields)]
        pub struct $name {
            $($head)*
            /// The full name.
            pub name: Option<String>,
            pub emails: Option<Vec<String>>,
            pub phones: Option<Vec<String>>,
            pub organization: Option<String>,
            /// Job title at the organization.
            pub title: Option<String>,
            /// YYYY-MM-DD, or --MM-DD without a year.
            pub birthday: Option<String>,
            pub address: Option<String>,
            pub notes: Option<String>,
        }
    };
}

contact_fields! { pub struct ContactFields {} }

contact_fields! {
    pub struct ContactsUpdate {
        /// The contact's resource name, e.g. people/c123.
        pub resource_name: String,
    }
}

impl ContactsUpdate {
    fn fields(self) -> (String, ContactFields) {
        let ContactsUpdate { resource_name, name, emails, phones, organization, title, birthday, address, notes } = self;
        (resource_name, ContactFields { name, emails, phones, organization, title, birthday, address, notes })
    }
}

#[derive(Debug, Default, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct ContactName {
    /// The contact's resource name, e.g. people/c123.
    pub resource_name: String,
}

fn resource(name: &str) -> Result<&str> {
    ensure!(
        name.strip_prefix("people/").is_some_and(|id| !id.is_empty() && id.bytes().all(|b| b.is_ascii_alphanumeric())),
        "a contact's resource name looks like people/c123"
    );
    Ok(name)
}

fn values<'a>(person: &'a Value, list: &str, key: &str) -> Vec<&'a str> {
    person
        .get(list)
        .and_then(Value::as_array)
        .into_iter()
        .flatten()
        .filter_map(|v| v.get(key)?.as_str())
        .collect()
}

fn person(p: &Value) -> Value {
    let birthday = p.pointer("/birthdays/0/date").map(|d| {
        let part = |k: &str| d.get(k).and_then(Value::as_u64);
        match (part("year"), part("month"), part("day")) {
            (Some(y), Some(m), Some(d)) => format!("{y:04}-{m:02}-{d:02}"),
            (None, Some(m), Some(d)) => format!("--{m:02}-{d:02}"),
            _ => String::new(),
        }
    });
    json!({
        "resource_name": p.get("resourceName"),
        "name": p.pointer("/names/0/displayName"),
        "emails": values(p, "emailAddresses", "value"),
        "phones": values(p, "phoneNumbers", "value"),
        "organization": p.pointer("/organizations/0/name"),
        "title": p.pointer("/organizations/0/title"),
        "birthday": birthday.filter(|b| !b.is_empty()),
        "address": p.pointer("/addresses/0/formattedValue"),
        "notes": p.pointer("/biographies/0/value"),
    })
}

/// The People API body for the fields given, and the field mask naming them.
fn body(f: &ContactFields) -> Result<(Map<String, Value>, Vec<&'static str>)> {
    let mut body = Map::new();
    let mut mask = Vec::new();
    if let Some(name) = &f.name {
        body.insert("names".into(), json!([{ "unstructuredName": name.trim() }]));
        mask.push("names");
    }
    if let Some(emails) = &f.emails {
        body.insert("emailAddresses".into(), json!(emails.iter().map(|e| json!({ "value": e.trim() })).collect::<Vec<_>>()));
        mask.push("emailAddresses");
    }
    if let Some(phones) = &f.phones {
        body.insert("phoneNumbers".into(), json!(phones.iter().map(|p| json!({ "value": p.trim() })).collect::<Vec<_>>()));
        mask.push("phoneNumbers");
    }
    if f.organization.is_some() || f.title.is_some() {
        body.insert("organizations".into(), json!([{ "name": f.organization, "title": f.title }]));
        mask.push("organizations");
    }
    if let Some(b) = &f.birthday {
        body.insert("birthdays".into(), json!([{ "date": birthday(b)? }]));
        mask.push("birthdays");
    }
    if let Some(a) = &f.address {
        body.insert("addresses".into(), json!([{ "formattedValue": a.trim() }]));
        mask.push("addresses");
    }
    if let Some(n) = &f.notes {
        body.insert("biographies".into(), json!([{ "value": n, "contentType": "TEXT_PLAIN" }]));
        mask.push("biographies");
    }
    Ok((body, mask))
}

fn birthday(text: &str) -> Result<Value> {
    let text = text.trim();
    let parts: Vec<&str> = text.trim_start_matches("--").split('-').collect();
    let number = |s: &str| s.parse::<u32>().with_context(|| format!("{text:?} is not YYYY-MM-DD or --MM-DD"));
    Ok(match parts.as_slice() {
        [y, m, d] => json!({ "year": number(y)?, "month": number(m)?, "day": number(d)? }),
        [m, d] if text.starts_with("--") => json!({ "month": number(m)?, "day": number(d)? }),
        _ => anyhow::bail!("{text:?} is not YYYY-MM-DD or --MM-DD"),
    })
}

pub(crate) async fn contacts_search(g: &Google, i: ContactsSearch) -> Result<Value> {
    ensure!(!i.query.trim().is_empty(), "say what to look for");
    // Google asks for an empty search first, to warm the contacts cache it searches.
    g.json(|http| http.get(format!("{BASE}people:searchContacts")).query(&[("query", ""), ("readMask", "names")]))
        .await?;
    let size = count(i.max_results, 10, 30).to_string();
    let found = g
        .json(|http| {
            http.get(format!("{BASE}people:searchContacts")).query(&[
                ("query", i.query.as_str()),
                ("readMask", FIELDS),
                ("pageSize", size.as_str()),
            ])
        })
        .await?;
    Ok(json!(
        found
            .get("results")
            .and_then(Value::as_array)
            .into_iter()
            .flatten()
            .filter_map(|r| r.get("person"))
            .map(person)
            .collect::<Vec<_>>()
    ))
}

pub(crate) async fn contacts_list(g: &Google, i: ContactsList) -> Result<Value> {
    let size = count(i.max_results, 100, 1000).to_string();
    let list = g
        .json(|http| {
            let mut q = vec![
                ("personFields", FIELDS),
                ("pageSize", size.as_str()),
                ("sortOrder", "LAST_MODIFIED_DESCENDING"),
            ];
            if let Some(page) = &i.page_token {
                q.push(("pageToken", page.as_str()));
            }
            http.get(format!("{BASE}people/me/connections")).query(&q)
        })
        .await?;
    Ok(json!({
        "contacts": list.get("connections").and_then(Value::as_array).into_iter().flatten().map(person).collect::<Vec<_>>(),
        "next_page_token": list.get("nextPageToken"),
        "total": list.get("totalPeople"),
    }))
}

pub(crate) async fn contacts_create(g: &Google, i: ContactFields) -> Result<Value> {
    let (body, mask) = body(&i)?;
    ensure!(mask.contains(&"names") || mask.contains(&"emailAddresses"), "give the contact a name or an email");
    let body = Value::Object(body);
    let created = g
        .json(|http| http.post(format!("{BASE}people:createContact")).query(&[("personFields", FIELDS)]).json(&body))
        .await?;
    Ok(person(&created))
}

pub(crate) async fn contacts_update(g: &Google, i: ContactsUpdate) -> Result<Value> {
    let (name, fields) = i.fields();
    let name = resource(&name)?;
    let (mut body, mask) = body(&fields)?;
    ensure!(!mask.is_empty(), "say what to change");
    let current = g.json(|http| http.get(format!("{BASE}{name}")).query(&[("personFields", "metadata")])).await?;
    body.insert("etag".into(), current.get("etag").cloned().context("Google gave the contact no etag")?);
    let (body, mask) = (Value::Object(body), mask.join(","));
    let changed = g
        .json(|http| {
            http.patch(format!("{BASE}{name}:updateContact"))
                .query(&[("updatePersonFields", mask.as_str()), ("personFields", FIELDS)])
                .json(&body)
        })
        .await?;
    Ok(person(&changed))
}

pub(crate) async fn contacts_delete(g: &Google, i: ContactName) -> Result<Value> {
    let name = resource(&i.resource_name)?;
    g.json(|http| http.delete(format!("{BASE}{name}:deleteContact"))).await?;
    Ok(json!({ "deleted": name }))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn people_api_shapes_round_into_plain_contacts() {
        let p = json!({
            "resourceName": "people/c1",
            "names": [{"displayName": "Anna Le"}],
            "emailAddresses": [{"value": "anna@example.com"}],
            "birthdays": [{"date": {"month": 3, "day": 9}}],
        });
        let c = person(&p);
        assert_eq!(c["name"], "Anna Le");
        assert_eq!(c["emails"][0], "anna@example.com");
        assert_eq!(c["birthday"], "--03-09");
        assert_eq!(birthday("1990-03-09").unwrap(), json!({"year": 1990, "month": 3, "day": 9}));
        assert_eq!(birthday("--03-09").unwrap(), json!({"month": 3, "day": 9}));
        assert!(birthday("March 9").is_err());
        assert!(resource("people/c1").is_ok());
        assert!(resource("people/../x").is_err());
    }
}
