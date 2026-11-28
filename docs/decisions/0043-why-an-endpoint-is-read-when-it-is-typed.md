# 43. Why an endpoint is read when it is typed

Date: 2026-11-28

## Status

Accepted

## Context

A human meets an endpoint in a quickstart, a colleague's message, or a
provider's console, and what they meet is the whole request URL:
`https://openrouter.ai/api/v1/chat/completions`. PEARL appends
`/chat/completions` itself, so storing that string verbatim asks for the path
twice and turns a working account into a 404 with an empty body. Leaving off
the scheme fails differently: as a URL it cannot be parsed, and `reqwest`
reports it as `builder error`, which names nothing. A path from another API
(`/responses`, `/messages`) is a third shape, and the provider appending its own
path to it produces a fourth.

None of these is a mistake the human should have to discover by reading a
status line. They are the same place written four ways, and the surface is the
one that knows which part of the URL it is going to write.

## Options considered

1. **Keep exactly what was typed.** The provider's append is then a rule the
   human has to know and apply; every quickstart URL is a trap.
2. **Refuse anything but the canonical base.** Correct, and it makes the human
   edit a URL by hand to satisfy a form, which is the burden this project is
   trying to remove.
3. **Read the address at entry and say what was read.** The base is kept, a
   missing scheme is supplied by the host, another API's path is replaced with
   the one PEARL speaks, and the notice names the wire URL.
4. **Guess and stay silent.** Rewriting without a note makes a 404 on the
   corrected address look like PEARL's own bug.

## Decision

An address is read when it is entered, in one place
(`catalog::complete_endpoint`), and both the picker and the settings grammar
use that reading:

- a whole URL loses its `/chat/completions`; a base keeps its path;
- the scheme defaults by host — `http` for this machine (localhost, `.local`,
  loopback, private and link-local addresses), `https` otherwise;
- a public house written with `http` is refused by name: the key travels with
  the request, so the scheme is part of the promise;
- `/responses` and `/messages` are replaced with `/chat/completions`, and the
  notice says PEARL speaks openai-chat only.

The list shows the wire URL (`catalog::wire_url`), not the stored base, so the
row reads as the thing a request will actually be sent to.

## Consequences

A human may paste a quickstart URL, omit the scheme, or arrive from another
API, and land on a row that works without editing the string first. The two
examples under the prompt (`https://openrouter.ai/api/v1`,
`http://localhost:11434/v1`) show a whole URL with the typed base carrying
weight, because a string that must be produced from memory needs an example
more than a rule does.

Cost: a public server that only speaks `http` cannot be reached even if it
works; `api.anthropic.com/v1/messages` is accepted as a chat base although that
host cannot answer on it, so the correction removes a symptom only where the
host is compatible; and the string a human typed is not the string they see
afterwards, which is a surprise unless the notice is read.
