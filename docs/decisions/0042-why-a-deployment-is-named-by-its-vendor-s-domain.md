# 42. Why a deployment is named by its vendor's domain

Date: 2026-11-28

## Status

Accepted

## Context

A candidate is an endpoint and a model id; the house is read off the endpoint
so that no second field can disagree with the first. The reader took the
leftmost DNS label, which is the house for `api.deepseek.com` and
`dashscope.aliyuncs.com`. It is not the house when the host is multi-tenant: a
dedicated Bailian deployment lives at
`llm-<id>.cn-beijing.maas.aliyuncs.com`, where the leftmost label is the
customer's deployment id, and iFlytek's Spark publishes
`spark-api-open.xf-yun.com`, where it is a product name with a hyphen.

The cost was concrete: the deployment was filed as its own house, so the
request carried `reasoning_effort` instead of the gateway's
`enable_thinking`/`thinking_budget`, and the key variable became
`LLM-<ID>_API_KEY`, a name no shell can export. The person could repair it only
by knowing and typing `dialect qwen`.

## Options considered

- **Keep the first label and document `dialect <name>`.** No code, but every
  person with a deployment pays the same discovery cost, and the key variable
  stays unsettable.
- **Add each deployment host to the exact table.** Correct per host, but a
  deployment id cannot be enumerated in advance; the table would grow with
  customers, not with vendors.
- **Learn the dialect from a refusal.** The runner already corrects a refusal
  once per task. It cannot help here: a gateway that silently ignores
  `reasoning_effort` returns 200, so there is no refusal to learn from, and
  when there is one the correction turns every rung off rather than trying the
  gateway's field.
- **Guess from the model id (`qwen*`).** The model id names the model, not the
  server; a proxy can serve Qwen through the OpenAI fields, and aliases are
  free.
- **Read the vendor off the domain it owns.** A suffix names who answers no
  matter which deployment or region sits to its left.

## Decision

`pearl_core::catalog::provider_name` checks exact hosts (`ODD_HOUSE`) first,
then a short table of vendor domains (`HOUSE_SUFFIX`), then falls back to the
first label. The suffix matches only at a dot boundary, so
`notmaas.aliyuncs.com` and `oss-cn-beijing.aliyuncs.com` are not claimed. The
table names the Bailian model gateways (`maas.aliyuncs.com`,
`maas.qwencloudapi.com`, `maas.qianwenaiapi.com`) as `dashscope`; Spark's
published host is an exact entry, because it is not a domain of deployments.
The two regional fronts of the same house, `dashscope-intl.aliyuncs.com` and
`dashscope-us.aliyuncs.com`, are exact entries too: the product name, not a
deployment id, is in the leftmost label, and the list still offers one address
per house.

`pearl_provider::auth::provider_name` delegates to that function instead of
carrying a copy, so the header, the key variable and the row cannot name the
house differently.

## Consequences

A deployment is named by its vendor with no user action: the dialect is Qwen's,
the row reads `dashscope/<model>`, and `DASHSCOPE_API_KEY` reaches the endpoint
it belongs to. The same rule covers every deployment and region under those
domains, including ones not yet seen.

Cost: each suffix is a claim about an entire domain rather than one machine, so
a non-model host under `maas.aliyuncs.com` would be called `dashscope`; the list
stays short, matches at a dot, and is not a general "ends with the vendor's
name" test. Two deployments of the same house serving the same model id also
produce the same `dashscope/<model>` label, and a person who keeps both must
rely on the address column to tell them apart.