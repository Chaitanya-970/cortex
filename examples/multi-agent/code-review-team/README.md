# Reference Multi-Agent Workflow: Code Review Team

This reference example demonstrates a production-grade, hierarchical multi-agent collaboration pipeline in Cortex runtime.

## Team Topology

```text
       ┌───────────┐
       │  Manager  │ (Supervisor & Stage Coordinator)
       └─────┬─────┘
   ┌─────────┼─────────┐
   ▼         ▼         ▼
┌────────┐ ┌───────┐ ┌──────────┐
│Research│ │ Coder │ │ Reviewer │
└────────┘ └───────┘ └──────────┘
```

- **Manager (`agents/manager.yaml`)**: Root supervisor. Deconstructs the user goal into stage delegations and enforces stage transition rules.
- **Researcher (`agents/researcher.yaml`)**: Specializes in repository inspection, error reproduction, and root-cause diagnosis.
- **Coder (`agents/coder.yaml`)**: Applies minimal atomic patches based on researcher findings.
- **Reviewer (`agents/reviewer.yaml`)**: Runs automated verification, inspects diffs for regression risks, and approves or requests rework.

## Workflow Stages (`workflow.yaml`)

1. **`research`**: Analyzes test failure in `workspace/tests/test_calc.py` and emits `bug_analysis`.
2. **`implementation`**: Consumes `bug_analysis` to patch `workspace/src/calc.py`.
3. **`review`**: Executes tests against the patch and provides the final sign-off.

## Running the Example

```bash
./run.sh
```

Or via the Cortex CLI:

```bash
cortex workflow run workflow.yaml \
  --task "Fix division by zero error in calc.py and verify test suite"
```
