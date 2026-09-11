# Product requirements

`ymp` is a universal, chat-oriented terminal application for a user-configured team of local AI agents. Software development is the initial acceptance scenario; the domain model does not require Git, source code, or automated tests for every task.

The user chooses provider-backed profiles, models, and instructions. Participants have no permanent leader. They propose plans, bid for tasks, and independently review outcomes. Application rules determine valid state changes and record the evidence used in assignments.

Quality takes priority over minimizing model calls. Runs operate autonomously within provider capabilities. Users can send steering messages, inspect task progress, stop execution, and resume saved sessions. The application does not continue running as a service after the terminal closes.

Experience has two components: competence statistics for profile versions and verified reusable knowledge. Project-specific knowledge stays project-scoped. General procedures can transfer between projects only after independent review. Learning does not rewrite user instructions or train model weights.

All application-owned metadata belongs under `~/.ymp2`. Installed providers retain their own authentication and native session storage. The project, documentation, and application interface use English.
