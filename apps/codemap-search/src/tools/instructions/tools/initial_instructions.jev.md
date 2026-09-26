Register the user's task and relevance questions before search, read or grep. Jev judges each code body with its supplied supporting evidence against these questions to filter unrelated code.

Copy the full user request into task_query. Build questions from the aspects needed for that request: target names and variants, the requested behavior, and contributing callers, data contracts or execution conditions. Each question must cover one aspect and ask whether the candidate contributes to the task, with yes meaning related. For example, "the JSON editor does not work" can become:

- Does this code define or reference the JSON editor, including alternate names?
- Does it implement the JSON editor's initialization, rendering or editing behavior?
- Does it provide data, configuration or calls needed for the JSON editor to work?

Keep the user's scope in each question. A helper can contribute through the supplied evidence without naming the target or performing the main operation itself. Set when_true to facts establishing that relationship and when_false to facts establishing unrelated behavior; a missing name or unavailable context alone does not establish false.

Set match="any" for these alternative ways of being relevant. Use "all" only when every criterion must hold for each candidate. Reuse the registration across retrieval queries and tool calls; replace it when the user's task changes.
