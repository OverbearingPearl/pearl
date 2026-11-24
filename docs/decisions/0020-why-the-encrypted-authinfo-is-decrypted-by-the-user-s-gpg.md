# 20. Why the encrypted authinfo is decrypted by the user's gpg

Date: 2026-11-24

## Status

Accepted

## Context

A user who keeps credentials encrypted keeps them in `~/.authinfo.gpg`, and
pearl did not read it: `default_path` offered only the plaintext file, and its
doc said an encrypted file needs a decryptor this does not have. Such a user
had to export a key into the environment or write a plaintext copy -- both
defeat the reason the file is encrypted. Emacs reads the encrypted form, so
the file is already there on machines that have one; what was missing was a
way to ask for its contents.

## Options considered

- **sequoia-openpgp**: pure Rust and maintained, but a large dependency tree
  for one subprocess-shaped act, and it would put key handling, passphrase
  prompts and algorithm choices inside our process.
- **rpgp**: lighter than sequoia, but the same shape of cost: a second
  OpenPGP implementation to keep current, and keys read into our memory
  instead of staying in the agent the user already trusts.
- **Shell out to the user's `gpg`**: what Emacs itself does; no new
  dependency, no key handling in our process, and the user's agent and
  pinentry answer as they do for every other tool on the machine.

## Decision

A `.gpg` authinfo path is decrypted by running the user's own
`gpg --quiet --batch --decrypt <path>` and parsing its stdout with the
existing `parse`. `default_path` prefers `$AUTHINFO`, then an existing
`~/.authinfo`, then an existing `~/.authinfo.gpg`; when neither file exists
it offers no path.

`--batch` makes gpg fail rather than prompt when no agent can answer, and
the wait is bounded besides (ten seconds, then the child is killed): a
pinentry that appears anyway must not hold the surface forever. A missing
gpg, a non-zero exit or a timeout yields no credential. There is no fallback
to reading the bytes as text -- a `.gpg` name is a claim about the contents,
and parsing ciphertext (or plaintext wearing the name) as authinfo would be
a silent wrong answer. The decrypted text is returned to the parser and
never logged.

## Consequences

The encrypted file works with the keys, the agent and the pinentry the user
already has, and pearl carries no OpenPGP code to audit or keep current.
Decryption happens at use, like every other read of the key, so nothing
decrypted is stored.

Cost: gpg must be installed to read the encrypted form -- a machine without
it gets no key from a `.gpg` file, reported as no credential rather than a
silent fallback -- and each read of an encrypted file pays a subprocess and,
when the agent holds nothing, a pinentry the bounded wait may cut off.