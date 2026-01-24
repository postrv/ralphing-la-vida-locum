window.BENCHMARK_DATA = {
  "lastUpdate": 1769279280661,
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
      },
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
          "id": "f2181e17fc7b253e8f18f3cb3665f6efbdd35bb2",
          "message": "fix: Remove unused affects_file methods to fix release build\n\nThe affects_file and affects_any_file methods were only used in\ndebug_assertions blocks which are stripped in release builds,\ncausing dead_code errors. Removed the unused methods, their tests,\nand the debug tracing block.\n\nThe has_explicit_affected_file_match method (which IS used) is\nretained for scoped task selection.\n\nCo-Authored-By: Claude Opus 4.5 <noreply@anthropic.com>",
          "timestamp": "2026-01-24T17:50:53Z",
          "tree_id": "18bbb557241d8b37867059f61c45e0435a68b763",
          "url": "https://github.com/postrv/ralphing-la-vida-locum/commit/f2181e17fc7b253e8f18f3cb3665f6efbdd35bb2"
        },
        "date": 1769277401634,
        "tool": "customSmallerIsBetter",
        "benches": [
          {
            "name": "parallel_vs_sequential/parallel",
            "value": 241568.5958518269,
            "unit": "ns"
          },
          {
            "name": "parallel_vs_sequential/parallel",
            "value": 222193.33592415578,
            "unit": "ns"
          },
          {
            "name": "parallel_vs_sequential/parallel",
            "value": 233059.2940211754,
            "unit": "ns"
          },
          {
            "name": "parallel_vs_sequential/sequential",
            "value": 1481814.2650081173,
            "unit": "ns"
          },
          {
            "name": "parallel_vs_sequential/sequential",
            "value": 370532.8466402389,
            "unit": "ns"
          },
          {
            "name": "parallel_vs_sequential/sequential",
            "value": 740347.6232822884,
            "unit": "ns"
          },
          {
            "name": "polyglot_detection/polyglot_project",
            "value": 435726.0261296545,
            "unit": "ns"
          },
          {
            "name": "polyglot_detection/single_language",
            "value": 260047.01299187046,
            "unit": "ns"
          },
          {
            "name": "gate_execution/no_allow_gate",
            "value": 745164.5117836013,
            "unit": "ns"
          },
          {
            "name": "gate_execution/no_allow_gate",
            "value": 382112.07379445696,
            "unit": "ns"
          },
          {
            "name": "gate_execution/no_allow_gate",
            "value": 91814.86265960636,
            "unit": "ns"
          },
          {
            "name": "language_detection/detect",
            "value": 869384.7779931747,
            "unit": "ns"
          },
          {
            "name": "language_detection/detect",
            "value": 576874.8538851226,
            "unit": "ns"
          },
          {
            "name": "language_detection/detect",
            "value": 437360.02525160386,
            "unit": "ns"
          },
          {
            "name": "language_detection/detect",
            "value": 321530.9809622829,
            "unit": "ns"
          },
          {
            "name": "prompt_building/plan",
            "value": 10564.77784544567,
            "unit": "ns"
          },
          {
            "name": "prompt_building/build",
            "value": 26190.712491554143,
            "unit": "ns"
          },
          {
            "name": "prompt_building/debug",
            "value": 12905.958962826217,
            "unit": "ns"
          },
          {
            "name": "context_building/full_context",
            "value": 3725.067484949437,
            "unit": "ns"
          },
          {
            "name": "context_building/minimal_context",
            "value": 527.6819754849089,
            "unit": "ns"
          },
          {
            "name": "context_building/typical_context",
            "value": 818.312378826902,
            "unit": "ns"
          }
        ]
      },
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
          "id": "81b108de07e2b240eea39334ede983884e32eaf9",
          "message": "feat(cli): Add --files and --changed flags for incremental execution (Phase 26.5)\n\nAdd two new incremental execution flags to `ralph loop`:\n- `--files <glob>`: Process only files matching a glob pattern\n- `--changed`: Shorthand for `--changed-since HEAD~1`\n\nThe three incremental flags (--changed-since, --files, --changed) are\nmutually exclusive and will error if more than one is specified.\n\nThis completes Sprint 26: Incremental Execution Mode.\n\nCo-Authored-By: Claude Opus 4.5 <noreply@anthropic.com>",
          "timestamp": "2026-01-24T18:05:01Z",
          "tree_id": "500b6f70214fc5f40a9157107bf6e5f9902883eb",
          "url": "https://github.com/postrv/ralphing-la-vida-locum/commit/81b108de07e2b240eea39334ede983884e32eaf9"
        },
        "date": 1769278286562,
        "tool": "customSmallerIsBetter",
        "benches": [
          {
            "name": "parallel_vs_sequential/parallel",
            "value": 249614.69204959323,
            "unit": "ns"
          },
          {
            "name": "parallel_vs_sequential/parallel",
            "value": 231369.5994292384,
            "unit": "ns"
          },
          {
            "name": "parallel_vs_sequential/parallel",
            "value": 239873.55308723214,
            "unit": "ns"
          },
          {
            "name": "parallel_vs_sequential/sequential",
            "value": 1504570.7183316662,
            "unit": "ns"
          },
          {
            "name": "parallel_vs_sequential/sequential",
            "value": 374177.25895447354,
            "unit": "ns"
          },
          {
            "name": "parallel_vs_sequential/sequential",
            "value": 747890.2740183347,
            "unit": "ns"
          },
          {
            "name": "polyglot_detection/polyglot_project",
            "value": 429189.93818114087,
            "unit": "ns"
          },
          {
            "name": "polyglot_detection/single_language",
            "value": 259948.61943777095,
            "unit": "ns"
          },
          {
            "name": "gate_execution/no_allow_gate",
            "value": 744712.7177949592,
            "unit": "ns"
          },
          {
            "name": "gate_execution/no_allow_gate",
            "value": 385005.8226037228,
            "unit": "ns"
          },
          {
            "name": "gate_execution/no_allow_gate",
            "value": 92049.02702724708,
            "unit": "ns"
          },
          {
            "name": "language_detection/detect",
            "value": 859520.780497558,
            "unit": "ns"
          },
          {
            "name": "language_detection/detect",
            "value": 569991.8710825734,
            "unit": "ns"
          },
          {
            "name": "language_detection/detect",
            "value": 428067.2216401496,
            "unit": "ns"
          },
          {
            "name": "language_detection/detect",
            "value": 323205.365039121,
            "unit": "ns"
          },
          {
            "name": "prompt_building/plan",
            "value": 10743.622818089638,
            "unit": "ns"
          },
          {
            "name": "prompt_building/build",
            "value": 28177.727802993475,
            "unit": "ns"
          },
          {
            "name": "prompt_building/debug",
            "value": 12961.055484879382,
            "unit": "ns"
          },
          {
            "name": "context_building/full_context",
            "value": 3475.43871112835,
            "unit": "ns"
          },
          {
            "name": "context_building/minimal_context",
            "value": 525.4314185675987,
            "unit": "ns"
          },
          {
            "name": "context_building/typical_context",
            "value": 750.719409651269,
            "unit": "ns"
          }
        ]
      },
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
          "id": "96b4cf44dc7e19ee73398aa488dbec8c17669819",
          "message": "feat(cli): Wire --model variant and add --no-fallback flag (Phase 23.5)\n\n- Add model field to RealClaudeProcess with configurable variant\n- Add with_model() constructor and model() getter\n- Wire Claude variant from LlmConfig through LoopManager to RealClaudeProcess\n- Add --no-fallback CLI flag (currently no-op until ProviderRouter integration)\n- Add 4 new tests for RealClaudeProcess model configuration\n\nThe --model CLI flag now properly uses the configured variant (opus/sonnet/haiku)\nwhen running Claude Code iterations. Previously the model was hardcoded.\n\nCo-Authored-By: Claude Opus 4.5 <noreply@anthropic.com>",
          "timestamp": "2026-01-24T18:21:43Z",
          "tree_id": "f3139d64a3251a58cc695d9c679a84405474ac1f",
          "url": "https://github.com/postrv/ralphing-la-vida-locum/commit/96b4cf44dc7e19ee73398aa488dbec8c17669819"
        },
        "date": 1769279279850,
        "tool": "customSmallerIsBetter",
        "benches": [
          {
            "name": "parallel_vs_sequential/parallel",
            "value": 241086.03045561645,
            "unit": "ns"
          },
          {
            "name": "parallel_vs_sequential/parallel",
            "value": 218038.27495548784,
            "unit": "ns"
          },
          {
            "name": "parallel_vs_sequential/parallel",
            "value": 232627.5800093496,
            "unit": "ns"
          },
          {
            "name": "parallel_vs_sequential/sequential",
            "value": 1480436.3588984257,
            "unit": "ns"
          },
          {
            "name": "parallel_vs_sequential/sequential",
            "value": 369745.0861563401,
            "unit": "ns"
          },
          {
            "name": "parallel_vs_sequential/sequential",
            "value": 740501.8759564823,
            "unit": "ns"
          },
          {
            "name": "polyglot_detection/polyglot_project",
            "value": 436394.1852119605,
            "unit": "ns"
          },
          {
            "name": "polyglot_detection/single_language",
            "value": 263670.4412584014,
            "unit": "ns"
          },
          {
            "name": "gate_execution/no_allow_gate",
            "value": 755553.9164170683,
            "unit": "ns"
          },
          {
            "name": "gate_execution/no_allow_gate",
            "value": 387087.50667529093,
            "unit": "ns"
          },
          {
            "name": "gate_execution/no_allow_gate",
            "value": 93226.82990729013,
            "unit": "ns"
          },
          {
            "name": "language_detection/detect",
            "value": 877774.9099950173,
            "unit": "ns"
          },
          {
            "name": "language_detection/detect",
            "value": 572321.1571785788,
            "unit": "ns"
          },
          {
            "name": "language_detection/detect",
            "value": 434109.6286710282,
            "unit": "ns"
          },
          {
            "name": "language_detection/detect",
            "value": 324746.7499715556,
            "unit": "ns"
          },
          {
            "name": "prompt_building/plan",
            "value": 10760.246978316396,
            "unit": "ns"
          },
          {
            "name": "prompt_building/build",
            "value": 26171.133165410185,
            "unit": "ns"
          },
          {
            "name": "prompt_building/debug",
            "value": 13283.785995594066,
            "unit": "ns"
          },
          {
            "name": "context_building/full_context",
            "value": 3596.8318209819345,
            "unit": "ns"
          },
          {
            "name": "context_building/minimal_context",
            "value": 549.4448612895637,
            "unit": "ns"
          },
          {
            "name": "context_building/typical_context",
            "value": 823.0600948933413,
            "unit": "ns"
          }
        ]
      }
    ]
  }
}