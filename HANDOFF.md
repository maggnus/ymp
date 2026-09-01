# HANDOFF — backend development

Короткая точка продолжения для следующего разработчика. Продуктовые этапы и общая матрица вынесены
в [`STAGES.md`](STAGES.md).

## Состояние репозитория

- Ветка: `main`, синхронизирована с `origin/main`.
- Версия workspace: `1.0.7`.
- Основной код: Rust workspace в `ymp-rust/`.
- Поставка: один executable `ymp`.
- Принятый целевой путь пользователя: `ymp-docs/USER_JOURNEY.md`.
- Краткий контекст будущего интерфейса: `ymp-docs/DESIGN_CONTEXT.md`.

## Что уже реализовано в backend

| Подсистема | Что работает | Основные точки входа |
|---|---|---|
| Application core | Типизированные команды, один доверенный writer, восстановление из журнала, пути к приватным workspace/evidence | `ymp-rust/crates/ymp-application/` |
| Хранение | Digest-linked JSONL journal, атомарные метаданные, content-addressed objects, один run на store | `ymp-storage`, `ymp-application` |
| Runtime supervisor | Запуск, события, usage, resume, cancel/interrupt, bounded shutdown, завершение потомков | `ymp-runtime-api`, `ymp-runtime-supervisor` |
| Runtime drivers | Управляемые Codex и Claude Code профили; поведенческая совместимость Codex | `ymp-runtime-codex`, `ymp-runtime-claude` |
| Providers/models | Сохранённое enable/disable, фоновое измерение, model catalog, ошибки каталога | `ymp-runtime-registry`, `ymp-application/src/provider.rs` |
| AgentPool | Автоматический `default`, допустимые provider/model/profile entries, freeze по значению на run | `ymp-domain/src/pool.rs`, `ymp-application/src/pool.rs` |
| Participants | Origin participant и механический recruitment gate | `ymp-application/src/participant.rs`, recruitment/application commands |
| Commitments | Offers, bids, task contracts, obligations, budgets, finite terminal transitions | `ymp-domain`, `ymp-kernel`, `ymp-protocol` |
| Collaboration board | Agent publish/read, inert payload, author, scope/audience, operator projection | `ymp-board`, `ymp-application` board projection |
| Candidates | Изолированные попытки, immutable candidate ancestry, competing branches | `ymp-candidate`, `ymp-application` |
| Verification | Независимый verifier, exact evidence binding, negative controls, accepted export | `ymp-verifier`, `ymp-application`, `ymp-cli` |
| TUI backend | Foreground session, typed Application projection, существующие runtime/provider/pool/run страницы | `ymp-rust/crates/ymp-tui/` |

## Что осталось реализовать

Работать сверху вниз: каждый пункт должен давать новый сквозной продуктовый результат, а не только
новый тип или экран.

| Приоритет | Backend-результат | Текущее ограничение | Точки входа |
|---:|---|---|---|
| **1** | Русская цель превращается во внутреннее структурированное намерение и различающие проверки | `derive_check` использует ASCII/английские основы; «Сделай игру “Морской бой”» не получает содержательной проверки | `ymp-application/src/answer.rs` |
| **2** | Provider setup честно проводит enable → background model catalog → models или точную ошибку | Управляемый sign-in отсутствует; ошибки поздних операций не везде привязаны к конкретному start/invocation | `ymp-runtime-registry/src/provider.rs`, provider Application API |
| **3** | Pool ceilings реально ограничивают participant starts, recruitment и расход | Freeze существует, но пределы не проведены через весь исполняемый путь | `ymp-domain/src/pool.rs`, `ymp-application/src/pool.rs`, participant/recruitment path |
| **4** | Несколько участников проходят один настоящий product run | Origin и recruitment реализованы отдельно, но основной путь доказан только с одним участником | `ymp-application/src/participant.rs`, runtime supervisor, recruitment tool |
| **5** | Пользователь читает `/board` через Application projection | Operator projection готова, но не проведена в целевой TUI/session | `Application::operator_board_projection`, `ymp-tui` session/projection |
| **6** | Пользователь публикует на board инертное приписанное вмешательство | Agent publish существует; отдельного user-authored command/path нет | board command types, `ymp-application`, `ymp-tui` input path |
| **7** | Team projection показывает только фактически запущенных участников и объявленную работу | Целевой `/agents` ещё не связан со сквозным multi-participant run | Application participant/task projections, `ymp-tui` |
| **8** | Результат открывается внешней командой/путём, отзыв возвращается в разговор, применение явно | Export работает, но полный external run → feedback → revision → apply flow не замкнут | result/export Application commands, CLI/TUI foreground session |
| **9** | Изменение результата создаёт новую версию с сохранённым provenance | Один store содержит один run; продолжение и история ограничены | storage root/run model, candidate ancestry, Application session |
| **10** | Model admission получает controller attestation и открывает gate | Последний preflight остановился на process-cleanup conformance; `model_ready=false` | `ymp-runtime-supervisor`, tool-host probe, `ymp-corpus/src/admission.rs` |

## Первая рекомендуемая задача

Начать с **русской нормализации намерения**, потому что без неё главный пользовательский сценарий не
может честно перейти от фразы к проверяемой работе.

Минимальный результат:

1. Ввод `Сделай игру «Морской бой»` сохраняется дословно.
2. Backend формирует структурированный результат: тип приложения, среда запуска, основные
   наблюдаемые требования и человеческий остаток.
3. Если среда действительно неоднозначна, задаётся один блокирующий вопрос; следующий вопрос не
   появляется до ответа.
4. Проверяющий план не зависит от требования пользователю писать английское имя файла.
5. Изменённые требования имеют различающий отрицательный пример.

Начать чтение с:

- `ymp-rust/crates/ymp-application/src/answer.rs`;
- тестов `answer`/drafting в `ymp-application` и `ymp-tui`;
- разделов «Непрерывная история» и «Текущая реализация и разрывы» в
  `ymp-docs/USER_JOURNEY.md`.

Не менять TUI-композицию в этой задаче. Сначала нужен типизированный Application-результат и его
исполняемая отрицательная проверка.

## Проверки для backend-карточек

- Узкие тесты изменённых packages и отрицательная половина load-bearing поведения.
- Строгий Clippy только затронутых packages.
- `cargo fmt --all -- --check`.
- `git diff --check`.
- Полный workspace suite — только на интеграционном/release gate.
- Behavioral/e2e запуск — только в свежем disposable root с отдельными `project`, `HOME`,
  `YMP_HOME`, `TMPDIR`, `build`, `export`.

## Не начинать без отдельного решения владельца

- новый live/model/provider admission run;
- matched-budget POC-3;
- message-intervention experiment;
- push/deploy/необратимую внешнюю операцию.
