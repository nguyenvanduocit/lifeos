//! How memories are ranked for what an agent looks for (remit's `crew::recall`, ADR-0117): pure. Retrieval casts wide
//! and a judge decides:
//!
//! 1. **Queries.** A search is its words. A turn is two: the message, and the agent's work in one line (its role).
//! 2. **Candidates.** Each query's words (FTS5's rank) and meaning (the cosine of its vector with each memory's) give an
//!    order, and links one step over the graph from the best of them. Reciprocal rank fusion joins the orders, so no
//!    signal's scale outweighs another's.
//! 3. **Relevance.** A cross-encoder reads the query beside each memory and says how relevant it is, from 0 to 1. It
//!    reads every memory the agent may recall while they are few (`JUDGE_ALL`), else the best `JUDGED` candidates. A turn
//!    is given only what clears `TURN_FLOOR`; a search shows what clears `SEARCH_FLOOR`. With no judge a turn is given
//!    only what words and meaning agree on, its meaning at `MEANING_FLOOR` or nearer, or what links to such a one; a
//!    search shows the fused order.
//! 4. **Weight.** The owner's confirmation and the memory's strength (what agents said of it, fading with disuse) weigh
//!    the score.
//!
//! lifeos holds no subjects (remit's files, crates and tools): a life's memories link to each other, not to code. With
//! no model key at all, lifeos gives a turn what words alone find (`words_only`), as it did before meaning: remit gives
//! nothing then, which would leave an owner with no key no memory at all.

use std::collections::{HashMap, HashSet};

use chrono::{Local, NaiveDateTime};
use lifeos_wire::Memory;

/// The constant of reciprocal rank fusion.
const RRF_K: f64 = 60.0;

/// How many of the meaning's nearest memories are candidates, per query.
pub const MEANING_TOP: usize = 24;

/// How many fused candidates the judge reads when the memories an agent may recall are more than `JUDGE_ALL`.
pub const JUDGED: usize = 32;
pub const JUDGE_ALL: usize = 256;

/// How much of a strong hit a linked memory takes.
const LINKED_SHARE: f64 = 0.5;

/// How many of the fused hits the graph is walked from.
const WALK_FROM: usize = 8;

/// How much more a memory the owner confirmed weighs.
const CONFIRMED_WEIGHT: f64 = 1.25;

/// The models and floors remit measured together (ADR-0117): the reranker gave every query about nothing 0.000 and every
/// best answer 0.843 or more; with no judge, the cosine under the embedding model, the query instructed, of 0.55 stands
/// above every unrelated memory and below every answer.
pub const EMBED_MODEL: &str = "qwen/qwen3-embedding-8b";
pub const RERANK_MODEL: &str = "qwen/qwen3-reranker-8b";
pub const TURN_FLOOR: f32 = 0.5;
pub const SEARCH_FLOOR: f32 = 0.05;
pub const MEANING_FLOOR: f32 = 0.55;

/// What the embedding model reads ahead of a query, and the judge ahead of a turn's.
pub const QUERY_INSTRUCTION: &str =
    "Instruct: Given what an agent is about to do for the person it serves, retrieve memories about that person and \
     their life that bear on it\nQuery: ";
const TURN_JUDGE_INSTRUCTION: &str =
    "Instruct: Judge whether this memory about the person bears on what the agent is about to do for them\nQuery: ";

/// How strength bends a score, and how many days of disuse halve praise.
const STRENGTH_SPAN: f64 = 0.25;
const DISUSE_HALF_DAYS: f64 = 90.0;

const MESSAGE_CHARS: usize = 2000;
const WORK_CHARS: usize = 300;

/// The links that carry no recall: what a memory superseded is not recalled through it.
const SUPERSEDES: &str = "supersedes";

/// Why memories are ranked: to give a turn unasked, or to answer a search.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Purpose {
    Turn,
    Search,
}

/// What memories are searched with: texts whose words and meaning each give an order, and the query the judge reads.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct Queries {
    pub texts: Vec<String>,
    pub judged: String,
}

impl Queries {
    /// A search for query.
    pub fn of(query: &str) -> Self {
        Self { texts: vec![query.to_owned()], judged: query.to_owned() }
    }
}

fn chars(s: &str, n: usize) -> String {
    s.chars().take(n).collect()
}

/// The queries of a turn on `message` by an agent whose work is `work` (its role, or its name). Pure.
pub fn turn_queries(work: &str, message: &str) -> Queries {
    let message = chars(message.trim(), MESSAGE_CHARS);
    let work = chars(work.lines().map(str::trim).find(|l| !l.is_empty()).unwrap_or_default(), WORK_CHARS);
    let mut texts = vec![message.clone()];
    if !work.is_empty() {
        texts.push(work.clone());
    }
    Queries { judged: format!("{TURN_JUDGE_INSTRUCTION}{work}\n{message}"), texts }
}

/// The cosine of a and b; 0 when either is empty or their lengths differ.
pub fn cosine(a: &[f32], b: &[f32]) -> f32 {
    if a.len() != b.len() || a.is_empty() {
        return 0.0;
    }
    let (mut dot, mut na, mut nb) = (0.0_f32, 0.0_f32, 0.0_f32);
    for (x, y) in a.iter().zip(b) {
        dot += x * y;
        na += x * x;
        nb += y * y;
    }
    if na == 0.0 || nb == 0.0 {
        return 0.0;
    }
    dot / (na.sqrt() * nb.sqrt())
}

/// The `MEANING_TOP` vectors nearest query, nearest first, each with its cosine.
pub fn nearest(query: &[f32], vectors: &[(i64, Vec<f32>)]) -> Vec<(i64, f32)> {
    let mut near: Vec<(f32, i64)> = vectors.iter().map(|(id, v)| (cosine(query, v), *id)).collect();
    near.sort_by(|a, b| b.0.total_cmp(&a.0).then_with(|| a.1.cmp(&b.1)));
    near.into_iter().take(MEANING_TOP).map(|(c, id)| (id, c)).collect()
}

fn at(place: usize) -> f64 {
    let place = u32::try_from(place).unwrap_or(u32::MAX);
    1.0 / (RRF_K + f64::from(place) + 1.0)
}

fn fuse(orders: &[Vec<i64>]) -> HashMap<i64, f64> {
    let mut score: HashMap<i64, f64> = HashMap::new();
    for order in orders {
        for (place, id) in order.iter().enumerate() {
            *score.entry(*id).or_default() += at(place);
        }
    }
    score
}

/// One link between memories: (from, to, kind).
pub type Edge = (i64, i64, String);

/// score raised by one step over edges from its `WALK_FROM` best: a memory linked to one of them, either way but a
/// supersede, takes `LINKED_SHARE` of it. Returns the ids it reached.
fn walk(score: &mut HashMap<i64, f64>, edges: &[Edge]) -> HashSet<i64> {
    let mut best: Vec<(i64, f64)> = score.iter().map(|(id, s)| (*id, *s)).collect();
    best.sort_by(|a, b| b.1.total_cmp(&a.1).then_with(|| a.0.cmp(&b.0)));
    let from: HashMap<i64, f64> = best.into_iter().take(WALK_FROM).collect();
    let mut given: HashMap<i64, f64> = HashMap::new();
    let mut give = |id: i64, s: f64| {
        let held = given.entry(id).or_default();
        *held = held.max(s);
    };
    for (src, dst, kind) in edges {
        if kind == SUPERSEDES {
            continue;
        }
        if let Some(s) = from.get(src) {
            give(*dst, s * LINKED_SHARE);
        }
        if let Some(s) = from.get(dst) {
            give(*src, s * LINKED_SHARE);
        }
    }
    let reached: HashSet<i64> = given.keys().copied().collect();
    for (id, s) in given {
        *score.entry(id).or_default() += s;
    }
    reached
}

/// What the signals found: per text, the order of its words and of its meaning (each with its cosine).
#[derive(Debug, Clone, Default, PartialEq)]
pub struct Found {
    pub words: Vec<Vec<i64>>,
    pub meaning: Vec<Vec<(i64, f32)>>,
}

/// One memory to rank: its fused score (0 when no signal found it) and whether the signals agree on it.
#[derive(Debug, Clone, PartialEq)]
pub struct Candidate {
    pub id: i64,
    pub fused: f64,
    pub agreed: bool,
}

/// The candidates for what found holds: every memory a signal found or a link reached, fused, best first, none of given
/// and only what is in readable.
pub fn candidates(readable: &HashMap<i64, Memory>, found: &Found, edges: &[Edge], given: &HashSet<i64>) -> Vec<Candidate> {
    let mut orders: Vec<Vec<i64>> = found.words.clone();
    orders.extend(found.meaning.iter().map(|o| o.iter().map(|(id, _)| *id).collect()));
    let mut score = fuse(&orders);
    let close: HashSet<i64> =
        found.meaning.iter().flatten().filter(|(_, c)| *c >= MEANING_FLOOR).map(|(id, _)| *id).collect();
    let agreed: HashSet<i64> = found.words.iter().flatten().filter(|id| close.contains(id)).copied().collect();
    let reached = walk(&mut score, edges);
    let linked_to_agreed = |id: i64| {
        edges.iter().any(|(src, dst, kind)| {
            kind != SUPERSEDES && ((*src == id && agreed.contains(dst)) || (*dst == id && agreed.contains(src)))
        })
    };
    let mut out: Vec<Candidate> = score
        .into_iter()
        .filter(|(id, _)| readable.contains_key(id) && !given.contains(id))
        .map(|(id, fused)| Candidate {
            agreed: agreed.contains(&id) || (reached.contains(&id) && linked_to_agreed(id)),
            id,
            fused,
        })
        .collect();
    out.sort_by(|a, b| b.fused.total_cmp(&a.fused).then_with(|| a.id.cmp(&b.id)));
    out
}

/// What the judge reads: every memory of readable but given while they are `JUDGE_ALL` or fewer (the candidates first),
/// else the best `JUDGED` candidates.
pub fn to_judge(candidates: &[Candidate], readable: &HashMap<i64, Memory>, given: &HashSet<i64>) -> Vec<Candidate> {
    let open = readable.keys().filter(|id| !given.contains(*id)).count();
    if open > JUDGE_ALL {
        return candidates.iter().take(JUDGED).cloned().collect();
    }
    let mut out = candidates.to_vec();
    let mut rest: Vec<i64> = readable
        .keys()
        .filter(|id| !given.contains(*id) && !candidates.iter().any(|c| c.id == **id))
        .copied()
        .collect();
    rest.sort_unstable();
    out.extend(rest.into_iter().map(|id| Candidate { id, fused: 0.0, agreed: false }));
    out
}

fn time_of(text: &str) -> Option<NaiveDateTime> {
    NaiveDateTime::parse_from_str(text, "%Y-%m-%d %H:%M").ok()
}

/// How strength bends m's score at now: a factor in 1 ± `STRENGTH_SPAN`, from the mean of the Beta posterior of helped
/// against misled, centred on 0.5; praise fades over `DISUSE_HALF_DAYS` of disuse, blame does not.
pub fn strength(m: &Memory, now: NaiveDateTime) -> f64 {
    let (helped, misled) = (f64::from(m.helped), f64::from(m.misled));
    let lean = 2.0 * ((helped + 1.0) / (helped + misled + 2.0)) - 1.0;
    let lean = if lean > 0.0 {
        let last = m.last_recalled.as_deref().and_then(time_of).or_else(|| time_of(&m.created));
        let idle_days = last.map_or(0.0, |l| (now - l).num_minutes().max(0) as f64 / 1440.0);
        lean * 0.5_f64.powf(idle_days / DISUSE_HALF_DAYS)
    } else {
        lean
    };
    1.0 + STRENGTH_SPAN * lean
}

pub fn now() -> NaiveDateTime {
    Local::now().naive_local()
}

/// The memories to recall from pool, best first, at most limit: with relevance (the judge's score of each), those that
/// clear the purpose's floor ranked by relevance; without, a turn takes only those the signals agreed on (every one a
/// signal found when `words_only`: no model was there to agree) and a search every one a signal found, by fused score.
/// Either way weighed by trust and strength at now.
pub fn rank(
    readable: &HashMap<i64, Memory>,
    pool: &[Candidate],
    relevance: Option<&HashMap<i64, f32>>,
    purpose: Purpose,
    words_only: bool,
    limit: usize,
    now: NaiveDateTime,
) -> Vec<Memory> {
    let floor = match purpose {
        Purpose::Turn => TURN_FLOOR,
        Purpose::Search => SEARCH_FLOOR,
    };
    let mut ranked: Vec<(f64, &Memory)> = pool
        .iter()
        .filter_map(|c| {
            let m = readable.get(&c.id)?;
            let base = match relevance {
                Some(r) => {
                    let r = *r.get(&c.id)?;
                    (r >= floor).then_some(f64::from(r))?
                }
                None if purpose == Purpose::Turn && !c.agreed && !words_only => return None,
                None if c.fused <= 0.0 => return None,
                None => c.fused,
            };
            let trust = if m.trust == "confirmed" { CONFIRMED_WEIGHT } else { 1.0 };
            Some((base * trust * strength(m, now), m))
        })
        .collect();
    ranked.sort_by(|a, b| {
        b.0.total_cmp(&a.0).then_with(|| b.1.updated.cmp(&a.1.updated)).then_with(|| a.1.id.cmp(&b.1.id))
    });
    ranked.into_iter().take(limit).map(|(_, m)| m.clone()).collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    fn mem(id: i64) -> Memory {
        Memory {
            id,
            holder: "assistant".into(),
            kind: "fact".into(),
            title: format!("m{id}"),
            body: String::new(),
            created: "2026-11-01 08:00".into(),
            updated: "2026-11-01 08:00".into(),
            ended: None,
            why_ended: None,
            recalled: 0,
            trust: "auto".into(),
            helped: 0,
            misled: 0,
            last_recalled: None,
            links: Vec::new(),
            source: None,
            evidence: String::new(),
        }
    }

    fn edge(src: i64, dst: i64, kind: &str) -> Edge {
        (src, dst, kind.to_owned())
    }

    // remit's main-path test, its subjects left out: candidates are what words or meaning found and what links to them,
    // never what the session was given, what a supersede points back to, or what the agent may not read.
    #[test]
    fn recall_gives_a_turn_what_the_judge_stands_behind_and_without_one_what_the_signals_agree_on() {
        let (a, b, c, e, f, old, z) = (1, 2, 3, 5, 6, 7, 8);
        let readable: HashMap<i64, Memory> = [a, b, c, e, f, old, z].map(|id| (id, mem(id))).into();
        let edges = vec![edge(a, c, "because"), edge(a, old, "supersedes"), edge(b, 99, "related")];
        let given: HashSet<i64> = [f].into();
        let found = Found { words: vec![vec![b, a, e, f]], meaning: vec![vec![(a, 0.7), (e, 0.4), (f, 0.9)]] };
        let cands = candidates(&readable, &found, &edges, &given);
        assert_eq!(cands.iter().map(|c| c.id).collect::<Vec<_>>(), [a, e, b, c]);

        let pool = to_judge(&cands, &readable, &given);
        assert_eq!(pool.iter().map(|c| c.id).collect::<Vec<_>>(), [a, e, b, c, old, z], "while few, the judge reads all");

        let now = time_of("2026-11-25 08:00").unwrap();
        let judged: HashMap<i64, f32> = [(a, 0.9), (b, 0.2), (c, 0.7), (e, 0.01), (z, 0.8)].into();
        let got = |r: Option<&HashMap<i64, f32>>, p: Purpose, words_only: bool| -> Vec<i64> {
            rank(&readable, &pool, r, p, words_only, 10, now).into_iter().map(|m| m.id).collect()
        };
        assert_eq!(got(Some(&judged), Purpose::Turn, false), [a, z, c]);
        assert_eq!(got(Some(&judged), Purpose::Search, false), [a, z, c, b]);
        assert_eq!(got(None, Purpose::Turn, false), [a, c], "e's words agree but its meaning is far");
        assert_eq!(got(None, Purpose::Turn, true), [a, e, b, c], "with no model at all, what words find");
    }

    #[test]
    fn praise_lifts_and_fades_with_disuse_blame_stays() {
        let now = time_of("2026-11-25 08:00").unwrap();
        let mut m = mem(1);
        assert!((strength(&m, now) - 1.0).abs() < 1e-9);
        m.helped = 8;
        m.last_recalled = Some("2026-11-25 08:00".into());
        let fresh = strength(&m, now);
        m.last_recalled = Some("2026-08-27 08:00".into());
        let idle = strength(&m, now);
        assert!(fresh > idle && idle > 1.0, "{fresh} {idle}");
        m.helped = 0;
        m.misled = 8;
        assert!((strength(&m, now) - 0.8).abs() < 1e-9, "blame bends it down by its share");
    }
}
