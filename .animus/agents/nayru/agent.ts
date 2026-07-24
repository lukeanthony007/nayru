import { defineAgent } from "@animus/agents";

/**
 * coding — full developer agent. Shell + file read/write/edit, project-scoped.
 * Pair this with a project's `.animus/agents/` so its workspace is the repo.
 */
export default defineAgent({
  // model omitted → inherits the active/selected model (provider-agnostic).
  description: "Developer agent — reads, edits, runs the repo inside the sandbox.",
  riskProfile: "trusted",
  tools: { allow: ["shell", "file_read", "file_write", "file_edit", "glob_search", "content_search"] },
  parallelTools: true,
  limits: { maxToolIterations: 40 },
});
