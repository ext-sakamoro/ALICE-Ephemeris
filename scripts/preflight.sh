#!/usr/bin/env bash
# scripts/preflight.sh — local reproduction of the CI gates before `git push`.
# Every command is the one CI runs. A step this file does not cover is a step
# that can only fail remotely — when a workflow changes, regenerate this file
# in the same commit (checks no workflow runs go in scripts/preflight.local.sh).
# Run `--quick` before every push.
#
# usage: scripts/preflight.sh [--quick]   (--quick skips the test / bench suites)
# generator rev 94ac16beb455be99e4675fe0648f996589e96bee
# local steps: scripts/preflight.local.sh 045135b5b7d0bf9e139903b699852ec18ae72e20
set -euo pipefail
cd "$(dirname "$0")/.."
# --- heavy slot (begin) ---
# ALICE_HEAVY_SLOTS (default 2, 1..16) heavy runs at a time on this machine. Slot k is a lock the kernel holds (flock
# on $ALICE_HEAVY_SLOT_DIR/lock.<k>, default $HOME/.cache/alice-heavy-slot) for as long as its holder lives. A small
# keeper process (perl) takes it and exits within a second of the holder's death, so a killed holder frees the slot
# and no child of the holder keeps it. Waiters queue: each keeper leaves a ticket in queue/ named
# `<start time %017.6f>-<pid>` for a `work` take and `0<start time>-<pid>` for a `land` take (a work ticket older
# than ALICE_HEAVY_SLOT_AGE_SECS, default 9000, renames itself to the land form), and only the first live ticket in
# the names' sort order may take a free slot: land before work, then by arrival. Copies of this block from before
# the classes write work tickets and use the same sort, so old and new waiters agree on the order. A ticket whose
# keeper is gone (a crash) is skipped and removed. A priority file ($ALICE_HEAVY_SLOT_DIR/priority: one label per
# line, top first; `#` starts a comment) ranks the waiters whose label is a line or starts with "<line>-" before all
# others: their tickets are named `00<rank, 2 digits><0 land | 1 work><start time>-<pid>`, so they sort first for
# every copy of this block, an older one too (it sorts the names). A waiter re-reads the file each poll and renames
# its ticket when its rank changes; an unlisted label keeps the names above; a line that is not a label is ignored
# with a warning; a rank never takes a held slot. The light slot (heavy_slot_acquire <label> <tool> <class> light)
# is the same with light.0, queue-light/, info-light.0 and a `light-keeper` title. A waiter prints its place and its wait once a minute. The keeper of slot k names
# itself `alice-heavy-slot keeper slot=<k> holder=<pid> dirhex=<the dir's bytes in hex> end` (its process title,
# which no file cleanup removes; the dir in hex so no dir can end the title early), and the holder of slot k is read
# from there (only from a perl process, and only while the holder lives); `info.<k>` only adds the holder's label and tool, and a
# release removes it only while it names the releasing shell. A wait past ALICE_HEAVY_SLOT_TIMEOUT seconds (default 7200 for land, 14400 for work) fails; the holder's own nested runs
# (ALICE_HEAVY_SLOT_HELD) run inside its slot.
heavy_slot_dir() { printf '%s\n' "${ALICE_HEAVY_SLOT_DIR:-$HOME/.cache/alice-heavy-slot}"; }
heavy_slot_count() {
  case "${ALICE_HEAVY_SLOTS:-2}" in
    [1-9] | 1[0-6]) printf '%s\n' "${ALICE_HEAVY_SLOTS:-2}" ;;
    *) echo "heavy slot: ALICE_HEAVY_SLOTS must be an integer from 1 to 16 (got '${ALICE_HEAVY_SLOTS:-}')" >&2; return 1 ;;
  esac
}
# whether $1 is a perl process (a keeper): its title is its own, so the executable is checked (macOS keeps it as
# the accounting name `ucomm`; Linux sets comm from the title, so the executable's path is read there)
heavy_slot_is_perl() {
  local exe
  if [ -e "/proc/$1/exe" ]; then
    exe=$(readlink "/proc/$1/exe" 2>/dev/null) || return 1
    case "${exe##*/}" in perl*) return 0 ;; *) return 1 ;; esac
  fi
  [ "$(ps -o ucomm= -p "$1" 2>/dev/null | tr -d ' ')" = perl ]
}
# "<k> <holder pid>" of every slot of $1 held by a live keeper (from the keepers' process titles: a perl process
# whose title names this dir, while its holder lives). A slot whose keeper has no title (a keeper of a copy of this
# block from before the titles: an older generated preflight, or a run started before the update) is read from
# info.<k> while its holder lives and lock.<k> is held
heavy_slot_live() {
  local d=$1 hex titled="" f k h p
  hex=$(printf '%s' "$d" | od -An -tx1 | tr -d ' \n')
  while read -r p k h; do
    heavy_slot_is_perl "$p" || continue
    kill -0 "$h" 2>/dev/null || continue
    titled="${titled:+$titled
}$k $h"
  done < <(ps -A -ww -o pid=,command= 2>/dev/null | awk -v hex="$hex" '
    { if (match($0, /alice-heavy-slot keeper slot=[0-9]+ holder=[0-9]+ dirhex=[0-9a-f]+ end/)) {
        split(substr($0, RSTART, RLENGTH), w, /[ =]/)
        if (w[8] == hex) print $1, w[4], w[6] } }')
  {
    [ -n "$titled" ] && printf '%s\n' "$titled"
    for f in "$d"/info.*; do
      k=${f##*/info.}
      case "$k" in '' | *[!0-9]*) continue ;; esac
      printf '%s\n' "$titled" | awk -v k="$k" '$1 == k { found = 1 } END { exit !found }' && continue
      h=$(sed -n 1p "$f" 2>/dev/null)
      case "$h" in '' | *[!0-9]*) continue ;; esac
      kill -0 "$h" 2>/dev/null || continue
      perl -MFcntl=:flock -e 'open(my $f, ">>", $ARGV[0]) or exit 1; exit(flock($f, LOCK_EX | LOCK_NB) ? 1 : 0)' \
        "$d/lock.$k" 2>/dev/null || continue
      printf '%s %s\n' "$k" "$h"
    done
  } | sort -n
}
# "label pid (tool)" of every held slot, joined with ", " (the label and tool from info.<k> while it names the
# holder, `unlabelled` otherwise)
heavy_slot_holders() {
  local d=$1 k h f out=""
  while read -r k h; do
    f="$d/info.$k"
    if [ "$(sed -n 1p "$f" 2>/dev/null)" = "$h" ]; then
      out="${out:+$out, }$(sed -n 3p "$f") pid $h ($(sed -n 4p "$f"))"
    else
      out="${out:+$out, }unlabelled pid $h (info.$k missing)"
    fi
  done < <(heavy_slot_live "$d")
  printf '%s\n' "${out:-nobody}"
}
heavy_slot_acquire() {
  local label=$1 tool=$2 class=${3:-${ALICE_HEAVY_SLOT_CLASS:-work}} pool=${4:-heavy} d n k f line err \
    limit="${ALICE_HEAVY_SLOT_TIMEOUT:-}" age="${ALICE_HEAVY_SLOT_AGE_SECS:-9000}" queue=queue lock=lock. \
    info=info. title=keeper
  case "$class" in land | work) ;; *) echo "heavy slot: the class must be land or work (got '$class')" >&2; return 2 ;; esac
  if [ -z "$limit" ]; then if [ "$class" = land ]; then limit=7200; else limit=14400; fi; fi
  case "$age" in '' | *[!0-9]*) echo "heavy slot: ALICE_HEAVY_SLOT_AGE_SECS must be a whole number of seconds" >&2; return 2 ;; esac
  d=$(heavy_slot_dir)
  n=$(heavy_slot_count) || return 1
  if [ -n "${ALICE_HEAVY_SLOT_HELD:-}" ] && kill -0 "$ALICE_HEAVY_SLOT_HELD" 2>/dev/null; then
    heavy_slot_live "$d" | awk -v h="$ALICE_HEAVY_SLOT_HELD" '$2 == h { found = 1 } END { exit !found }' && return 0
  fi
  if [ "$pool" = light ]; then
    # one light slot; a light take inside a light holder runs inside it
    if [ -n "${ALICE_HEAVY_SLOT_LIGHT_HELD:-}" ] && kill -0 "$ALICE_HEAVY_SLOT_LIGHT_HELD" 2>/dev/null; then return 0; fi
    n=1 queue=queue-light lock=light. info=info-light. title=light-keeper
  fi
  if ! err=$(mkdir -p "$d/$queue" 2>&1); then
    echo "heavy slot: cannot create $d: ${err:-not writable}" >&2
    return 1
  fi
  for k in $(seq 0 $((n - 1))); do
    if ! err=$({ : >> "$d/$lock$k"; } 2>&1); then
      echo "heavy slot: cannot create $d: ${err:-not writable}" >&2
      return 1
    fi
  done
  exec 8< <(exec perl -MFcntl=:flock -MTime::HiRes=time,sleep -e '
    my ($dir, $n, $limit, $class, $age, $qname, $lock, $title, $label) = @ARGV; my $parent = getppid(); $| = 1;
    my $q = "$dir/$qname";
    my $stamp = sprintf("%017.6f", time);
    my %warned;
    my $name = sub {
      my $r;
      if (defined $label && open(my $pf, "<", "$dir/priority")) {
        my ($i, $ln) = (0, 0);
        while (my $line = <$pf>) {
          $ln++; chomp $line; (my $l = $line) =~ s/#.*//; $l =~ s/^\s+|\s+$//g;
          next if $l eq "";
          if ($l !~ /^[A-Za-z0-9._-]+$/) {
            print STDERR "heavy slot: priority file line $ln ignored (not a label): $line\n" unless $warned{$ln}++;
            next;
          }
          if ($label eq $l || index($label, "$l-") == 0) { $r = $i; last }
          last if ++$i > 99;
        }
        close $pf;
      }
      my $landish = $class =~ /^land/;
      return defined $r ? sprintf("00%02d%s%s-%d", $r, $landish ? "0" : "1", $stamp, $$) : ($landish ? "0" : "") . "$stamp-$$";
    };
    my $me = $name->();
    my $ticket = "$q/$me";
    open(my $tf, ">", $ticket) or do { print "error $!\n"; exit 2 }; close $tf;
    END { unlink $ticket if defined $ticket }
    my @fh;
    for my $k (0 .. $n - 1) { open($fh[$k], ">>", "$dir/$lock$k") or do { print "error $!\n"; exit 2 } }
    print "pid $$\n";
    my $start = time; my $note = 0;
    while (1) {
      my $want = $name->();
      if ($want ne $me && rename($ticket, "$q/$want")) { $me = $want; $ticket = "$q/$me" }
      opendir(my $dh, $q) or do { print "error $!\n"; exit 2 };
      my @live;
      for my $t (sort grep { /^\d/ } readdir $dh) {
        my ($p) = $t =~ /-(\d+)$/;
        if ($p && ($p == $$ || kill(0, $p))) { push @live, $t } else { unlink "$q/$t" }
      }
      closedir $dh;
      my ($place) = grep { $live[$_] eq $me } 0 .. $#live;
      if (defined $place && $place == 0) {
        for my $k (0 .. $n - 1) {
          next unless flock($fh[$k], LOCK_EX | LOCK_NB);
          unlink $ticket; undef $ticket;
          $0 = "alice-heavy-slot $title slot=$k holder=$parent dirhex=" . unpack("H*", $dir) . " end";
          print "held $k\n"; close(STDOUT);
          sleep 1 while getppid() == $parent;
          exit 0;
        }
      }
      my $w = int(time - $start);
      if ($class eq "work" && $w >= $age) { $class = "land (promoted)" }  # (renamed at the next poll)
      if ($w >= $limit) { print "timeout\n"; exit 1 }
      if ($w >= $note) { printf "wait %d %d %s\n", ($place // 0) + 1, $w, $class; $note += 60 }
      sleep 0.2;
      exit 3 if getppid() != $parent;
    }
  ' "$d" "$n" "$limit" "$class" "$age" "$queue" "$lock" "$title" "$label")
  while IFS= read -r line <&8; do
    case "$line" in
      "pid "*) HEAVY_SLOT_KEEPER=${line#pid } ;;
      "wait "*)
        set -- $line
        case "$2" in 1) k=1st ;; 2) k=2nd ;; 3) k=3rd ;; *) k="$2th" ;; esac
        echo "heavy slot: waiting ($k in the queue, $3 s) for $(heavy_slot_holders "$d"); ${pool} slot, as $4${5:+ $5}" >&2
        ;;
      "held "*)
        exec 8<&-
        HEAVY_SLOT_INDEX=${line#held }
        HEAVY_SLOT_INFO="$d/$info$HEAVY_SLOT_INDEX"
        printf '%s\n%s\n%s\n%s\n%s\n' "$$" "$(date +%s)" "$label" "$tool" "$class" > "$HEAVY_SLOT_INFO"
        if [ "$pool" = light ]; then
          ALICE_HEAVY_SLOT_LIGHT_HELD=$$
          export ALICE_HEAVY_SLOT_LIGHT_HELD
        else
          ALICE_HEAVY_SLOT_HELD=$$
          export ALICE_HEAVY_SLOT_HELD
        fi
        return 0
        ;;
      timeout)
        exec 8<&-
        echo "heavy slot: waited ${limit}s for $(heavy_slot_holders "$d"); giving up (ALICE_HEAVY_SLOT_TIMEOUT=$limit)" >&2
        return 1
        ;;
      "error "*)
        exec 8<&-
        echo "heavy slot: cannot open $d/lock: ${line#error }" >&2
        return 1
        ;;
    esac
  done
  exec 8<&-
  echo "heavy slot: the lock keeper stopped (is perl on PATH?)" >&2
  return 1
}
heavy_slot_release() {
  local d f
  d=$(heavy_slot_dir)
  if { [ "${ALICE_HEAVY_SLOT_HELD:-}" = "$$" ] || [ "${ALICE_HEAVY_SLOT_LIGHT_HELD:-}" = "$$" ]; } &&
    [ -n "${HEAVY_SLOT_KEEPER:-}" ]; then
    # the info first: once the keeper is gone the next holder writes its own. Only while it names this shell: when
    # the keeper died first (`pkill -P` on the holder), the slot may already be another holder's, with its info
    f=${HEAVY_SLOT_INFO:-$d/info.${HEAVY_SLOT_INDEX:-}}
    if [ "$(sed -n 1p "$f" 2>/dev/null)" = "$$" ]; then
      rm -f "$f"
    fi
    kill "$HEAVY_SLOT_KEEPER" 2>/dev/null || true
    HEAVY_SLOT_KEEPER=
  fi
}
# memory: `heavy_slot_watch <pid> &` sums the RSS of that process and its descendants about once a second
# (ALICE_HEAVY_SLOT_RSS_POLL) and past ALICE_HEAVY_SLOT_RSS_CAP_MIB (default 8192 = 8 GiB) kills the tree (itself
# aside) and prints `[heavy-slot] RSS cap <N>GiB exceeded by <pid> <cmd> (tree <M> MiB)`. It also sums the trees of
# every held slot; past ALICE_HEAVY_SLOT_RSS_TOTAL_MIB (default 20480 = 20 GiB) the watcher whose slot holds the
# largest tree kills its own tree and prints `[heavy-slot] RSS total cap ...` (every slot's watcher looks, so no
# coordinator is needed). It returns once the process is gone. A time limit does not bound memory (a test that loops
# while pushing fills the machine inside its timeout), this does. A `ps` that cannot list the tree turns the cap off
# with a note, not silently.
heavy_slot_cap_ok() {
  case "${ALICE_HEAVY_SLOT_RSS_CAP_MIB:-8192}" in
    '' | *[!0-9]* | 0) echo "heavy slot: ALICE_HEAVY_SLOT_RSS_CAP_MIB must be a positive integer" >&2; return 2 ;;
  esac
  case "${ALICE_HEAVY_SLOT_RSS_TOTAL_MIB:-20480}" in
    '' | *[!0-9]* | 0) echo "heavy slot: ALICE_HEAVY_SLOT_RSS_TOTAL_MIB must be a positive integer" >&2; return 2 ;;
  esac
}
# from a `ps -A -o pid=,ppid=,rss=,comm=` listing on stdin: "<pid> <rss KiB> <command>" for the process $1 and its
# descendants, leaving out $2 and its descendants
heavy_slot_tree_in() {
  awk -v root="$1" -v skip="${2:-}" '
    { pid[NR] = $1; ppid[NR] = $2; rss[NR] = $3; $1 = $2 = $3 = ""; cmd[NR] = substr($0, 4) }
    END {
      in_tree[root] = 1; grew = 1
      while (grew) {
        grew = 0
        for (i = 1; i <= NR; i++)
          if (!(pid[i] in in_tree) && pid[i] != skip && (ppid[i] in in_tree)) { in_tree[pid[i]] = 1; grew = 1 }
      }
      for (i = 1; i <= NR; i++) if (pid[i] in in_tree) print pid[i], rss[i], cmd[i]
    }'
}
heavy_slot_tree() { ps -A -o pid=,ppid=,rss=,comm= | heavy_slot_tree_in "$@"; }
# every process as `<pid> <ppid> <KiB> <comm>`, the KiB being its physical footprint (macOS: proc_pid_rusage
# ri_phys_footprint; Linux: RssAnon + RssShmem + VmSwap) and its RSS where that cannot be read. RSS alone misses memory the
# kernel compressed: on 2026-10-28 a test filling 16 GiB with zeros showed 3 MiB of RSS until the machine stopped
# (the compressor full), and live processes here showed 36 MiB of RSS on a 1.8 GiB footprint. Without python3 it
# is the plain RSS listing (heavy_slot_watch notes that once).
heavy_slot_mem_snapshot() {
  local listing
  listing=$(ps -A -o pid=,ppid=,rss=,comm= 2>/dev/null) || return 1
  if command -v python3 >/dev/null 2>&1; then
    printf '%s\n' "$listing" | python3 -c '
import ctypes, ctypes.util, sys
fp = None
if sys.platform == "darwin":
    try:
        lib = ctypes.CDLL(ctypes.util.find_library("proc") or "/usr/lib/libproc.dylib")
        class RU(ctypes.Structure):
            _fields_ = [("uuid", ctypes.c_uint8 * 16)] + [("f%d" % i, ctypes.c_uint64) for i in range(18)]
        def fp(pid):
            r = RU()
            return r.f7 // 1024 if lib.proc_pid_rusage(pid, 2, ctypes.byref(r)) == 0 else None
    except OSError:
        fp = None
elif sys.platform.startswith("linux"):
    def fp(pid):
        try:
            kb = {}
            for line in open("/proc/%d/status" % pid):
                k, _, v = line.partition(":")
                if k in ("RssAnon", "RssShmem", "VmSwap"):
                    kb[k] = int(v.split()[0])
            # anonymous + shared memory, resident or swapped (zswap included): file pages left out, as the
            # macOS footprint leaves them out
            return sum(kb.values()) if "RssAnon" in kb else None
        except (OSError, ValueError):
            return None
for line in sys.stdin:
    p = line.split(None, 3)
    if len(p) < 3 or not p[0].isdigit():
        continue
    kib = fp(int(p[0])) if fp else None
    print(p[0], p[1], p[2] if kib is None else kib, p[3].rstrip("\n") if len(p) > 3 else "")
' && return 0
  fi
  printf '%s\n' "$listing"
}
# MiB of the process $2 and its descendants (leaving out $3 and its) in a heavy_slot_mem_snapshot listing on stdin
heavy_slot_tree_mib() {
  heavy_slot_tree_in "$1" "${2:-}" | awk '{ s += $2 } END { printf "%d", s / 1024 }'
}
# whether $1 runs (a zombie has ended: a watcher started by the process it watches outlives it until reaped)
heavy_slot_alive() {
  local st
  st=$(ps -o stat= -p "$1" 2>/dev/null) || return 1
  case "$st" in '' | *Z*) return 1 ;; esac
}
heavy_slot_watch() {
  local root=$1 cap="${ALICE_HEAVY_SLOT_RSS_CAP_MIB:-8192}" total_cap="${ALICE_HEAVY_SLOT_RSS_TOTAL_MIB:-20480}" \
    poll="${ALICE_HEAVY_SLOT_RSS_POLL:-1}" d self snap tree sum top f h hsum total largest largest_sum slots \
    largest_new largest_new_sum victim \
    peak=0 samples=0 noted=0
  d=$(heavy_slot_dir)
  self=$(exec sh -c 'echo "$PPID"')
  if kill -0 "$root" 2>/dev/null && ! heavy_slot_alive "$root"; then
    echo "[heavy-slot] note: ps does not show pid $root here, so the memory cap is off" >&2
    return 0
  fi
  if ! command -v python3 >/dev/null 2>&1; then
    echo "[heavy-slot] note: no python3 here, so the memory cap reads RSS (it does not see compressed memory)" >&2
  fi
  while heavy_slot_alive "$root"; do
    snap=$(heavy_slot_mem_snapshot 2>/dev/null) || snap=""
    tree=$(printf '%s\n' "$snap" | heavy_slot_tree_in "$root" "$self" 2>/dev/null) || tree=""
    if [ -z "$tree" ]; then
      if [ "$noted" = 0 ] && heavy_slot_alive "$root"; then
        echo "[heavy-slot] note: ps does not list pid $root here, so the memory cap is off" >&2
        noted=1
      fi
    else
      sum=$(printf '%s\n' "$snap" | heavy_slot_tree_mib "$root" "$self")
      samples=$((samples + 1))
      if [ "$sum" -gt "$peak" ]; then peak=$sum; fi
      if [ "$sum" -gt "$cap" ]; then
        top=$(printf '%s\n' "$tree" | sort -k2,2nr | head -1)
        # shellcheck disable=SC2046 # one pid per word
        kill -9 $(printf '%s\n' "$tree" | cut -d" " -f1) 2>/dev/null || true
        echo "[heavy-slot] RSS cap $(awk -v c="$cap" 'BEGIN { printf "%g", c / 1024 }')GiB exceeded by ${top%% *} $(printf '%s\n' "$top" | cut -d" " -f3-) (tree $sum MiB, physical footprint)" >&2
        return 137
      fi
      # every held slot's tree (by its holder), the total, the holder of the largest, and the largest of the
      # slots that have a watcher: `info` without a number is the one slot of a copy of this block from before
      # the slots were numbered (an older generated preflight), which has no watcher, so the watcher of the
      # largest numbered slot stops it when it is the largest
      total=0 largest="" largest_sum=-1 largest_new="" largest_new_sum=-1 slots=0
      for f in $(heavy_slot_live "$d" | awk '{ print "keeper:" $2 }') "$d/info"; do
        case "$f" in keeper:*) h=${f#keeper:} ;; *) h=$(sed -n 1p "$f" 2>/dev/null) || continue ;; esac
        [ -n "$h" ] && kill -0 "$h" 2>/dev/null || continue
        hsum=$(printf '%s\n' "$snap" | heavy_slot_tree_mib "$h")
        total=$((total + hsum)) slots=$((slots + 1))
        if [ "$hsum" -gt "$largest_sum" ]; then largest=$h largest_sum=$hsum; fi
        if [ "$f" != "$d/info" ] && [ "$hsum" -gt "$largest_new_sum" ]; then largest_new=$h largest_new_sum=$hsum; fi
      done
      if [ "$total" -gt "$total_cap" ] && [ -n "$largest" ]; then
        victim=""
        if [ "$largest" = "${ALICE_HEAVY_SLOT_HELD:-}" ]; then
          victim=$tree
        elif [ "$largest" = "$(sed -n 1p "$d/info" 2>/dev/null)" ] && [ "$largest_new" = "${ALICE_HEAVY_SLOT_HELD:-}" ]; then
          victim=$(printf '%s\n' "$snap" | heavy_slot_tree_in "$largest")
        fi
        if [ -n "$victim" ]; then
          top=$(printf '%s\n' "$victim" | sort -k2,2nr | head -1)
          # shellcheck disable=SC2046 # one pid per word
          kill -9 $(printf '%s\n' "$victim" | cut -d" " -f1) 2>/dev/null || true
          echo "[heavy-slot] RSS total cap $(awk -v c="$total_cap" 'BEGIN { printf "%g", c / 1024 }')GiB exceeded ($total MiB in $slots slots), killed the largest tree ${top%% *} $(printf '%s\n' "$top" | cut -d" " -f3-) ($largest_sum MiB)" >&2
          [ "$largest" = "${ALICE_HEAVY_SLOT_HELD:-}" ] && return 137
        fi
      fi
    fi
    sleep "$poll"
  done
  if [ "${ALICE_HEAVY_SLOT_RSS_REPORT:-}" = 1 ]; then echo "[heavy-slot] RSS peak $peak MiB over $samples samples" >&2; fi
  return 0
}
# --- heavy slot (end) ---
heavy_slot_acquire "$(basename "$PWD")" preflight || exit 1
gh_files=$(mktemp -d)
gh_local=0
[ -z "${GITHUB_ENV:-}" ] && gh_local=1
export GITHUB_OUTPUT="${GITHUB_OUTPUT:-$gh_files/output}" GITHUB_STEP_SUMMARY="${GITHUB_STEP_SUMMARY:-$gh_files/summary}"
export RUNNER_TEMP="${RUNNER_TEMP:-$gh_files/runner-temp}"
mkdir -p "$RUNNER_TEMP"
trap 'heavy_slot_release; rm -rf "$gh_files"' EXIT
# gh_step <job>: this job's $GITHUB_ENV / $GITHUB_PATH, with what its earlier steps wrote loaded
gh_step() {
  [ "$gh_local" = 1 ] || return 0
  export GITHUB_ENV="$gh_files/$1.env" GITHUB_PATH="$gh_files/$1.path"
  touch "$GITHUB_ENV" "$GITHUB_PATH"
  local line
  while IFS= read -r line || [ -n "$line" ]; do
    case "$line" in
      '') ;;
      *'<<'*) echo "preflight: a multi-line \$GITHUB_ENV value is not reproduced here: $line" >&2; return 1 ;;
      [A-Za-z_]*=*) export "${line%%=*}=${line#*=}" ;;
      *) echo "preflight: not a NAME=value line in \$GITHUB_ENV: $line" >&2; return 1 ;;
    esac
  done < "$GITHUB_ENV"
  while IFS= read -r line || [ -n "$line" ]; do [ -n "$line" ] && PATH="$line:$PATH"; done < "$GITHUB_PATH"
  export PATH
}
heavy_slot_cap_ok || exit 2
heavy_slot_watch "$$" >/dev/null &

quick=0
[[ "${1:-}" == "--quick" ]] && quick=1

step() { printf '\n\033[1;34m== %s\033[0m\n' "$*"; }
# `cargo clippy` reuses fresh `cargo check` artifacts and then lints nothing;
# touching the crate roots invalidates only this repo's fingerprints.
relint() { git ls-files | grep -E '(^|/)src/(lib|main)\.rs$' | xargs -r touch; }
need() { command -v "$1" >/dev/null 2>&1 || { echo "missing tool: $1 ($2)" >&2; exit 1; }; }
has_toolchain() { grep -q "^$1" <<<"$(rustup toolchain list)"; }

# Steps CI runs that this file cannot reproduce locally (they can only fail remotely):
#   - ci.yml:wasm:Ensure wasm32-wasip1 rust-std is installed (no cargo / grep)
#   - ci.yml:wasm: uses actions/cache@v6 (an action this file cannot run); Install wasmtime / Check wasmtime runs fetch the linux x86_64 tarball into $RUNNER_TEMP (runner-only, here wasmtime comes from PATH)
#   - ci.yml:browser:Ensure wasm32-unknown-unknown rust-std is installed (no cargo / grep)
#   - ci.yml:browser: uses actions/setup-node@v5 (an action this file cannot run)
#   - ci.yml:no-std:Ensure thumbv7em-none-eabihf rust-std is installed (no cargo / grep)
#   - ci.yml:oracle: uses actions/setup-python@v6 (an action this file cannot run)
#   - security-audit.yml:audit:Install cargo-audit (needs network / runner-only)
#   - security-audit.yml:deny:Install cargo-deny (needs network / runner-only)
#   - fuzz.yml:fuzz:Install cargo-fuzz [target=fuzz_utc_round_trip] (needs network / runner-only)
#   - fuzz.yml:fuzz:Run fuzz target [target=fuzz_utc_round_trip] (continue-on-error, runner-only (open-ended or unprovable-bounded fuzz run))
#   - fuzz.yml:fuzz:Report a crash [target=fuzz_utc_round_trip] (its `if:` runs on what happened earlier on the runner)
#   - fuzz.yml:fuzz:Install cargo-fuzz [target=fuzz_tai_to_utc] (needs network / runner-only)
#   - fuzz.yml:fuzz:Run fuzz target [target=fuzz_tai_to_utc] (continue-on-error, runner-only (open-ended or unprovable-bounded fuzz run))
#   - fuzz.yml:fuzz:Report a crash [target=fuzz_tai_to_utc] (its `if:` runs on what happened earlier on the runner)
#   - fuzz.yml:fuzz:Install cargo-fuzz [target=fuzz_tdb_and_jd] (needs network / runner-only)
#   - fuzz.yml:fuzz:Run fuzz target [target=fuzz_tdb_and_jd] (continue-on-error, runner-only (open-ended or unprovable-bounded fuzz run))
#   - fuzz.yml:fuzz:Report a crash [target=fuzz_tdb_and_jd] (its `if:` runs on what happened earlier on the runner)

need cargo-audit "cargo install cargo-audit --locked"
need cargo-deny "cargo install cargo-deny --locked"
need cargo-machete "cargo install cargo-machete --locked"
has_toolchain 1.85 || { echo "missing toolchain 1.85 (rustup toolchain install 1.85)" >&2; exit 1; }

step "ci.yml / wasm: Every integration test under wasmtime (debug, then release)"
(
  gh_step ci.yml.wasm
(
  export CARGO_TERM_COLOR="always"
  scripts/run_wasm_tests.sh
  scripts/run_wasm_tests.sh --release
)
)

step "ci.yml / browser: rustfmt (browser/)"
(
  gh_step ci.yml.browser
( export CARGO_TERM_COLOR="always"; cargo fmt --manifest-path browser/Cargo.toml -- --check )
)

step "ci.yml / browser: clippy (browser/, wasm32-unknown-unknown)"
(
  gh_step ci.yml.browser
relint
grep -q '^wasm32-unknown-unknown$' <<<"$(rustup target list --installed)" || rustup target add wasm32-unknown-unknown
( export CARGO_TERM_COLOR="always"; cargo clippy --locked --manifest-path browser/Cargo.toml --target wasm32-unknown-unknown -- -D warnings )
)

step "ci.yml / browser: Law id checks under node (debug, then release)"
(
  gh_step ci.yml.browser
( export CARGO_TERM_COLOR="always"; scripts/run_browser_wasm.sh )
)

step "ci.yml / browser: rustfmt (sky/)"
(
  gh_step ci.yml.browser
( export CARGO_TERM_COLOR="always"; cargo fmt --manifest-path sky/Cargo.toml -- --check )
)

step "ci.yml / browser: clippy (sky/, wasm32-unknown-unknown)"
(
  gh_step ci.yml.browser
relint
grep -q '^wasm32-unknown-unknown$' <<<"$(rustup target list --installed)" || rustup target add wasm32-unknown-unknown
( export CARGO_TERM_COLOR="always"; cargo clippy --locked --manifest-path sky/Cargo.toml --target wasm32-unknown-unknown -- -D warnings )
)

step "ci.yml / browser: sky module under node"
(
  gh_step ci.yml.browser
( export CARGO_TERM_COLOR="always"; scripts/run_sky_wasm.sh )
)

step "ci.yml / browser: wasm path scan tests"
(
  gh_step ci.yml.browser
( export CARGO_TERM_COLOR="always"; python3 scripts/test_wasm_path_scan.py )
)

step "ci.yml / browser: the star table's generator (tests)"
(
  gh_step ci.yml.browser
( export CARGO_TERM_COLOR="always"; python3 scripts/test_gen_star_table.py )
)

step "ci.yml / browser: Upstream previous identifiers (test, then whole-list uses only)"
(
  gh_step ci.yml.browser
(
  export CARGO_TERM_COLOR="always"
  python3 scripts/test_upstream_previous_ids_check.py
  python3 scripts/upstream_previous_ids_check.py
)
)

step "ci.yml / browser: Lock sources and agreement across workspaces (test, then check)"
(
  gh_step ci.yml.browser
(
  export CARGO_TERM_COLOR="always"
  python3 scripts/test_lock_sources_check.py
  python3 scripts/lock_sources_check.py
)
)

step "ci.yml / browser: sky module is the same built in 2 directories"
(
  gh_step ci.yml.browser
( export CARGO_TERM_COLOR="always"; scripts/sky_wasm_repro.sh )
)

step "ci.yml / clippy: rustfmt"
(
  gh_step ci.yml.clippy
( export CARGO_TERM_COLOR="always"; cargo fmt --all -- --check )
)

step "ci.yml / clippy: clippy (all targets)"
(
  gh_step ci.yml.clippy
relint
( export CARGO_TERM_COLOR="always"; cargo clippy --locked --all-targets -- -D warnings )
)

step "ci.yml / clippy: clippy (no_std lib)"
(
  gh_step ci.yml.clippy
relint
( export CARGO_TERM_COLOR="always"; cargo clippy --locked --lib --no-default-features -- -D warnings )
)

step "ci.yml / clippy: Doc (warnings are errors)"
(
  gh_step ci.yml.clippy
( export CARGO_TERM_COLOR="always"; RUSTDOCFLAGS="-Dwarnings" cargo doc --locked --lib --no-deps )
)

step "ci.yml / no-std: Build rlib for a target without std"
(
  gh_step ci.yml.no-std
grep -q '^thumbv7em-none-eabihf$' <<<"$(rustup target list --installed)" || rustup target add thumbv7em-none-eabihf
( export CARGO_TERM_COLOR="always"; cargo rustc --locked --lib --no-default-features --crate-type rlib --target thumbv7em-none-eabihf )
)

step "ci.yml / msrv: cargo check on MSRV"
(
  gh_step ci.yml.msrv
(
  export CARGO_TERM_COLOR="always"
  cargo +1.85 check --lib --locked
  cargo +1.85 check --lib --locked --no-default-features
)
)

step "ci.yml / oracle: Reference tables reproduce (src/fb1990_table.rs, tests/golden)"
(
  gh_step ci.yml.oracle
( export CARGO_TERM_COLOR="always"; scripts/oracle_check.sh )
)

step "security-audit.yml / deny: cargo deny (all features)"
(
  gh_step security-audit.yml.deny
( export CARGO_TERM_COLOR="always" CARGO_NET_RETRY="5"; cargo deny --all-features check all )
)

step "security-audit.yml / unused-deps: cargo machete"
(
  gh_step security-audit.yml.unused-deps
cargo machete
)

step "security-audit.yml / unused-deps: No cargo-machete ignore list in any manifest (tests, then check)"
(
  gh_step security-audit.yml.unused-deps
(
  export CARGO_TERM_COLOR="always" CARGO_NET_RETRY="5"
  python3 scripts/test_machete_metadata_check.py
  python3 scripts/machete_metadata_check.py .
)
)

step "security-audit.yml / stub-guard: Detect todo! / unimplemented! / panic!(STUB) / dbg!"
(
  gh_step security-audit.yml.stub-guard
(
  export CARGO_TERM_COLOR="always" CARGO_NET_RETRY="5"
  set -eo pipefail
  hits=$(grep -rnE 'todo!\(|unimplemented!\(|panic!\([^)]*STUB|dbg!\(' src/ --include="*.rs" || true)
  if [ -n "$hits" ]; then
    echo "stub or debug macro in src/:"
    echo "$hits"
    exit 1
  fi
  echo "no todo! / unimplemented! / STUB panic / dbg! in src/"
)
)

step "security-audit.yml / stub-guard: Detect platform libm / mul_add calls in src/"
(
  gh_step security-audit.yml.stub-guard
(
  export CARGO_TERM_COLOR="always" CARGO_NET_RETRY="5"
  set -eo pipefail
  n=$(find src -name '*.rs' | wc -l)
  if [ "$n" -eq 0 ]; then echo "scanned 0 files"; exit 1; fi
  hits=$(grep -rnE '\.(sin|cos|tan|asin|acos|atan|atan2|sin_cos|sinh|cosh|tanh|exp|exp2|ln|log|log2|log10|powf|powi|cbrt|hypot|sqrt|mul_add)\(' src/ --include="*.rs" \
    | grep -vE '^\S+:[0-9]+:\s*//' || true)
  if [ -n "$hits" ]; then
    echo "platform libm / mul_add call in src/:"
    echo "$hits"
    exit 1
  fi
  echo "scanned $n files: no platform libm / mul_add call"
)
)

step "fuzz.yml / fuzz: Build fuzz target [target=fuzz_utc_round_trip]"
(
  gh_step fuzz.yml.fuzz
if has_toolchain nightly && cargo +nightly fuzz --version >/dev/null 2>&1; then
  ( export CARGO_TERM_COLOR="always"; cargo +nightly fuzz build fuzz_utc_round_trip )
else
  echo 'PREFLIGHT-SKIP: fuzz.yml / fuzz: Build fuzz target [target=fuzz_utc_round_trip]: nightly / cargo-fuzz not installed' >&2
fi
)

step "fuzz.yml / fuzz: Build fuzz target [target=fuzz_tai_to_utc]"
(
  gh_step fuzz.yml.fuzz
if has_toolchain nightly && cargo +nightly fuzz --version >/dev/null 2>&1; then
  ( export CARGO_TERM_COLOR="always"; cargo +nightly fuzz build fuzz_tai_to_utc )
else
  echo 'PREFLIGHT-SKIP: fuzz.yml / fuzz: Build fuzz target [target=fuzz_tai_to_utc]: nightly / cargo-fuzz not installed' >&2
fi
)

step "fuzz.yml / fuzz: Build fuzz target [target=fuzz_tdb_and_jd]"
(
  gh_step fuzz.yml.fuzz
if has_toolchain nightly && cargo +nightly fuzz --version >/dev/null 2>&1; then
  ( export CARGO_TERM_COLOR="always"; cargo +nightly fuzz build fuzz_tdb_and_jd )
else
  echo 'PREFLIGHT-SKIP: fuzz.yml / fuzz: Build fuzz target [target=fuzz_tdb_and_jd]: nightly / cargo-fuzz not installed' >&2
fi
)

python3 scripts/test_check_eol.py
python3 scripts/check_eol.py
python3 scripts/test_public_text.py
scripts/public_text.sh
python3 scripts/test_mutants_equivalent_check.py
python3 scripts/mutants_equivalent_check.py --list-only

if [[ $quick -eq 1 ]]; then
  echo; echo "preflight --quick OK (test / bench suites skipped)"; exit 0
fi

step "ci.yml / test: Test (debug)"
(
  gh_step ci.yml.test
( export CARGO_TERM_COLOR="always"; cargo test --locked )
)

step "ci.yml / test: Test (release)"
(
  gh_step ci.yml.test
( export CARGO_TERM_COLOR="always"; cargo test --locked --release )
)

step "ci.yml / test: Test (no default features)"
(
  gh_step ci.yml.test
( export CARGO_TERM_COLOR="always"; cargo test --locked --no-default-features )
)

step "ci.yml / browser: sky native test"
(
  gh_step ci.yml.browser
( export CARGO_TERM_COLOR="always"; cargo test --locked --release --manifest-path sky/Cargo.toml )
)

step "security-audit.yml / audit: cargo audit"
(
  gh_step security-audit.yml.audit
( export CARGO_TERM_COLOR="always" CARGO_NET_RETRY="5"; cargo audit --deny yanked )
)

step "security-audit.yml / coverage: Coverage summary"
(
  gh_step security-audit.yml.coverage
(
  log="$(mktemp)"
  set +e
  {
    ( export CARGO_TERM_COLOR="always" CARGO_NET_RETRY="5"; cargo llvm-cov --summary-only )
  } 2>&1 | tee "${log}"
  rc=${PIPESTATUS[0]}
  n=$(sed $'s/\033\[[0-9;]*m//g' "${log}" | awk '$1 == "TOTAL" && $2 ~ /^[0-9]+$/ && $3 ~ /^[0-9]+$/ {n = $2 - $3} END {print n + 0}')
  rm -f "${log}"
  set -e
  if [ -z "${n}" ] || [ "${n}" -eq 0 ]; then echo 'security-audit.yml / coverage: Coverage summary: cargo-llvm-cov compared nothing (continue-on-error in CI; a run with no count fails)' >&2; exit 1; fi
  echo 'PREFLIGHT-INFO: security-audit.yml / coverage: Coverage summary: continue-on-error in CI, '"exit ${rc}, ${n} compared" >&2
)
)

echo; echo "preflight OK"
