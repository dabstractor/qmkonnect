No Valid Bug-Hunt Verdict

The bug finder ran on 2026-08-07T14:47:38Z but produced no parseable, self-consistent
TestResults JSON verdict -- neither in its file nor in its chat output,
and the forced-conversion step could not produce one either.
Reason: no TestResults JSON object found in file or transcript

This is NOT 'clean'. The orchestrator treats a missing verdict as a FAILURE
and refuses to mark the run done. Recover the findings from the transcript
and persist a TestResults JSON, then re-run:
  transcript: plan/007_fb356ba503b4/bugfix/001_e0af83b781d3/bug-hunt-transcript.log
  expected:   plan/007_fb356ba503b4/bugfix/001_e0af83b781d3/bug_hunt_result.json

Delete this file AND the transcript only after you confirm the run was clean.
