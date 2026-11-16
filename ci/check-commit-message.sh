#!/bin/sh
# Check the mechanical half of the commit message rules: the parts a script can
# see. The prose rules -- why the change was made, one logical change per
# commit, no padding -- are still on the author, and no hook can read them.
#
# Called by the commit-msg hook with the path to the message file. Exit 1 on a
# violation; a subject between 60 and 80 columns is a warning, not a failure,
# because the rules call those soft and hard limits.
set -eu

msg=${1:-}
[ -n "$msg" ] && [ -f "$msg" ] || {
  echo "usage: $0 <commit-message-file>" >&2
  exit 2
}

fail=0
max_subject=80
soft_subject=60
max_body=72

problem() {
  echo "commit message: $1" >&2
  fail=1
}

# Drop the comment lines git would strip, so a template's own text is not read
# as part of the message.
body=$(sed -e '/^#/d' -e '/^# ------------------------ >8/q' "$msg")
subject=$(printf '%s\n' "$body" | sed -n '1p')

[ -n "$subject" ] || problem "no subject line"

# Subject: capital, imperative-looking, no prefix, no trailing period.
case $subject in
  [A-Z]*) ;;
  *) problem "subject does not start with a capital letter" ;;
esac
case $subject in
  *.) problem "subject ends with a period" ;;
esac
case $subject in
  *:*)
    prefix=${subject%%:*}
    case $prefix in
      *\ *) ;;
      *) problem "subject starts with a '$prefix:' style prefix" ;;
    esac
    ;;
esac

len=${#subject}
[ "$len" -le "$max_subject" ] || problem "subject is $len columns, hard limit is $max_subject"
if [ "$len" -gt "$soft_subject" ]; then
  echo "commit message: subject is $len columns, past the soft limit of $soft_subject" >&2
fi

# Body: lines within 72 columns. A line with no space in it is a URL or a path,
# and those are allowed to run long.
printf '%s\n' "$body" | awk -v max="$max_body" '
  NR > 2 && length($0) > max && $0 ~ / / && $0 !~ /https?:\/\// {
    printf "commit message: body line %d is %d columns, limit is %d\n", NR, length($0), max > "/dev/stderr"
    print "  " substr($0, 1, 60) > "/dev/stderr"
    bad = 1
  }
  END { exit bad ? 1 : 0 }' || fail=1

# The second line must be blank when there is a body at all, and the body must
# not be a restatement of the diff.
second=$(printf '%s\n' "$body" | sed -n '2p')
lines=$(printf '%s\n' "$body" | wc -l | tr -d ' ')
if [ "$lines" -gt 1 ] && [ -n "$second" ]; then
  problem "second line is not blank (the body needs one empty line after the subject)"
fi

# These are forbidden as statements and as trailers, which is how they are
# written; a message that discusses the rule may name it mid-sentence.
for phrase in "This commit fixes" "I changed" "This PR" "Generated with" "Co-authored-by"; do
  if printf '%s\n' "$body" | grep -Fqi "$phrase" 2>/dev/null; then
    if printf '%s\n' "$body" | grep -qi "^[[:space:]]*$phrase" 2>/dev/null; then
      problem "opens a line with '$phrase'"
    fi
  fi
done

for word in improve enhance robust seamless comprehensive leverage utilize; do
  if printf '%s\n' "$body" | grep -Fqiw "$word"; then
    problem "contains the marketing word '$word'"
  fi
done

if printf '%s\n' "$body" | LC_ALL=C grep -q '[^ -~	]'; then
  echo "commit message: non-ASCII characters; typographic dashes are fine, emoji are not" >&2
fi

exit "$fail"
