//! Bring the records at one name to what a product wants, touching only the
//! records it says are its own.
//!
//! [`Owned::Ids`] is for a product sharing zones with their owners (Swarm):
//! records whose ids it did not record are never changed or deleted, even at
//! the managed name. [`Owned::All`] treats every record of the reconciled
//! type at the name as the product's (the honeypot, whose names exist only
//! because it made them).

use std::{collections::BTreeSet, net::IpAddr};

use crate::{Changed, Client, NewRecord, Record, Result, Token};

/// Which records at the name the caller owns.
#[derive(Debug, Clone, Copy)]
pub enum Owned<'a> {
    /// Every record of the reconciled type at the name.
    All,
    /// Only records with these ids.
    Ids(&'a [String]),
}

impl Owned<'_> {
    fn covers(&self, record: &Record) -> bool {
        match self {
            Self::All => true,
            Self::Ids(ids) => ids.contains(&record.id),
        }
    }
}

/// The owned records at the name once reconciled, and whether any write
/// happened.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Outcome {
    pub records: Vec<Record>,
    pub changed: Changed,
}

/// One A/AAAA record per wanted address at `host`, each `proxied` as asked.
///
/// A matching owned record is kept. A missing address gets a record before
/// any stale one is deleted, so the name keeps answering throughout. The one
/// exception is a stale record with the same address but other proxying:
/// Cloudflare refuses an identical record, so that one goes first. Owned
/// records for addresses no longer wanted are deleted last; an empty
/// `wanted` deletes them all.
pub async fn addresses(
    client: &Client,
    token: &Token,
    zone_id: &str,
    host: &str,
    wanted: &[IpAddr],
    proxied: bool,
    owned: Owned<'_>,
) -> Result<Outcome> {
    let there: Vec<Record> = client
        .records(token, zone_id, host)
        .await?
        .into_iter()
        .filter(|record| record.is_address() && owned.covers(record))
        .collect();
    let wanted: BTreeSet<IpAddr> = wanted.iter().copied().collect();
    let mut kept: Vec<(IpAddr, Record)> = Vec::new();
    let mut stale = Vec::new();
    for record in there {
        match record.content.parse::<IpAddr>() {
            Ok(ip)
                if wanted.contains(&ip)
                    && record.proxied == proxied
                    && !kept.iter().any(|(have, _)| *have == ip) =>
            {
                kept.push((ip, record));
            }
            _ => stale.push(record),
        }
    }
    let mut changed = false;
    for ip in &wanted {
        if kept.iter().any(|(have, _)| have == ip) {
            continue;
        }
        if let Some(at) = stale
            .iter()
            .position(|record| record.content.parse::<IpAddr>().ok() == Some(*ip))
        {
            let old = stale.remove(at);
            client.delete_record(token, zone_id, &old.id).await?;
        }
        let made = client
            .create_record(token, zone_id, &NewRecord::address(host, *ip, proxied))
            .await?;
        kept.push((*ip, made));
        changed = true;
    }
    for record in stale {
        client.delete_record(token, zone_id, &record.id).await?;
        changed = true;
    }
    kept.sort_by_key(|(ip, _)| *ip);
    Ok(Outcome {
        records: kept.into_iter().map(|(_, record)| record).collect(),
        changed,
    })
}

/// One CNAME at `host` pointing at `target`, `proxied` as asked.
///
/// A matching owned CNAME is kept; otherwise the first owned CNAME is
/// changed in place (a CNAME cannot sit beside another record, so it is
/// never created next to the old one), or one is created. Any further owned
/// CNAMEs at the name are deleted.
pub async fn cname(
    client: &Client,
    token: &Token,
    zone_id: &str,
    host: &str,
    target: &str,
    proxied: bool,
    owned: Owned<'_>,
) -> Result<Outcome> {
    let wanted = NewRecord::cname(host, target, proxied);
    let mut there: Vec<Record> = client
        .records(token, zone_id, host)
        .await?
        .into_iter()
        .filter(|record| record.kind == "CNAME" && owned.covers(record))
        .collect();
    let matches = |record: &Record| {
        record
            .content
            .trim_end_matches('.')
            .eq_ignore_ascii_case(&wanted.content)
            && record.proxied == proxied
    };
    let mut changed = false;
    let keep = match there.iter().position(matches) {
        Some(at) => there.remove(at),
        None if !there.is_empty() => {
            let old = there.remove(0);
            changed = true;
            client
                .update_record(token, zone_id, &old.id, &wanted)
                .await?
        }
        None => {
            changed = true;
            client.create_record(token, zone_id, &wanted).await?
        }
    };
    for extra in there {
        client.delete_record(token, zone_id, &extra.id).await?;
        changed = true;
    }
    Ok(Outcome {
        records: vec![keep],
        changed,
    })
}
