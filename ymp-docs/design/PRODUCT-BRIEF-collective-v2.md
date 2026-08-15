# Product brief v2: TUI ownership model and CRD-oriented resource model

Recorded verbatim from the owner, 2026-08-15 (HKT). Two parts, delivered in one sitting. Part A
is described by the owner as "желаемая и неоспоримая модель для TUI"; Part B is the CRD-oriented
resource model. Together they refine [PRODUCT-BRIEF-collective.md](PRODUCT-BRIEF-collective.md) at
the product level: where v2 and v1 or the current design disagree, v2 wins. Kernel trust boundaries
and verification principles are explicitly preserved by v2. Where v2 and owner decision D1 disagree
on the pool entity, v2 wins and D1 is annotated accordingly.

Owner's Russian is kept as written; the CRD part is in the owner's English.

---

## Part A — Desired TUI / ownership model

вот желаемая и не оспаримая модель для tui.

Нужно скорректировать дизайн с учётом следующей продуктовой модели. Не упрощай внутреннюю архитектуру за счёт удаления контрактов, verification, budget, isolation и других защитных механизмов. Меняется прежде всего ownership и пользовательский интерфейс: внутренняя сложность должна быть скрыта за продуктовой границей.

Главная идея продукта:

    operator starts ymp
        -> sees a simple TUI
        -> configures/enables providers if necessary
        -> enters a natural-language goal
        -> an autonomous collective of agents solves the task
        -> the collective самостоятельно решает, сколько участников нужно,
           какие модели использовать, как декомпозировать работу,
           кому делегировать, кого привлечь, кого заменить,
           как проверять и исправлять результат
        -> operator receives the result and evidence

Ключевая UX-модель:

    "Я даю ymp задачу.
     Коллектив сам решает, как её выполнить.
     ymp обеспечивает ресурсы, безопасность, ограничения и проверяемость.
     Я получаю результат."

Оператор НЕ должен вручную:
- создавать contract;
- создавать verifier;
- писать acceptance criteria;
- создавать oracle;
- создавать команду;
- создавать отдельных agents;
- назначать роли;
- назначать количество agents;
- распределять задачи;
- вручную выбирать, какой agent выполняет какую часть;
- вручную запускать review/testing;
- вручную запускать verification.

Если contract/oracle/verifier/internal acceptance machinery необходимы архитектурно — сохранить их.
Но это внутренняя ответственность ymp/collective/kernel, а не prerequisite для оператора.

Оператор должен отвечать только за:
- исходный goal;
- подключение/включение providers;
- при необходимости явные ограничения/policy;
- genuine ambiguity, когда намерение невозможно определить автоматически;
- наблюдение;
- pause/cancel/intervention;
- inspect/export результата.

Не спрашивать оператора о внутренней machinery.

----------------------------------------------------------------
1. ОСНОВНАЯ МОДЕЛЬ PROVIDER / MODEL / AGENT POOL / AGENT
----------------------------------------------------------------

Нужно чётко разделить:

Provider
    источник моделей и способ доступа к ним.

Model
    конкретная модель, обнаруженная у Provider.

AgentPool
    набор моделей/возможностей, из которых Collective имеет право создавать
    участников конкретной задачи.

Agent / Participant
    конкретный запущенный участник Collective.

Task
    пользовательская задача.

Collective
    самоорганизующаяся группа участников, решающая Task.

Не путать Model и Agent.

Если Provider содержит 100 моделей, это НЕ означает 100 agents.

Пример:

    Anthropic
      ├── Opus 5
      ├── Sonnet 5
      └── Haiku 5

    OpenAI
      ├── ...
    
    NVIDIA
      ├── GLM
      └── ...

Это каталог доступных возможностей.

Из него создаётся AgentPool.

Например:

    AgentPool: default
      - Opus 5
      - GLM
      - Fable

Это НЕ означает:

    Opus × 1
    GLM × 1
    Fable × 1

Это означает только:

    "Collective разрешено использовать эти модели."

Далее Collective сам решает:

    Opus × 1

или:

    Opus × 1
    GLM × 1
    Fable × 1

или:

    Opus × 2
    GLM × 3

или начать с одного участника и позднее привлечь ещё.

Количество и фактический состав участников — семантическое решение Collective.

Kernel только проверяет механические ограничения:
- model/provider availability;
- allowed pool;
- budget;
- concurrency;
- participant-start limits;
- runtime availability;
- capability restrictions;
- disclosure policy;
- execution assurance;
- resource limits.

Kernel НЕ решает, какая модель "лучше" для задачи.

----------------------------------------------------------------
2. KUBERNETES-АНАЛОГИЯ
----------------------------------------------------------------

Архитектуру и будущие CRD следует проектировать так, чтобы она естественно
ложилась на Kubernetes mental model, но не копировать Kubernetes буквально.

Полезная аналогия:

    Provider
        -> источник моделей

    Model
        -> конкретная доступная модель

    AgentPool
        -> примерно аналог NodePool:
           набор возможностей, из которых Collective может создавать Agents

    Agent
        -> примерно аналог Pod:
           конкретный runtime participant

    Collective
        -> семантический controller:
           решает, сколько и каких Agents создать/завершить

Важно:
AgentPool НЕ является фиксированной командой и НЕ должен автоматически
создавать по одному Agent на каждую модель.

AgentPool задаёт пространство допустимых возможностей.

Collective фактически выполняет роль интеллектуального controller/autoscaler:
он сам принимает решение о необходимости дополнительных участников.

Поэтому replicas не должны означать:
"запусти N агентов постоянно".

Если нужны min/max, они являются ограничениями:

    minAgents
    maxAgents

а не требованием поддерживать N реплик.

Не надо преждевременно вводить отдельный механический AgentAutoscaler,
если это не нужно. В текущей модели масштабирование является семантическим
решением Collective.

----------------------------------------------------------------
3. ПРЕДВАРИТЕЛЬНАЯ CRD-МОДЕЛЬ
----------------------------------------------------------------

Используй следующую модель как направление, но не считай её догмой.
Нужно проверить её против реальных пользовательских сценариев.

Provider:

    kind: Provider

Provider включает/выключает источник и содержит его connection/auth/configuration.
После явного включения выполняется autodetect.

Model:

    kind: Model

Model является обнаруженной записью каталога Provider.

AgentPool:

    kind: AgentPool

AgentPool содержит разрешённые модели.

Например концептуально:

    kind: AgentPool
    metadata:
      name: default
    spec:
      models:
        - anthropic/opus-5
        - glm/glm-5
        - fable/fable

Task:

    kind: Task

Task содержит пользовательский goal и workspace/context.

Например:

    kind: Task
    spec:
      goal: "Создай браузерную игру сапер"
      agentPool: default
      workspace: .

Collective:

    kind: Collective

Collective является runtime-сущностью, связанной с Task, и организует
самоорганизующуюся работу.

Agent:

    kind: Agent

Agent является конкретным runtime participant и аналогичен Pod:
он создаётся динамически Collective, а не оператором.

Candidate:

    kind: Candidate

Immutable результат определённой ветки/синтеза.

Verification:

    kind: Verification

Внутренняя проверка Candidate. Оператор не создаёт её вручную.

Result:

    kind: Result

Пользовательский финальный результат Task с verification/evidence.

Эта модель должна быть проверена против существующей архитектуры и не должна
приводить к появлению лишних сущностей в пользовательском интерфейсе.

----------------------------------------------------------------
4. OPERATOR НЕ СОЗДАЁТ AGENTPOOL ОБЯЗАТЕЛЬНО
----------------------------------------------------------------

Очень важное UX-решение:

Если Provider включён и модели обнаружены, ymp автоматически создаёт
полезный default AgentPool.

То есть оператор НЕ должен получать:

    "No AgentPool configured"

и НЕ должен быть вынужден создавать pool вручную только для того,
чтобы начать работу.

Пример:

Provider:
    Anthropic ✓
    Models: 8

После этого ymp автоматически имеет:

    AgentPool: default
    Models: all currently available/allowed models

Оператор сразу может написать:

    Создай браузерную игру сапер

Продвинутый пользователь позднее может изменить default pool или создать
специализированный pool, например для эксперимента.

Но pool — это конфигурация возможностей, а не команда.

----------------------------------------------------------------
5. ПЕРВЫЙ ЗАПУСК TUI
----------------------------------------------------------------

TUI должен начинаться максимально просто.

Без необходимости предварительно проходить setup wizard.

Базовый экран:

    ┌─────────────────────────────────────┐
    │                 ymp                 │
    │                                     │
    │  ~/projects/minesweeper             │
    │                                     │
    │  >                                   │
    └─────────────────────────────────────┘

Показывать:
- logo;
- базовую информацию;
- текущий каталог/workspace;
- строку ввода.

Это минимальная поверхность.

Все дополнительные удобные элементы можно добавлять позже,
но они не должны ломать базовый mental model.

----------------------------------------------------------------
6. ПЕРВАЯ ЗАДАЧА
----------------------------------------------------------------

Оператор пишет:

    Создай браузерную игру сапер

Если всё готово:

    Task created
    Collective starting...

Если ничего не настроено, НЕ показывать техническую ошибку:

    "no providers configured"

Вместо этого понятное состояние:

    Для выполнения задачи нужен хотя бы один
    включенный AI provider.

    /providers

Важно: не создавать отдельный conversational setup wizard.
Использовать существующую командную модель TUI.

----------------------------------------------------------------
7. /providers
----------------------------------------------------------------

Provider management должен быть в первую очередь табличным,
в стиле k9s.

Поддерживается фиксированный список Provider'ов.

Например:

    PROVIDER        STATUS       MODELS
    ─────────────────────────────────────
    Anthropic       disabled     —
    OpenAI          disabled     —
    NVIDIA          disabled     —
    Google          disabled     —

Пользователь видит весь поддерживаемый список.

Provider НЕ должен autodetect'иться до явного включения пользователем.

После выбора Provider открывается обычный property/detail view.

Например:

    Anthropic
    ─────────────────────────────
    Status       disabled
    Authentication ...
    Models       —
    
    Enable

После явного Enable:

    Anthropic
    ─────────────────────────────
    Status       ready
    Authentication detected
    Models       8
    Last refresh  just now

    Refresh models
    Disable

После успешного autodetect ymp получает каталог моделей.

Каталог моделей также преимущественно табличный:

    MODEL             STATUS
    ───────────────────────────────
    opus-5            available
    sonnet-5          available
    haiku-5           available

Нужна возможность Refresh для повторного получения каталога.

Provider configuration и Model catalog — разные вещи.

Provider — источник/connection.
Model — обнаруженная capability.

----------------------------------------------------------------
8. ТАБЛИЧНЫЙ UI
----------------------------------------------------------------

По возможности интерфейс должен использовать таблицы,
аналогичные k9s.

Обычные property/detail views использовать только там,
где таблица неудобна.

Это также должно хорошо подготовить модель интерфейса
к будущей реализации через Kubernetes CRD-подобные сущности.

Основной UI mental model:

    list -> select -> properties -> action

а не длинные wizard/dialog chains.

----------------------------------------------------------------
9. ПОЛНЫЙ HAPPY PATH
----------------------------------------------------------------

Реальный путь:

    $ ymp

    -> startup
    -> workspace detection
    -> provider/runtime readiness
    -> operator enters:

       "Создай браузерную игру сапер"

    -> если Provider не готов:
       /providers

    -> включение Provider
    -> autodetect
    -> model catalog
    -> default AgentPool автоматически готов

    -> Task created
    -> Collective starts

    -> Agent A starts
    -> Agent A анализирует задачу

    -> Agent A понимает, что нужна дополнительная expertise
    -> Collective requests another participant

    -> Kernel проверяет mechanical constraints
    -> Agent B admitted

    -> Agent A/B самостоятельно исследуют/реализуют/обсуждают

    -> при необходимости Agent C появляется для review/testing

    -> Candidate

    -> Verification

    -> если verification failed:
       Collective получает разрешённый диагностический результат
       и решает, как исправлять

    -> новый Candidate

    -> Verification succeeds

    -> Operator sees:

       ✓ VERIFIED

       Result
       Participants
       Time
       Cost
       Evidence

       [Inspect] [Verify] [Export] [Continue] [Archive]

Оператор ни разу не создавал:
- contract;
- verifier;
- oracle;
- Agent;
- role;
- team;
- decomposition;
- model assignment;
- review assignment.

----------------------------------------------------------------
10. INTERNAL CONTRACT / ORACLE / VERIFICATION
----------------------------------------------------------------

Существующую строгую архитектуру contract/oracle/verification НЕ удалять.

Изменить ownership.

Внутри системы:

    operator goal
        ↓
    intent interpretation
        ↓
    observable requirements
        ↓
    internal contract
        ↓
    acceptance strategy
        ↓
    verification
        ↓
    result

Разделять provenance:

A. operator-supplied
B. repository-discovered
C. collective-inferred
D. internally-generated verification machinery
E. explicit operator clarification

Если существенное предположение нельзя безопасно разрешить автоматически,
спросить оператора.

Нельзя спрашивать:

    "Создайте acceptance oracle."

Можно спросить:

    "Для авторизации использовать OAuth Authorization Code
     или Client Credentials?"

То есть спрашивать только о реальном intent ambiguity.

Сохраняются:
- exact digests;
- protected verification;
- no secret requirements;
- negative controls;
- infrastructure_error ≠ rejection;
- bounded verification-query budget;
- independence from producer conversation;
- candidate immutability;
- reproducibility.

----------------------------------------------------------------
11. NO CENTRAL SEMANTIC ORCHESTRATOR
----------------------------------------------------------------

Не создавать скрытого "manager agent", который фактически становится
центральным planner.

Collective — распределённая интеллектуальная система.

Kernel:
- authority;
- accounting;
- lifecycle;
- isolation;
- concurrency;
- leases;
- capabilities;
- provenance;
- state integrity;
- candidate immutability;
- verification authorization.

Collective:
- semantic decisions;
- decomposition;
- delegation;
- recruitment;
- model selection;
- collaboration;
- review;
- synthesis;
- revision;
- stopping.

TUI:
- interaction;
- visibility;
- control.

Verification:
- independent evidence.

Kernel НЕ:
- выбирает лучшую модель;
- назначает роли;
- декомпозирует задачу;
- ранжирует agents по intelligence;
- синтезирует ответ;
- решает semantic quality.

----------------------------------------------------------------
12. AGENT RECRUITMENT
----------------------------------------------------------------

Agent creation должен быть динамическим.

Например:

    Agent A: Opus
        ↓
    "Мне нужен ещё один независимый взгляд"
        ↓
    recruitment request
        ↓
    kernel checks:
      pool
      budget
      concurrency
      provider availability
      runtime capability
      policy
        ↓
    Agent B: GLM
        ↓
    joins collective

Не создавать всех возможных agents заранее.

100 моделей в Provider ≠ 100 agents.

100 моделей = 100 потенциальных capabilities.

Количество agents определяется Collective в пределах механических лимитов.

Нужно сохранить finite resource accounting:
- recruitment/proposal budget;
- execution escrow;
- communication budget;
- task budget;
- runtime-start budget;
- concurrency budget.

Запуск дополнительного agent не должен быть бесплатным.

----------------------------------------------------------------
13. OPERATOR CONTROL
----------------------------------------------------------------

Оператор — principal, а не team manager.

Минимальный набор команд/поверхностей:

    /providers
    /models
    /agents
    /tasks
    /activity
    /budget
    /verify
    /result
    /help

Точные названия могут быть скорректированы после анализа текущего TUI,
но mental model должен остаться.

Operator может:
- видеть providers;
- видеть models;
- видеть pools;
- видеть agents;
- видеть tasks;
- видеть activity;
- видеть budgets;
- pause;
- cancel;
- clarify;
- optionally intervene;
- inspect result;
- inspect evidence;
- export;
- archive;
- continue.

TUI не должен превращаться в per-agent control panel.

----------------------------------------------------------------
14. /agents
----------------------------------------------------------------

Оператор должен видеть коллектив как живую систему.

Например:

    AGENTS

    NAME          MODEL       STATUS      TASK
    ─────────────────────────────────────────────
    agent-a       Opus 5      working     minesweeper
    agent-b       GLM         reviewing   minesweeper
    agent-c       Fable       testing     minesweeper

При выборе Agent — property/detail view.

Показывать:
- identity;
- model;
- provider;
- runtime;
- lifecycle state;
- task;
- resource usage;
- progress events;
- findings;
- submissions;
- verification requests/results;
- recruitment;
- failures;
- yields;
- cancellation.

НЕ показывать private chain-of-thought.

Collaboration messages — untrusted collaboration data,
сохраняется existing observation policy.

----------------------------------------------------------------
15. RESULT
----------------------------------------------------------------

Главный пользовательский результат:

    ✓ VERIFIED

а не:

    Agents stopped.

Result должен показывать:
- completed summary;
- verification status;
- participants;
- attempts;
- elapsed time;
- cost;
- candidate digest;
- evidence;
- export capability.

Actions:

    Inspect result
    Inspect evidence
    Export
    Continue
    Archive

Если verification failed, Collective должен иметь возможность
реагировать при наличии budget/policy.

Если ресурсы закончились:

    BUDGET EXHAUSTED

Если controller/infrastructure failed:

    INFRASTRUCTURE ERROR

Если operator cancelled:

    CANCELLED

Если ambiguity не разрешена:

    NEEDS CLARIFICATION

Не смешивать эти terminal states и никогда не выдавать exhausted
как success.

----------------------------------------------------------------
16. POOLS
----------------------------------------------------------------

Нужна концепция pool, но не превращать её в ручную сборку команды.

Default pool создаётся автоматически после успешного Provider/model discovery.

Продвинутый пользователь может позже иметь:

    /pools

Например:

    NAME       MODELS
    ─────────────────────────────────
    default    Opus 5, GLM, Fable
    cheap      Haiku, ...
    research   Opus, Fable

Но эти pools — capability/resource boundaries,
а не semantic roles.

Не создавать:

    architect-pool
    coder-pool
    reviewer-pool

как обязательную систему.

Не делать hard-coded assumptions:
"Opus = architect"
"GLM = coder"
"Fable = reviewer"

Если Collective использует модель для такой роли — это его собственное
семантическое решение.

----------------------------------------------------------------
17. PROVIDER VS POOL
----------------------------------------------------------------

Важно не смешивать:

Provider:
    "какие модели у меня вообще доступны?"

AgentPool:
    "какие из доступных моделей Collective разрешено использовать
     для этой работы?"

Collective:
    "каких и сколько участников мне реально нужно?"

Agent:
    "конкретный участник, который сейчас работает."

Это должно быть понятно и в CRD-модели, и в TUI.

----------------------------------------------------------------
18. RUN-SCOPED FREEZE
----------------------------------------------------------------

Нужно сохранить сопоставимость экспериментов и корректную attribution,
но не заставлять оператора изучать отдельную сложную сущность.

Рекомендуемая модель:

- Provider registry остаётся местом, где оператор включает/выключает Provider.
- Model catalog отражает текущие обнаруженные модели.
- При создании Task/run его effective AgentPool фиксируется внутренним snapshot/digest.
- Изменение Provider/model availability после начала run не должно незаметно
  менять возможности уже работающего run.
- Следующий run может получить обновлённый pool.

Если это необходимо для experimental comparability,
механизм freeze остаётся внутренней частью run contract/provenance.

Не заставлять оператора вручную создавать этот snapshot.

----------------------------------------------------------------
19. ПРОВЕРИТЬ ТЕКУЩУЮ АРХИТЕКТУРУ
----------------------------------------------------------------

Теперь нужно не просто переписать UI.

Нужно инспектировать существующий repository и определить,
где текущая архитектура нарушает эту модель.

Особенно найти ownership leaks:

- PROJECT-CONTRACT.md требуется от пользователя;
- verifier creation требуется от пользователя;
- manual agent creation;
- manual team assembly;
- manual model assignment;
- manual decomposition;
- provider configuration смешан с participant creation;
- runtime profiles выглядят как user-level roles;
- acceptance machinery вынесена в operator workflow;
- hidden manager/orchestrator фактически принимает semantic decisions.

Для каждого такого места определить:
- что остаётся;
- кому передаётся ownership;
- какой API/state transition нужен;
- что меняется в TUI;
- какие тесты нужны.

Не удалять safety mechanisms только потому,
что они усложняют UX.

----------------------------------------------------------------
20. CRD-ORIENTED DESIGN
----------------------------------------------------------------

Будущий дизайн должен быть пригоден для естественного представления
через Kubernetes-like CRDs.

Предварительная conceptual model:

    Provider
    Model
    AgentPool
    Task
    Collective
    Agent
    Candidate
    Verification
    Result

Но не считать это окончательным набором CRD.

Главный критерий:

    operator-facing configuration
        ≠
    runtime objects
        ≠
    trusted kernel state

Не заставлять пользователя создавать runtime objects вручную.

Особенно:

    Agent
    Candidate
    Verification

должны возникать в ходе выполнения.

AgentPool должен быть похож на NodePool:
он описывает пространство доступных возможностей,
а не фиксированный набор runtime instances.

----------------------------------------------------------------
21. TUI INFORMATION ARCHITECTURE
----------------------------------------------------------------

Начать проектирование TUI с нуля от lifecycle, а не от существующих экранов.

Минимально пройти:

    startup
    first run
    provider discovery
    provider enable/auth
    model discovery
    model catalog
    automatic default pool
    task creation
    collective startup
    live collective
    /agents
    task/activity
    recruitment
    candidate
    verification
    revision after failure
    result
    evidence
    export
    pause
    resume where supported
    cancellation
    intervention
    budget exhaustion
    infrastructure failure
    ambiguity/clarification
    history
    archive
    recovery from invalid configuration

Большинство списков — tables.
Properties/details — обычные views.

Стиль должен быть близок к k9s:
list -> select -> properties -> action.

По ощущению продукт должен быть ближе к:

    Claude Code / Paseo

но architecture/lifecycle/security semantics остаются собственными.

----------------------------------------------------------------
22. ACCEPTANCE TEST
----------------------------------------------------------------

Основной end-to-end сценарий:

    $ ymp

    configure Anthropic + OpenAI + NVIDIA
    enter:
        "Создай браузерную игру сапер"

    No manual:
        contract
        oracle
        verifier
        team
        agents
        roles
        model assignment
        decomposition

    default AgentPool exists automatically.

    Collective starts one participant.

    Participant decides additional expertise is useful.

    Collective recruits another participant from AgentPool.

    Kernel validates only mechanical constraints.

    Agents collaborate.

    One implements.

    Another challenges/reviews.

    Candidate produced.

    Verification fails.

    Collective receives permitted diagnostics.

    Collective revises.

    Verification succeeds.

    Operator receives:

        COMPLETED
        VERIFIED

    with result/evidence/cost/time/participants
    and can Inspect / Export / Continue / Archive.

Если для прохождения этого сценария оператору всё ещё приходится
создавать contract/verifier/team/roles/agents/model assignments,
ownership boundary всё ещё неправильный.

----------------------------------------------------------------
23. IMPORTANT: DO NOT OVER-DESIGN THE USER EXPERIENCE
----------------------------------------------------------------

Не превращать operator UX в Kubernetes administration console.

CRD analogy нужна для правильной архитектуры и будущего declarative API,
но основной локальный продукт должен оставаться простым:

    $ ymp
    > Создай браузерную игру сапер

    [collective works]

    ✓ Verified result

Пользователь должен чувствовать:

    "Я дал задачу коллективу."

а не:

    "Я создал десять Kubernetes-like объектов,
     настроил oracle и собрал workflow."

----------------------------------------------------------------
24. WHAT TO DELIVER
----------------------------------------------------------------

На основе этого redesign:

1. Проведи анализ текущего repository.
2. Покажи текущие ownership leaks.
3. Определи целевую product mental model.
4. Определи operator lifecycle от запуска до Result.
5. Спроектируй TUI с нуля.
6. Определи Provider/Model/AgentPool/Task/Collective/Agent lifecycle.
7. Определи dynamic recruitment.
8. Определи default AgentPool creation.
9. Определи run-scoped pool snapshot/freeze.
10. Сохрани strict contract/oracle/verification internals.
11. Перенеси их ownership внутрь системы.
12. Определи необходимые domain/state/API changes.
13. Определи Rust module/crate changes.
14. Определи migration path.
15. Определи end-to-end tests.
16. Отдельно укажи решения, которые действительно требуют человеческого решения.

Не придумывай недостающие детали ради полноты.
Если решение действительно не определено этим brief — зафиксируй его как open decision.

Главный критерий дизайна:

    Operator:
        provides intent and boundaries.

    Collective:
        provides intelligence and semantic decisions.

    Kernel:
        provides authority, resources, lifecycle, isolation,
        accounting and state integrity.

    Verification:
        provides independent evidence.

    TUI:
        makes the whole process simple and observable.

Итоговый пользовательский experience:

    GIVE THE COLLECTIVE A GOAL
        →
    GET A VERIFIED RESULT

---

## Part B — CRD-oriented resource model

Design the product domain from the beginning as a Kubernetes/CRD-oriented
declarative resource model, even though the current implementation remains a
single local `ymp` executable and does not require Kubernetes.

The TUI should feel simple and natural; Kubernetes terminology does not need to
be exposed to the operator. However, the underlying domain resources must have
clear declarative `spec` and observed `status` semantics wherever appropriate,
so they can later map naturally to Kubernetes CRDs without redesigning the
domain.

Use these conceptual resources:

- Provider — configured external AI provider and its discovered model catalog.
- AgentPool — the permitted pool of models/runtimes from which the collective
  may create participants, together with capacity and execution policy.
- Agent — one actual participant created from an AgentPool; analogous to a Pod.
- Task — the operator's requested work.
- Run — one execution of a Task.
- Candidate — an immutable result/artifact produced by the collective.
- Verification — an independent verification operation and its resulting status.

The relationship is:

    Provider
      └── Model Catalog
            └── AgentPool
                  ├── allowed models/providers
                  ├── resource limits
                  ├── replicas / min replicas / max replicas
                  └── autoscaling policy
                         │
                         ▼
                      Agents
                         │
                         ▼
                       Task
                         │
                         ▼
                     Candidate
                         │
                         ▼
                    Verification

Do not instantiate every model as an Agent. Providers may expose hundreds of
models; an Agent is created only when the collective actually needs a
participant.

AgentPool is the important abstraction for this purpose. It defines the set of
agent implementations available to the collective and the mechanical limits
within which it may recruit them.

Replicas and autoscaling belong to the pool/execution policy, not to an
individual Agent. The operator may establish minimum, maximum, desired or
autoscaling bounds; the collective decides how many participants it actually
needs within those bounds.

Do not force an incorrect Kubernetes analogy:
- Provider is NOT a Node.
- AgentPool is NOT literally a NodePool.
- Agent is conceptually similar to a Pod because it represents an actual
  running participant.
- The analogy is about declarative resource/lifecycle semantics, not a
  one-to-one mapping to Kubernetes objects.

For every resource, explicitly define where appropriate:

- `spec` — desired/configured state;
- `status` — observed state;
- references/ownership between resources;
- mutable versus immutable fields;
- lifecycle and terminal states;
- reconciliation responsibility;
- events and conditions.

The future Kubernetes implementation should be able to represent these
resources as CRDs and reconcile them without changing the core product
semantics.

Most importantly, introducing a CRD-oriented model must NOT introduce a hidden
central semantic orchestrator. Controllers may reconcile mechanical state,
capacity, lifecycle and resource availability. The collective remains
responsible for semantic decisions: decomposition, recruitment, collaboration,
model selection, implementation, review, revision and stopping.

The current local TUI and single-process implementation should therefore be
treated as the first local controller/runtime for this declarative model, with
Kubernetes as a future execution/control-plane implementation rather than a
different product architecture.
