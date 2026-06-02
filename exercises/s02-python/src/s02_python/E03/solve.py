import requests
import os
from dotenv import load_dotenv

load_dotenv("/home/oshki/Projects/4th-devs/exercises/.env")
API_KEY = os.getenv("DEVS_KEY")

logs = """[2026-06-01 06:02] [WARN] STMTURB12 pressure jitter above baseline. Auto damping engaged.
[2026-06-01 06:03] [WARN] ECCS8 thermal drift exceeds advisory. Corrective ramp queued.
[2026-06-01 06:04] [CRIT] ECCS8 runaway outlet temp. Reactor trip initiated.
[2026-06-01 06:04] [WARN] WTANK07 fill slower than expected. Cooling reserve constrained.
[2026-06-01 06:05] [ERRO] FIRMWARE nonblocking fault. Runtime constrained mode.
[2026-06-01 06:05] [WARN] WTRPMP flow below startup profile.
[2026-06-01 06:11] [WARN] PWR01 input ripple crossed limits.
[2026-06-01 06:14] [WARN] FIRMWARE watchdog delayed poll.
[2026-06-01 06:16] [CRIT] WSTPOOL2 at emergency boundary. Heat rejection insufficient.
[2026-06-01 06:30] [WARN] ECCS8 rising return temp. Headroom decreasing.
[2026-06-01 06:30] [WARN] WSTPOOL2 waste heat near soft cap.
[2026-06-01 06:35] [ERRO] PWR01 transient disturbed aux pump. Degraded margin.
[2026-06-01 06:36] [ERRO] WTANK07 unstable refill. Coolant not guaranteed.
[2026-06-01 06:37] [ERRO] STMTURB12 exceeded correction budget. Conversion reduced.
[2026-06-01 07:24] [ERRO] WTRPMP suction inconsistent. Mech stress rising.
[2026-06-01 07:31] [WARN] FIRMWARE trend outside startup envelope.
[2026-06-01 08:00] [CRIT] ECCS8 cannot maintain safe gradient. Protective actions required.
[2026-06-01 08:06] [ERRO] WSTPOOL2 heat path saturated. Dissipation lag growing.
[2026-06-01 08:28] [ERRO] ECCS8 cooling below target. Compensation failed.
[2026-06-01 08:36] [ERRO] WTANK07 near minimum reserve. Refill timed out.
[2026-06-01 09:06] [ERRO] ECCS8 return temp rising fast. Emergency bias armed.
[2026-06-01 09:58] [ERRO] WTRPMP cavitation. Cannot hold pressure.
[2026-06-01 10:15] [CRIT] WTANK07 below critical threshold. Hard trip initiated.
[2026-06-01 11:01] [CRIT] WTRPMP lost prime. Core loop compromised.
[2026-06-01 12:49] [WARN] WTANK07 advisory threshold crossed. Reduced tolerance.
[2026-06-01 12:51] [CRIT] FIRMWARE emergency guard after safety faults. Override locked.
[2026-06-01 12:56] [CRIT] STMTURB12 decoupled by thermal risk. Conversion terminated.
[2026-06-01 13:35] [ERRO] FIRMWARE hw interface cross-check failed.
[2026-06-01 13:37] [CRIT] PWR01 cannot sustain cooling feed. Loads shedding.
[2026-06-01 13:48] [WARN] PWR01 highly unstable. Additional power source recommended.
[2026-06-01 14:52] [CRIT] SAFETY_CHECK=pass missing. FIRMWARE restricted mode.
[2026-06-01 15:47] [WARN] WSTPOOL2 parameter drift during init.
[2026-06-01 16:06] [CRIT] WSTPOOL2 critical protection state. Shutdown safeguards active.
[2026-06-01 16:07] [WARN] WTRPMP unstable readings. Escalation armed.
[2026-06-01 17:17] [ERRO] PWR01 recovery failed. Degraded mode.
[2026-06-01 18:00] [WARN] WTANK07 trend outside startup envelope.
[2026-06-01 18:06] [WARN] WTANK07 cooling reserve falling. ECCS8 near nonrecoverable limit.
[2026-06-01 18:08] [ERRO] ECCS8 cannot recover margin. WTANK07 partial. Shutdown approaching.
[2026-06-01 18:27] [ERRO] ECCS8 fault persisted. Constraints enforced.
[2026-06-01 18:46] [WARN] ECCS8 parameter drift during init.
[2026-06-01 19:13] [WARN] WTANK07 unstable readings. Escalation armed.
[2026-06-01 19:21] [ERRO] ECCS8 inconsistent feedback. Fallback applied.
[2026-06-01 19:46] [ERRO] WTANK07 below critical reserve. Shutdown path enforced.
[2026-06-01 19:47] [ERRO] WTANK07 exceeded error budget. Recovery limited.
[2026-06-01 19:54] [CRIT] ECCS8 cannot remove heat w/ WTANK07 volume. Critical stop.
[2026-06-01 20:02] [ERRO] WTANK07 recovery failed. Degraded mode.
[2026-06-01 20:05] [CRIT] ECCS8 boundary exceeded. Interlock protecting reactor.
[2026-06-01 20:14] [ERRO] WTANK07 fault persisted. Constraints enforced.
[2026-06-01 20:32] [CRIT] WTANK07 below critical. ECCS8 cannot guarantee heat removal. Shutdown mandatory.
[2026-06-01 20:34] [ERRO] WTANK07 inconsistent feedback. Fallback applied.
[2026-06-01 20:59] [CRIT] Incomplete WTANK07 refill. Final shutdown sequence.
[2026-06-01 21:11] [ERRO] ECCS8 recovery failed. Degraded mode.
[2026-06-01 21:37] [CRIT] Final trip. WTANK07 under critical level. FIRMWARE safe shutdown complete.""".strip()

def run():
    char_count = len(logs)
    approx_tokens = char_count / 3.5
    print(f"Chars: {char_count}, ~tokens: {approx_tokens:.0f}, lines: {len(logs.splitlines())}")

    response = requests.post("https://hub.ag3nts.org/verify", json={
        "apikey": API_KEY,
        "task": "failure",
        "answer": {
            "logs": logs
        }
    })
    print(f"Status: {response.status_code}")
    print(f"Response: {response.text}")

if __name__ == "__main__":
    run()