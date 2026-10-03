#!/usr/bin/env bash
# Live smoke for guestkit.netpolicy.* / guestkit.lsm.* (run as root on Linux).
# Runs guestkitd on a private socket and polices only a scratch cgroup whose
# processes talk over a veth pair into a scratch netns; nothing else on the
# host is attached to or enforced on.
#
#   sudo GK_BIN=target/debug scripts/ebpf-policy-smoke.sh
set -uo pipefail

BIN=${GK_BIN:-target/debug}
SOCK=/tmp/gks-$$.sock
POL=/tmp/gks-policy-$$.yaml
CG=/sys/fs/cgroup/gk-smoke-$$
CG2=/sys/fs/cgroup/gk-smoke-out-$$
NS=gks-$$
H=gksh$$
P=gksp$$
PASS=0
FAIL=0
PIDS=()

ok() { if [ "$1" = 0 ]; then PASS=$((PASS + 1)); echo "  ok   $2"; else FAIL=$((FAIL + 1)); echo "  FAIL $2"; fi; }
call() {
    local p=${2:-}
    [ -n "$p" ] || p='{}'
    "$BIN/guestkitctl" --socket "$SOCK" --json call "$1" --params "$p" 2>&1
}
jget() { python3 -c "import json,sys; d=json.load(sys.stdin); print(eval(sys.argv[1], {}, {'d': d}))" "$1" 2>/dev/null; }
in_cg() { local cg=$1; shift; sh -c "echo \$\$ > $cg/cgroup.procs; exec ip netns exec $NS $*"; }
fetch() { in_cg "$1" curl -s -o /dev/null -m 2 -w '%{http_code}' "http://$NET.1:$2/"; }

cleanup() {
    for p in "${PIDS[@]}"; do kill "$p" 2>/dev/null; done
    sleep 0.5
    for c in "$CG" "$CG2"; do
        [ -d "$c" ] && { cat "$c/cgroup.procs" 2>/dev/null | xargs -r kill 2>/dev/null; sleep 0.2; rmdir "$c" 2>/dev/null; }
    done
    ip link del "$H" 2>/dev/null
    ip netns del "$NS" 2>/dev/null
    rm -f "$SOCK" "$POL"
}
trap cleanup EXIT

NET="10.250.$(( $$ % 200 ))"
echo "== setup (netns $NS, cgroups $CG $CG2)"
ip netns add "$NS"
ip link add "$H" type veth peer name "$P"
ip link set "$P" netns "$NS"
ip addr add "$NET.1/24" dev "$H"; ip link set "$H" up
ip -n "$NS" addr add "$NET.2/24" dev "$P"; ip -n "$NS" link set "$P" up; ip -n "$NS" link set lo up
mkdir -p "$CG" "$CG2"
for port in 18080 18081; do
    python3 -m http.server --bind "$NET.1" "$port" >/dev/null 2>&1 &
    PIDS+=($!)
done
printf 'capabilities:\n  ebpf: false\n' > "$POL"
tail -f /dev/null | GUESTKIT_LOCAL_SOCKET="$SOCK" ZYVOR_AGENT_POLICY="$POL" RUST_LOG=warn "$BIN/guestkitd" --channel stdio &
GKPID=$!
PIDS+=($GKPID)
for _ in $(seq 50); do [ -S "$SOCK" ] && break; sleep 0.2; done
sleep 0.5
[ "$(fetch "$CG" 18080)" = 200 ] && [ "$(fetch "$CG" 18081)" = 200 ]
ok $? "baseline: both ports reachable before any policy"

echo "== policy gate"
out=$(call guestkit.netpolicy.status)
echo "$out" | grep -q "ebpf disabled by local policy"; ok $? "status denied while capabilities.ebpf=false"
printf 'capabilities:\n  ebpf: true\n' > "$POL"
out=$(call guestkit.netpolicy.status)
[ "$(echo "$out" | jget "d['available']")" = True ]; ok $? "status available once ebpf=true"
[ "$(echo "$out" | jget "len(d['targets'])")" = 0 ]; ok $? "no targets at rest"

echo "== netpolicy audit"
RULE="{\"direction\":\"egress\",\"cidr\":\"$NET.1/32\",\"proto\":\"tcp\",\"port\":18080}"
out=$(call guestkit.netpolicy.apply "{\"cgroup\":\"$CG\",\"rules\":[$RULE]}")
[ "$(echo "$out" | jget "d['mode']")" = audit ]; ok $? "apply defaults to audit"
[ "$(echo "$out" | jget "d['attached']")" = True ]; ok $? "programs attached to the scratch cgroup"
[ "$(fetch "$CG" 18081)" = 200 ]; ok $? "audit: non-matching port still reachable"
sleep 0.5
out=$(call guestkit.netpolicy.status "{\"cgroup\":\"$CG\"}")
[ "$(echo "$out" | jget "d['targets'][0]['counters']['audited'] > 0")" = True ]; ok $? "audit counter incremented"
[ "$(echo "$out" | jget "any(e['port']==18081 and not e['denied'] for e in d['events'])")" = True ]; ok $? "audit event for :18081 (not denied)"

echo "== netpolicy enforce (lease)"
out=$(call guestkit.netpolicy.apply "{\"cgroup\":\"$CG\",\"mode\":\"enforce\",\"rules\":[$RULE]}")
echo "$out" | grep -q "enforce needs lease_secs"; ok $? "enforce without lease refused"
out=$(call guestkit.netpolicy.apply "{\"cgroup\":\"$CG\",\"mode\":\"enforce\",\"lease_secs\":4000,\"rules\":[$RULE]}")
echo "$out" | grep -q "lease_secs must be"; ok $? "lease above 3600 refused"
out=$(call guestkit.netpolicy.apply "{\"cgroup\":\"$CG\",\"mode\":\"enforce\",\"lease_secs\":3,\"rules\":[$RULE]}")
[ "$(echo "$out" | jget "d['mode']")" = enforce ]; ok $? "enforce armed with 3s lease"
[ "$(fetch "$CG" 18080)" = 200 ]; ok $? "enforce: allowed peer:port reachable"
[ "$(fetch "$CG" 18081)" != 200 ]; ok $? "enforce: non-matching port blocked"
[ "$(fetch "$CG2" 18081)" = 200 ]; ok $? "enforce is scoped: other cgroup unaffected"
sleep 3.5
[ "$(fetch "$CG" 18081)" = 200 ]; ok $? "lease expired in-kernel: traffic flows again"
out=$(call guestkit.netpolicy.status "{\"cgroup\":\"$CG\"}")
[ "$(echo "$out" | jget "(d['targets'][0]['mode'], d['targets'][0]['lease_expired'])")" = "('audit', True)" ]
ok $? "status reports audit + lease_expired"
[ "$(echo "$out" | jget "d['targets'][0]['counters']['denied'] > 0")" = True ]; ok $? "denied counter incremented"

echo "== netpolicy ingress"
sh -c "echo \$\$ > $CG/cgroup.procs; exec ip netns exec $NS python3 -m http.server --bind $NET.2 18082" >/dev/null 2>&1 &
PIDS+=($!)
sleep 1
[ "$(curl -s -o /dev/null -m 2 -w '%{http_code}' "http://$NET.2:18082/")" = 200 ]; ok $? "ingress baseline reachable"
call guestkit.netpolicy.apply "{\"cgroup\":\"$CG\",\"mode\":\"enforce\",\"lease_secs\":30,\"egress\":false,\"ingress\":true}" >/dev/null
[ "$(curl -s -o /dev/null -m 2 -w '%{http_code}' "http://$NET.2:18082/")" != 200 ]; ok $? "ingress-isolated container drops unknown peer"
call guestkit.netpolicy.apply "{\"cgroup\":\"$CG\",\"mode\":\"enforce\",\"lease_secs\":30,\"egress\":false,\"ingress\":true,\"rules\":[{\"direction\":\"ingress\",\"cidr\":\"$NET.0/24\"}]}" >/dev/null
[ "$(curl -s -o /dev/null -m 2 -w '%{http_code}' "http://$NET.2:18082/")" = 200 ]; ok $? "ingress allow rule admits the peer subnet"

echo "== netpolicy off"
out=$(call guestkit.netpolicy.apply "{\"cgroup\":\"$CG\",\"mode\":\"off\"}")
[ "$(echo "$out" | jget "d['removed']")" = True ]; ok $? "off removes the policy"
if command -v bpftool >/dev/null; then
    [ -z "$(bpftool cgroup show "$CG" 2>/dev/null | grep gk_np)" ]; ok $? "cgroup programs detached"
fi
out=$(call guestkit.netpolicy.apply '{"container":"no-such-container-gks"}')
echo "$out" | grep -q "not found or not running"; ok $? "unknown container rejected"
out=$(call guestkit.netpolicy.apply "{\"cgroup\":\"/sys/fs/cgroup/../etc\"}")
echo "$out" | grep -q "not allowed"; ok $? "cgroup path traversal rejected"

echo "== lsm"
out=$(call guestkit.lsm.status)
active=$(echo "$out" | jget "d['lsm_active']")
[ -n "$active" ]; ok $? "lsm.status reports lsm_active=$active"
out=$(call guestkit.lsm.apply "{\"cgroup\":\"$CG\"}")
echo "$out" | grep -q "enable at least one"; ok $? "empty MAC policy refused"
out=$(call guestkit.lsm.apply "{\"cgroup\":\"$CG\",\"deny_exec\":true,\"allow_exec\":[\"/usr/bin/true\"],\"deny_wx\":true,\"restrict_devices\":true,\"restrict_writes\":true,\"writable_paths\":[\"/tmp\"]}")
[ "$(echo "$out" | jget "d['mode']")" = audit ]; ok $? "audit MAC policy applied (hooks pass the verifier)"
out=$(call guestkit.lsm.status)
[ "$(echo "$out" | jget "d['hooks_attached']")" = True ]; ok $? "LSM hooks attached"
if [ "$active" = True ]; then
    in_cg "$CG" /bin/ls / >/dev/null 2>&1
    sleep 0.5
    out=$(call guestkit.lsm.status)
    [ "$(echo "$out" | jget "any(e['action']=='exec' for e in d['events'])")" = True ]; ok $? "exec outside allowlist audited"
else
    echo "$out" | grep -q "lsm_inactive"; ok $? "lsm_inactive surfaced in status"
    out=$(call guestkit.lsm.apply "{\"cgroup\":\"$CG\",\"mode\":\"enforce\",\"lease_secs\":10,\"deny_wx\":true}")
    echo "$out" | grep -q "lsm_inactive: refusing to enforce"; ok $? "enforce refused while BPF-LSM inactive"
fi
out=$(call guestkit.lsm.apply "{\"cgroup\":\"$CG\",\"mode\":\"off\"}")
[ "$(echo "$out" | jget "d['removed']")" = True ]; ok $? "lsm off removes the policy"
out=$(call guestkit.lsm.status)
[ "$(echo "$out" | jget "d['hooks_attached']")" = False ]; ok $? "LSM hooks detached with no policies left"

echo "== agent exit"
call guestkit.netpolicy.apply "{\"cgroup\":\"$CG\",\"rules\":[$RULE]}" >/dev/null
kill "$GKPID" 2>/dev/null
sleep 1
if command -v bpftool >/dev/null; then
    [ -z "$(bpftool cgroup show "$CG" 2>/dev/null | grep gk_np)" ]; ok $? "programs gone after guestkitd exits (nothing persisted)"
fi

echo
echo "passed $PASS, failed $FAIL"
[ "$FAIL" = 0 ]
