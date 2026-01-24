window.BENCHMARK_DATA = {
  "lastUpdate": 1769276548115,
  "repoUrl": "https://github.com/postrv/ralphing-la-vida-locum",
  "entries": {
    "Ralph Performance Benchmarks": [
      {
        "commit": {
          "author": {
            "email": "71533421+postrv@users.noreply.github.com",
            "name": "postrv",
            "username": "postrv"
          },
          "committer": {
            "email": "71533421+postrv@users.noreply.github.com",
            "name": "postrv",
            "username": "postrv"
          },
          "distinct": true,
          "id": "164c1437dc06aa4cd6b0c5f12c8e39eed2067c06",
          "message": "docs: Archive Sprint 25, streamline plan, apply rustfmt\n\n- Archive Sprint 25 (Analytics Dashboard) to COMPLETED_SPRINTS.md\n- Streamline IMPLEMENTATION_PLAN.md to only show Sprint 26.5 remaining\n- Update test count to 1,918 passing\n- Apply rustfmt formatting to recent Phase 26 code\n- Clear stale session state (43 orphaned tasks)\n\nCo-Authored-By: Claude Opus 4.5 <noreply@anthropic.com>",
          "timestamp": "2026-01-24T17:36:39Z",
          "tree_id": "329c1f65a4bc997e6a2d4c8c0d420af4d38b53d9",
          "url": "https://github.com/postrv/ralphing-la-vida-locum/commit/164c1437dc06aa4cd6b0c5f12c8e39eed2067c06"
        },
        "date": 1769276547828,
        "tool": "customSmallerIsBetter",
        "benches": [
          {
            "name": "parallel_vs_sequential/parallel",
            "value": 247192.1480878223,
            "unit": "ns"
          },
          {
            "name": "parallel_vs_sequential/parallel",
            "value": 227981.53755437478,
            "unit": "ns"
          },
          {
            "name": "parallel_vs_sequential/parallel",
            "value": 238714.3797924189,
            "unit": "ns"
          },
          {
            "name": "parallel_vs_sequential/sequential",
            "value": 1495263.905700009,
            "unit": "ns"
          },
          {
            "name": "parallel_vs_sequential/sequential",
            "value": 371601.9194726754,
            "unit": "ns"
          },
          {
            "name": "parallel_vs_sequential/sequential",
            "value": 744269.8336454097,
            "unit": "ns"
          },
          {
            "name": "polyglot_detection/polyglot_project",
            "value": 428659.47962620953,
            "unit": "ns"
          },
          {
            "name": "polyglot_detection/single_language",
            "value": 261528.0938137268,
            "unit": "ns"
          },
          {
            "name": "gate_execution/no_allow_gate",
            "value": 743523.6204431269,
            "unit": "ns"
          },
          {
            "name": "gate_execution/no_allow_gate",
            "value": 385734.4989871694,
            "unit": "ns"
          },
          {
            "name": "gate_execution/no_allow_gate",
            "value": 92519.80136506008,
            "unit": "ns"
          },
          {
            "name": "language_detection/detect",
            "value": 863626.9800081514,
            "unit": "ns"
          },
          {
            "name": "language_detection/detect",
            "value": 571527.615163051,
            "unit": "ns"
          },
          {
            "name": "language_detection/detect",
            "value": 431139.14811623486,
            "unit": "ns"
          },
          {
            "name": "language_detection/detect",
            "value": 323878.46078723937,
            "unit": "ns"
          },
          {
            "name": "prompt_building/plan",
            "value": 10500.35639905942,
            "unit": "ns"
          },
          {
            "name": "prompt_building/build",
            "value": 25974.23981801429,
            "unit": "ns"
          },
          {
            "name": "prompt_building/debug",
            "value": 12913.695962514845,
            "unit": "ns"
          },
          {
            "name": "context_building/full_context",
            "value": 3656.0057208210987,
            "unit": "ns"
          },
          {
            "name": "context_building/minimal_context",
            "value": 581.7527845729238,
            "unit": "ns"
          },
          {
            "name": "context_building/typical_context",
            "value": 819.7754793972738,
            "unit": "ns"
          }
        ]
      }
    ]
  }
}