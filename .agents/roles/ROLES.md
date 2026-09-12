# Roles

Index of available role files in `.agents/roles/`. Each role is a specialized mode with its own instructions, workflow, and constraints.

| Role | File | Short Description |
|------|------|-------------------|
| Architect | [architect.md](architect.md) | Plan and design before implementation |
| Ask | [ask.md](ask.md) | Get answers and explanations |
| Code | [code.md](code.md) | Write, modify, and refactor code |
| Librarian | [librarian.md](librarian.md) | Document and organize project knowledge |
| Orchestrator | [orchestrator.md](orchestrator.md) | Coordinate complex multi-step tasks |
| Prompter | [prompter.md](prompter.md) | Craft, refine, and debug LLM prompts |
| Scoper | [scoper.md](scoper.md) | Turn vague task ideas into unambiguous specifications |

## When to Use

### Architect

Use when you need to **plan, design, or strategize before implementation**. Ideal for breaking down complex problems, creating technical specifications, designing system architecture, or brainstorming solutions before coding. Gathers context, asks clarifying questions, produces an actionable todo list, and saves the plan as a markdown file in `plans/`. Does not implement — the user switches to another role (usually Code) once the plan is approved.

### Ask

Use when you need **explanations, documentation, or answers to technical questions**. Best for understanding concepts, analyzing existing code, getting recommendations, or learning about technologies — without making any changes. Answers thoroughly and does not switch to implementing code unless explicitly requested.

### Code

Use when you need to **write, modify, or refactor code**. Ideal for implementing features, fixing bugs, creating new files, or making code improvements across any programming language or framework. Follows the development guidelines stored in the memory files.

### Librarian

Use when **capturing or reorganizing project knowledge**:

- Completing a significant feature or subsystem and needing to document what was built and why
- A user correction reveals a reusable pattern or preference worth recording
- Memory files exceed the 500-line limit and need splitting
- New architectural decisions, patterns, or conventions emerged during work
- Onboarding a new codebase area into the knowledge base
- Updating the `memory/MEMORY.md` entrypoint index or extracting large content into `memory/assets/`

Not for: writing implementation code (Code), debugging runtime issues (Debug), or planning new features (Architect).

### Orchestrator

Use for **complex, multi-step projects requiring coordination across specialties**. Decomposes large tasks into ordered sub-tasks, decides what to handle in the current chat versus separate chats, delegates implementation to the Code role with exact file paths and build commands, verifies each sub-task's output, and re-verifies the assembled whole. Reads plans and memory files itself; delegates only implementation writes.

### Prompter

Use when **crafting or refining LLM prompts**:

- Writing a new prompt for an LLM-based feature or tool
- An existing prompt produces inconsistent or low-quality outputs
- Debugging prompt behavior with unusual inputs or edge cases
- Designing prompt structure for a new AI-powered capability
- Creating few-shot examples or output format specifications
- Reviewing a prompt for anti-patterns or ambiguity

Not for: implementing code that calls the LLM API (Code), designing overall system architecture (Architect), or debugging runtime application errors (Debug).

### Scoper

Use when **requirements are unclear or incomplete**:

- User has a complex task idea but only a brief or vague description
- Requirements are unclear, incomplete, or contradictory
- The task touches multiple subsystems and needs context discovery before planning
- User wants to explore feasibility or scope before committing to implementation
- Previous planning attempts failed due to missing requirements
- User explicitly wants to "think through" a feature before building it

Operates in three phases: autonomous context discovery, iterative requirements elicitation, then delivery of a complete task specification. Writes no files until explicitly asked. Not for tasks with clear requirements (Architect or Code directly), simple fixes, or documentation work (Librarian).

## Choosing a Role

- **Vague idea, unclear requirements** → Scoper
- **Clear task, needs a plan** → Architect
- **Plan approved, needs implementation** → Code
- **Large multi-phase plan, needs coordination** → Orchestrator
- **Question, no changes needed** → Ask
- **Knowledge capture or memory maintenance** → Librarian
- **Prompt engineering for LLM features** → Prompter
