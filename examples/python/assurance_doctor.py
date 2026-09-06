#!/usr/bin/env python3
# Copyright 2026 Zyvor AI Labs · https://zyvor.dev
# SPDX-License-Identifier: Apache-2.0
"""Example: GuestKit assurance APIs (v1.1.0+).

Usage:
  python assurance_doctor.py /path/to/disk.qcow2
  python assurance_doctor.py /path/to/disk.qcow2 --repair --apply
"""
from __future__ import annotations

import argparse
import json
import sys

import guestkit


def main() -> int:
    parser = argparse.ArgumentParser(description="GuestKit assurance doctor + optional repair")
    parser.add_argument("image", help="Path to disk image (.qcow2, .vmdk, …)")
    parser.add_argument("--target", default="kvm", help="Migration target (default: kvm)")
    parser.add_argument("--repair", action="store_true", help="Run migrate-repair instead of doctor only")
    parser.add_argument("--apply", action="store_true", help="Apply repairs (default: dry-run)")
    parser.add_argument("--explain", action="store_true", help="Include root-cause explanation in doctor output")
    args = parser.parse_args()

    if args.repair:
        result = guestkit.run_migrate_repair(
            args.image,
            target=args.target,
            apply=args.apply,
            verbose=True,
        )
        print(json.dumps(result, indent=2, default=str))
        if args.apply and not result.get("applied"):
            return 1
        return 0

    report = guestkit.run_doctor(args.image, target=args.target, explain=args.explain)
    print(json.dumps(report, indent=2, default=str))
    score = report.get("bootability", {}).get("score", 0)
    if score < 80:
        print(f"WARN: bootability score {score} < 80", file=sys.stderr)
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
