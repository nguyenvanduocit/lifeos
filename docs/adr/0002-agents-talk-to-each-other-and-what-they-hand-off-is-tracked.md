# 0002. Agents talk to each other, and what they hand off is tracked

Status: accepted, 2026-11-25. Built. Amends 0001's "Agents talk through messages, never directly" and its Messages step.

## Context

Under 0001 the Assistant was the hub: it sent a member `message_agent`, a member asked it `ask_assistant`, and members
could not reach each other. A reply came back as one prompt per message, with no record of who had what: asked "how is
it going?", the Assistant had nothing to read, a member's work lost to a restart or dropped by the owner's Stop was never
heard of again, and an agent in the middle of a chain could not take its colleague's check into its own work.

agentes.chat (`references/agentes-chat`, AGPL, read for reference only) solved the same problem for a team workspace:
`ask_agent` and `tell_agent` between any agents (its ADR 0034), every hand-off a row that comes back by itself, once,
with the rest of the turn's, rolled up through the agent in the middle, never expiring while a colleague works (its ADR
0095). It tried making the asker wait on the answer and took it out: agents that paused said "meanwhile I'll look at
the code" and could not. lifeos has one owner, so its audience rules, channel leads and judges do not apply.

## Decision

1. **Any agent asks or hands work to any other.** Every agent but the librarian has `ask_agent {agent, question}`
   (an answer it needs) and `hand_off {agent, task, heads_up?}` (work that is the other's part). They replace
   `message_agent` and `ask_assistant`. Nothing waits: the tool returns at once. `heads_up` is news: no turn starts,
   nothing comes back, and the target reads it in `<news>` with its next message.
2. **Every one is a row** (`handoffs`, migration v14; `Handoff` in lifeos-wire; read tool `list_handoffs`): kind,
   sender, target, text, status (queued → working → waiting → done | failed | cancelled), result, `parent`, `chain`,
   attempts, the health check's note. The target's prompt carries the hand-off's id (`PromptData.handoff`); the turn on
   it marks it working, and its end closes it with the turn's last message, or as failed when the turn failed or was
   stopped.
3. **What one piece of work handed out comes back together, once.** A row's `batch` is the hand-off the sender was
   working on (`h:<id>`) or else the sender's turn (`p:<prompt>`). Whatever closes a row claims its batch
   (`claim_batch`); the claim succeeds only when none of the batch is still out, so the sender gets one prompt,
   `[what came of what you handed off]`, with every result and what to do next. Results the sender only took back
   itself (cancelled) are not sent.
4. **The agent in the middle goes on.** An agent whose turn on hand-off P handed part of it on keeps P open (`waiting`,
   its answer so far held). When the parts come back, their prompt carries P, so the agent takes a turn on them —
   check, fix what they found, put the parts together — and that turn's last message closes P and goes up. Unlike
   agentes.chat, no judge decides whether it goes on: in lifeos it always does, at the cost of one turn.
5. **Loops are bounded in code.** An ask or task may not go to an agent already in its chain (it waits on the sender),
   a chain holds at most `MAX_CHAIN` = 4 agents, the same words to the same agent are refused while the first is out,
   and remit's no-progress rail still counts (it now counts turns on results too).
6. **Everyone sees where the work is.** Each turn of an agent reads `<handed-off>`: what it handed out still out or
   closed within the hour, newest first, at most 8, with where each stands. `team_status` gives every agent's standing
   and everything still out. `cancel_handoff` takes one back (dropped if queued, its turn stopped if at work, and what
   it handed on with it); the Assistant may take back any, on the owner's word. The Assistant panel pins an agent's
   open hand-offs, sent and received, under its stream.
7. **Health check** (`handoffs::keep_handoffs`): two seconds after lifeos starts, every minute, and when the owner stops
   an agent. A hand-off whose prompt was dropped from the outbox fails, with why. One lost while worked on (lifeos
   restarted; the turn is gone) is handed again once, saying so, then fails. One at work whose agent did nothing for
   15 minutes is marked quiet; after 2 hours its turn is stopped and it fails. One waiting whose parts all came back
   but has no turn left to take them closes with the answer it held. A hand-off to a retired agent fails. Every close
   reports its batch, so the sender always hears, and can hand the work again or fix it.

## Consequences

- A delegating turn costs one more turn of the sender per batch, and the agent in the middle one more turn per batch
  it handed on. No model call is added for judging.
- The Assistant stops being the only path between members; it remains the one that decides `ask_owner` questions and
  manages the crew.
- Not done: a cap on how much one owner word may fan out (agentes.chat's stretch limits); a hand-off's files travelling
  with it; showing the two agents' exchange as one thread in the panel (each agent's stream still shows its side).
