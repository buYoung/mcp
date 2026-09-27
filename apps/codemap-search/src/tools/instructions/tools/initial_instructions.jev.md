Register the user's task and relevance questions before search, read, grep or non-root overview. Jev judges each candidate with its supplied source evidence against these questions to filter unrelated code or declarations.

Copy the full user request into task_query. Split it into distinct ways code can contribute to the answer; yes means related. Each question should test one coherent relationship. Merge equivalent questions and keep name or spelling variants as clues within a question, but do not pack independently useful roles into a long checklist merely to reduce the count. The 1–8 question limit is a capacity, not a target count.

Make each question independently state the requested target, relationship and scope. A candidate need only contribute to that part of the task, not implement a whole flow. A short wrapper or setup function can contribute through a source-backed connection without naming the target itself.

Set when_true to observable facts establishing that contribution, including a qualifying branch, callback or delegation. Set when_false to facts establishing unrelated behavior; missing names or unavailable context alone do not establish false. Do not turn a request to explain a flow into proof that every candidate performs all its steps.

Set match="any" for these alternative ways of being relevant. Use "all" only when every criterion must hold for each candidate. Reuse the registration across retrieval queries and tool calls; replace it when the user's task changes.

Enabled search, read, grep and non-root overview use the same task criteria. Root overview stays unfiltered. A Jev omission is a classification result, not a missing file. An exact read location, view=source or include_seen=true does not bypass enabled Jev filtering; include_seen controls only duplicate delivery.
