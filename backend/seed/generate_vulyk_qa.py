#!/usr/bin/env python3
"""Generate backend/seed/vulyk-qa.json — Vulyk QA demo project.

Run: python3 backend/seed/generate_vulyk_qa.py
Idempotent output; commit the JSON. Keep this script for maintainability.
"""
from __future__ import annotations

import json
from pathlib import Path

OUT = Path(__file__).with_name("vulyk-qa.json")

# Case counter
_n = 0


def L(en: str, uk: str | None = None) -> dict:
    return {"en": en, "uk": uk if uk is not None else en}


def next_key() -> str:
    global _n
    _n += 1
    return f"VQA-{_n:03d}"


def case(
    title_en: str,
    *,
    title_uk: str | None = None,
    desc_en: str,
    desc_uk: str | None = None,
    pre_en: str,
    pre_uk: str | None = None,
    priority: str = "medium",
    status: str = "ready",
    tags: list[str],
    case_type: str = "functional",
    steps: list[dict],
    external_key: str | None = None,
) -> dict:
    """Build one seed case. case_type is stored as tag type:<name>."""
    all_tags = list(dict.fromkeys([*tags, f"type:{case_type}", "vulyk-qa"]))
    c: dict = {
        "key": next_key(),
        "title": L(title_en, title_uk),
        "description": L(desc_en, desc_uk),
        "precondition": L(pre_en, pre_uk),
        "priority": priority,
        "status": status,
        "tags": all_tags,
        "steps": steps,
    }
    if external_key:
        c["external_key"] = external_key
    return c


def step(action_en: str, expected_en: str, data_en: str = "", *, action_uk: str | None = None, expected_uk: str | None = None, data_uk: str | None = None) -> dict:
    return {
        "action": L(action_en, action_uk),
        "expected_result": L(expected_en, expected_uk),
        "data": L(data_en, data_uk if data_uk is not None else data_en),
    }


def suite(name_en: str, name_uk: str, children: list | None = None, cases: list | None = None) -> dict:
    return {
        "name": L(name_en, name_uk),
        "children": children or [],
        "cases": cases or [],
    }


ADMIN = "admin@vulyk.local / admin (must_change_password on first login after seed)"
DEMO = "user@vulyk.local / user (demo user, VULYK_SEED_DEMO=1)"
BASE = "Demo install with VULYK_SEED_SAMPLE=1 (or wizard seed_sample checked); auth may be open locally"


def build() -> dict:
    global _n
    _n = 0

    suites = [
        # ── 01 Setup & Install ──────────────────────────────────────────
        suite(
            "01. Setup & Install",
            "01. Встановлення та перший запуск",
            cases=[
                case(
                    "Complete first-run setup via web wizard",
                    title_uk="Завершити перше налаштування через веб-майстер",
                    desc_en="Verify a fresh install can complete setup with token, locale, timezone, and admin credentials.",
                    desc_uk="Перевірити завершення першого налаштування токеном, локаллю, часовим поясом і обліковими даними адміна.",
                    pre_en=f"Fresh DB; setup not completed; setup token known. {BASE}.",
                    pre_uk="Чиста БД; setup не завершено; відомий setup-токен.",
                    priority="critical",
                    tags=["smoke", "regression", "e2e", "setup"],
                    external_key="e2e:setup-wizard.spec.ts",
                    steps=[
                        step(
                            "Open the UI root URL (e.g. http://localhost:5173/) while setup is incomplete.",
                            "Setup wizard is shown (or redirect to setup); getByLabel(/Setup token|Setup-токен/i) is visible.",
                        ),
                        step(
                            "Fill Setup token, Display name, Admin email, Admin password (≥8 chars with letter+digit), locale en, timezone UTC, registration open; click Complete setup.",
                            "Setup completes; user lands in app shell (.app-shell) or login; setup_completed is true.",
                            "admin@example.com / Str0ngPass1",
                        ),
                        step(
                            "GET /api/v1/setup/status (or auth/bootstrap) and confirm setup is completed.",
                            "Response indicates setup completed; wizard is no longer offered.",
                        ),
                    ],
                ),
                case(
                    "Reject setup wizard without valid setup token",
                    title_uk="Відхилити майстер без валідного setup-токена",
                    desc_en="Negative: incomplete or wrong setup token must not create the admin.",
                    pre_en="Fresh install; setup incomplete; valid token exists but tester uses a wrong one.",
                    priority="high",
                    tags=["regression", "e2e", "setup", "negative"],
                    case_type="negative",
                    external_key="e2e:setup-wizard.spec.ts",
                    steps=[
                        step(
                            "Open setup wizard and submit with an invalid setup token.",
                            "Error message is visible; admin user is not created; setup_completed remains false.",
                            "token: not-the-real-token",
                        ),
                    ],
                ),
                case(
                    "Seed admin via env on API boot",
                    title_uk="Сід адміна через env при старті API",
                    desc_en="With VULYK_ADMIN_EMAIL and VULYK_ADMIN_PASSWORD set, API boot creates admin and marks setup complete.",
                    pre_en="Compose/dev with VULYK_ADMIN_EMAIL=admin@vulyk.local and VULYK_ADMIN_PASSWORD=admin.",
                    priority="critical",
                    tags=["smoke", "api", "setup"],
                    steps=[
                        step(
                            "Start API with admin env vars set; wait until /health returns ok.",
                            "GET /health → 200 {\"status\":\"ok\"}.",
                        ),
                        step(
                            "POST /api/v1/auth/login with seeded admin credentials.",
                            "200 with token/session; user email is admin@vulyk.local.",
                            ADMIN,
                        ),
                    ],
                ),
                case(
                    "CLI setup --help documents seed-sample flag",
                    title_uk="CLI setup --help документує прапорець seed-sample",
                    desc_en="vulyk-api setup --help lists --seed-sample and first-run flags.",
                    pre_en="vulyk-api binary available (cargo run -p vulyk-api -- setup --help).",
                    priority="low",
                    tags=["regression", "setup", "cli"],
                    external_key="e2e:setup-wizard.spec.ts",
                    steps=[
                        step(
                            "Run `vulyk-api setup --help` (or cargo run -p vulyk-api -- setup --help).",
                            "Help text includes --seed-sample, --admin-email, --locale, --registration-mode.",
                        ),
                    ],
                ),
                case(
                    "Prod mode does not seed demo user unless enabled",
                    title_uk="Prod-режим не сідить demo-користувача без увімкнення",
                    desc_en="With VULYK_SEED_DEMO=0 (prod default), user@vulyk.local is absent.",
                    pre_en="Prod-like env: VULYK_SEED_DEMO=0, setup completed with admin only.",
                    priority="high",
                    tags=["regression", "api", "setup", "negative"],
                    case_type="negative",
                    steps=[
                        step(
                            "List users as system admin (Admin → Users or GET /api/v1/admin/users).",
                            "user@vulyk.local is not present; only the configured admin exists (plus any manually created users).",
                        ),
                    ],
                ),
                case(
                    "Sample projects seed when VULYK_SEED_SAMPLE enabled",
                    title_uk="Сід sample-проєктів при увімкненому VULYK_SEED_SAMPLE",
                    desc_en="After setup with seed_sample, both Kyiv concert and Vulyk QA projects exist.",
                    pre_en=f"{BASE}; setup completed with seed_sample on.",
                    priority="critical",
                    tags=["smoke", "regression", "e2e", "api", "setup"],
                    external_key="e2e:sample-project.spec.ts",
                    steps=[
                        step(
                            "Sign in as admin and GET /api/v1/projects.",
                            "Response includes keys KYIV_CONCERT_2026 and VULYK_QA.",
                            ADMIN,
                        ),
                        step(
                            "Select project Vulyk QA in .project-select.",
                            "Cases page shows folder tree with '01. Setup & Install' (or UK equivalent).",
                        ),
                    ],
                ),
                case(
                    "VULYK_SEED_SAMPLE=0 skips sample projects",
                    title_uk="VULYK_SEED_SAMPLE=0 пропускає sample-проєкти",
                    desc_en="Opt-out flag prevents Kyiv and Vulyk QA sample seeding.",
                    pre_en="Fresh setup with VULYK_SEED_SAMPLE=0 (or wizard unchecked).",
                    priority="medium",
                    tags=["regression", "api", "setup", "negative"],
                    case_type="negative",
                    steps=[
                        step(
                            "GET /api/v1/projects as admin after setup.",
                            "Neither KYIV_CONCERT_2026 nor VULYK_QA is present.",
                        ),
                    ],
                ),
            ],
        ),
        # ── 02 Authentication ───────────────────────────────────────────
        suite(
            "02. Authentication",
            "02. Автентифікація",
            children=[
                suite(
                    "Login",
                    "Вхід",
                    cases=[
                        case(
                            "Login with seeded admin succeeds",
                            title_uk="Вхід сідованим адміном успішний",
                            desc_en="Admin can sign in and reach the app shell with Cases nav.",
                            pre_en=f"Setup completed; {ADMIN}.",
                            priority="critical",
                            tags=["smoke", "regression", "e2e", "auth"],
                            external_key="e2e:auth.spec.ts",
                            steps=[
                                step(
                                    "Open login panel; fill Email and Password; submit.",
                                    ".app-shell is visible; getByRole('navigation', {name: 'Primary'}) contains Cases|Кейси.",
                                    "admin@vulyk.local / admin (or changed password)",
                                ),
                            ],
                        ),
                        case(
                            "Bad credentials show an error",
                            title_uk="Невірні облікові дані показують помилку",
                            desc_en="Wrong password does not enter the app.",
                            pre_en="Login page reachable; admin account exists.",
                            priority="high",
                            tags=["regression", "e2e", "auth", "negative"],
                            case_type="negative",
                            external_key="e2e:auth.spec.ts",
                            steps=[
                                step(
                                    "Fill Email with admin@vulyk.local and Password with definitely-wrong-password; submit.",
                                    "Login error is visible; .app-shell count is 0.",
                                ),
                            ],
                        ),
                        case(
                            "Login API returns JWT/session for valid user",
                            title_uk="Login API повертає JWT/сесію для валідного користувача",
                            desc_en="POST /api/v1/auth/login succeeds for demo user.",
                            pre_en=f"{DEMO}; VULYK_SEED_DEMO=1.",
                            priority="high",
                            tags=["smoke", "api", "auth"],
                            steps=[
                                step(
                                    "POST /api/v1/auth/login with JSON email/password.",
                                    "200 with access token (or Set-Cookie session); subsequent authenticated GET /api/v1/auth/me returns the user.",
                                    "user@vulyk.local / user",
                                ),
                            ],
                        ),
                    ],
                ),
                suite(
                    "Forced password change",
                    "Примусова зміна пароля",
                    cases=[
                        case(
                            "Seeded admin with password admin must change password",
                            title_uk="Сідований адмін з паролем admin мусить змінити пароль",
                            desc_en="must_change_password blocks normal navigation until ChangePassword succeeds.",
                            pre_en="Admin seeded with VULYK_ADMIN_PASSWORD=admin and must_change_password true.",
                            priority="critical",
                            tags=["smoke", "regression", "e2e", "auth"],
                            steps=[
                                step(
                                    "Sign in with admin@vulyk.local / admin.",
                                    "Change password page/form is shown (ChangePassword); primary app nav for Cases is not fully usable until change.",
                                ),
                                step(
                                    "Submit a new strong password and confirm.",
                                    "Password updated; user can open Cases; subsequent login uses the new password.",
                                    "NewPassword1!",
                                ),
                            ],
                        ),
                        case(
                            "Reject weak password on forced change",
                            title_uk="Відхилити слабкий пароль при примусовій зміні",
                            desc_en="Negative: password policy rejects too-short passwords.",
                            pre_en="User has must_change_password=true.",
                            priority="medium",
                            tags=["regression", "auth", "negative"],
                            case_type="negative",
                            steps=[
                                step(
                                    "On ChangePassword form, enter a short password (e.g. 'abc') and submit.",
                                    "Validation error is shown; must_change_password remains true.",
                                    "abc",
                                ),
                            ],
                        ),
                    ],
                ),
                suite(
                    "Registration",
                    "Реєстрація",
                    cases=[
                        case(
                            "Open registration shows create-account flow",
                            title_uk="Відкрита реєстрація показує створення акаунта",
                            desc_en="When registration_mode=open, login offers create account.",
                            pre_en="Admin settings registration_mode=open (compose default).",
                            priority="high",
                            tags=["regression", "e2e", "auth"],
                            external_key="e2e:registration-sso.spec.ts",
                            steps=[
                                step(
                                    "Open login page and look for create-account / register control.",
                                    "Create account UI is visible; new user can register with email/password.",
                                ),
                                step(
                                    "Register a unique email and sign in.",
                                    "Account is created; user can authenticate; no system_admin flag.",
                                    "qa+register@example.com / RegPass1!",
                                ),
                            ],
                        ),
                        case(
                            "Disabled registration hides create-account",
                            title_uk="Вимкнена реєстрація ховає створення акаунта",
                            desc_en="registration_mode=disabled removes self-signup.",
                            pre_en="Admin sets registration_mode=disabled via Admin → Settings or API.",
                            priority="high",
                            tags=["regression", "e2e", "auth", "negative"],
                            case_type="negative",
                            external_key="e2e:registration-sso.spec.ts",
                            steps=[
                                step(
                                    "As admin, set registration mode to disabled; open login as anonymous.",
                                    "Create-account control is hidden; POST /api/v1/auth/register returns error/403/409.",
                                ),
                            ],
                        ),
                        case(
                            "Invite registration mode without invite token fails",
                            title_uk="Режим invite без токена запрошення не вдається",
                            desc_en="Partial feature: invite mode requires token; creating invites may be planned.",
                            pre_en="registration_mode=invite.",
                            priority="medium",
                            status="draft",
                            tags=["regression", "auth", "planned", "negative"],
                            case_type="negative",
                            steps=[
                                step(
                                    "Attempt register without invite token.",
                                    "Registration rejected. (If admin invite-create UI/API is missing, treat create-invite as planned.)",
                                ),
                            ],
                        ),
                    ],
                ),
                suite(
                    "SSO",
                    "SSO",
                    cases=[
                        case(
                            "Mock OIDC SSO provider login succeeds",
                            title_uk="Вхід через Mock OIDC SSO успішний",
                            desc_en="Configured SSO provider appears on login and completes callback.",
                            pre_en="Admin enabled a mock/test OIDC provider (e2e fixture).",
                            priority="high",
                            tags=["regression", "e2e", "auth", "sso"],
                            external_key="e2e:registration-sso.spec.ts",
                            steps=[
                                step(
                                    "Open login; click the SSO provider button for the mock OIDC provider.",
                                    "Browser redirects to IdP (or mock); after callback user is authenticated in .app-shell.",
                                ),
                            ],
                        ),
                        case(
                            "Admin can list and configure SSO providers",
                            title_uk="Адмін може переглядати та налаштовувати SSO-провайдерів",
                            desc_en="Admin → SSO lists Google, Entra, GitHub, GitLab, Nextcloud, generic OIDC slots.",
                            pre_en=f"Signed in as system admin. {ADMIN}.",
                            priority="medium",
                            tags=["regression", "e2e", "auth", "sso", "admin"],
                            steps=[
                                step(
                                    "Open Admin → SSO (or GET /api/v1/admin/sso/providers).",
                                    "Provider list is returned; enabling a provider without secrets shows validation error.",
                                ),
                            ],
                        ),
                        case(
                            "Disabled SSO provider is not shown on login",
                            title_uk="Вимкнений SSO-провайдер не показується на логіні",
                            desc_en="Negative: disabled providers must not appear as login buttons.",
                            pre_en="All SSO providers disabled in admin settings.",
                            priority="medium",
                            tags=["regression", "auth", "sso", "negative"],
                            case_type="negative",
                            steps=[
                                step(
                                    "Open login page as anonymous user.",
                                    "No SSO provider buttons are shown (only local email/password unless registration link).",
                                ),
                            ],
                        ),
                    ],
                ),
                suite(
                    "Logout & sessions",
                    "Вихід і сесії",
                    cases=[
                        case(
                            "Logout returns to login panel",
                            title_uk="Вихід повертає на панель логіну",
                            desc_en="Logout clears session and allows signing back in.",
                            pre_en="User signed in as admin.",
                            priority="critical",
                            tags=["smoke", "regression", "e2e", "auth"],
                            external_key="e2e:auth.spec.ts",
                            steps=[
                                step(
                                    "Click logout control from the shell.",
                                    "Login panel visible; getByLabel(/^Email$/i) is present; .app-shell gone.",
                                ),
                                step(
                                    "Sign back in with admin credentials.",
                                    ".app-shell visible again within timeout.",
                                    ADMIN,
                                ),
                            ],
                        ),
                        case(
                            "Auth me returns current user preferences",
                            title_uk="Auth me повертає поточного користувача та вподобання",
                            desc_en="GET /api/v1/auth/me includes email, roles, locale/theme prefs when set.",
                            pre_en="Authenticated session.",
                            priority="medium",
                            tags=["api", "auth"],
                            steps=[
                                step(
                                    "GET /api/v1/auth/me with Bearer/cookie.",
                                    "200 JSON with email and id; unauthenticated call returns 401 when auth required.",
                                ),
                            ],
                        ),
                    ],
                ),
            ],
        ),
        # ── 03 RBAC ─────────────────────────────────────────────────────
        suite(
            "03. RBAC",
            "03. RBAC",
            children=[
                suite(
                    "Admin",
                    "Admin",
                    cases=[
                        case(
                            "Admin sees Admin nav and users list",
                            title_uk="Admin бачить навігацію Admin і список користувачів",
                            desc_en="System/project admin can open Admin and list users.",
                            pre_en=f"Signed in as system admin. {ADMIN}.",
                            priority="critical",
                            tags=["smoke", "regression", "e2e", "rbac"],
                            external_key="e2e:rbac.spec.ts",
                            steps=[
                                step(
                                    "Click Primary nav link Admin.",
                                    "Admin page loads; users list is visible.",
                                ),
                            ],
                        ),
                        case(
                            "Admin can manage project memberships",
                            title_uk="Admin може керувати членством у проєкті",
                            desc_en="Project admin assigns roles to users.",
                            pre_en="Admin on a project; another user exists.",
                            priority="high",
                            tags=["regression", "api", "rbac"],
                            steps=[
                                step(
                                    "POST membership with role viewer for a user on the project.",
                                    "201/200; user appears with role viewer; Viewer restrictions apply on next login.",
                                ),
                            ],
                        ),
                        case(
                            "Admin can delete project",
                            title_uk="Admin може видалити проєкт",
                            desc_en="Only admin role may delete a project.",
                            pre_en="Disposable project created for the test.",
                            priority="medium",
                            tags=["regression", "api", "rbac"],
                            steps=[
                                step(
                                    "DELETE /api/v1/projects/{id} as project admin.",
                                    "204/200; project no longer listed.",
                                ),
                            ],
                        ),
                    ],
                ),
                suite(
                    "Test Automation Engineer",
                    "Test Automation Engineer",
                    cases=[
                        case(
                            "TAE can create and edit cases but not delete",
                            title_uk="TAE може створювати/редагувати кейси, але не видаляти",
                            desc_en="test_automation_engineer lacks delete_cases permission.",
                            pre_en=f"{DEMO} membership as test_automation_engineer on a project.",
                            priority="high",
                            tags=["regression", "e2e", "api", "rbac"],
                            steps=[
                                step(
                                    "As TAE, create a case via UI Create case button.",
                                    "Case appears with auto PREFIX-N key.",
                                ),
                                step(
                                    "Attempt DELETE case via API.",
                                    "403 Forbidden; case still exists.",
                                ),
                            ],
                        ),
                        case(
                            "TAE can configure CI and create API tokens",
                            title_uk="TAE може налаштовувати CI і створювати API-токени",
                            desc_en="TAE has CI config + api_tokens permissions.",
                            pre_en="TAE membership on project.",
                            priority="medium",
                            tags=["regression", "rbac", "ci"],
                            steps=[
                                step(
                                    "Open CI page and save a CI config; open Admin→Tokens and create a token.",
                                    "CI config saved; token value shown once (vulyk_at_… prefix).",
                                ),
                            ],
                        ),
                        case(
                            "TAE cannot open requirements or import",
                            title_uk="TAE не може відкрити вимоги чи імпорт",
                            desc_en="Negative: Import/Requirements nav hidden or API 403.",
                            pre_en="TAE membership; no admin.",
                            priority="medium",
                            tags=["regression", "rbac", "negative"],
                            case_type="negative",
                            steps=[
                                step(
                                    "As TAE, try Primary nav Import and Requirements (or corresponding API).",
                                    "Nav items hidden and/or API returns 403.",
                                ),
                            ],
                        ),
                    ],
                ),
                suite(
                    "Test Architect",
                    "Test Architect",
                    cases=[
                        case(
                            "Test Architect can import and manage requirements",
                            title_uk="Test Architect може імпортувати і керувати вимогами",
                            desc_en="Architect has import + requirements; no CI/tokens.",
                            pre_en="User with test_architect membership.",
                            priority="high",
                            tags=["regression", "rbac"],
                            steps=[
                                step(
                                    "Open Import and Requirements; create a requirement.",
                                    "Both pages accessible; requirement created.",
                                ),
                                step(
                                    "Attempt to create API token / CI config.",
                                    "403 or UI controls absent.",
                                ),
                            ],
                        ),
                        case(
                            "Test Architect can delete cases and configure LLM",
                            title_uk="Test Architect може видаляти кейси і налаштовувати LLM",
                            desc_en="Architect has delete_cases and llm_config.",
                            pre_en="test_architect on project with disposable case.",
                            priority="medium",
                            tags=["regression", "rbac", "llm"],
                            steps=[
                                step(
                                    "Delete a case; open Admin LLM and save a mock provider config for the project.",
                                    "Case deleted (key retired); LLM config saved.",
                                ),
                            ],
                        ),
                    ],
                ),
                suite(
                    "Developer",
                    "Developer",
                    cases=[
                        case(
                            "Developer can execute runs but not edit cases",
                            title_uk="Developer може виконувати прогони, але не редагувати кейси",
                            desc_en="Developer: view + execute + trigger CI; no case edit.",
                            pre_en="developer membership; run with cases exists.",
                            priority="high",
                            tags=["regression", "rbac"],
                            steps=[
                                step(
                                    "Open a run and mark a case Passed.",
                                    "Status updates; history row created.",
                                ),
                                step(
                                    "Attempt to edit case title via API PATCH.",
                                    "403 Forbidden.",
                                ),
                            ],
                        ),
                        case(
                            "Developer cannot access Import or Generate",
                            title_uk="Developer не має доступу до Import чи Generate",
                            desc_en="Nav gated by permissions.",
                            pre_en="developer role only.",
                            priority="medium",
                            tags=["regression", "rbac", "negative"],
                            case_type="negative",
                            steps=[
                                step(
                                    "Inspect Primary nav for Import and Generate.",
                                    "Links hidden; direct navigation/API returns 403.",
                                ),
                            ],
                        ),
                    ],
                ),
                suite(
                    "Viewer",
                    "Viewer",
                    cases=[
                        case(
                            "Viewer cannot mutate and Create case is hidden",
                            title_uk="Viewer не може змінювати дані; Create case сховано",
                            desc_en="Viewer has view_project + reports only.",
                            pre_en="viewer membership on project.",
                            priority="critical",
                            tags=["smoke", "regression", "e2e", "rbac", "negative"],
                            case_type="negative",
                            external_key="e2e:rbac.spec.ts",
                            steps=[
                                step(
                                    "Sign in as viewer; open Cases.",
                                    "Create case button not visible; Import hidden.",
                                ),
                                step(
                                    "POST create case via API as viewer.",
                                    "403 Forbidden.",
                                ),
                            ],
                        ),
                        case(
                            "Viewer can open Reports and export",
                            title_uk="Viewer може відкрити Reports і експортувати",
                            desc_en="view_reports and export_reports allowed.",
                            pre_en="viewer on project with some run data.",
                            priority="medium",
                            tags=["regression", "rbac", "reports"],
                            steps=[
                                step(
                                    "Open Reports; click Export CSV.",
                                    "Dashboard/KPIs visible; CSV download starts or API export 200.",
                                ),
                            ],
                        ),
                    ],
                ),
            ],
        ),
        # ── 04 Projects ─────────────────────────────────────────────────
        suite(
            "04. Projects",
            "04. Проєкти",
            cases=[
                case(
                    "Admin creates project with key and name",
                    title_uk="Admin створює проєкт з ключем і назвою",
                    desc_en="Admin → create project with explicit key.",
                    pre_en=f"Signed in as admin. {ADMIN}.",
                    priority="critical",
                    tags=["smoke", "regression", "e2e", "projects"],
                    external_key="e2e:projects-cases.spec.ts",
                    steps=[
                        step(
                            "Open Admin; create project with name and unique key.",
                            "Project appears in .project-select; GET project returns same key.",
                            "key: unique e.g. PJXXXX",
                        ),
                    ],
                ),
                case(
                    "Create project with name only auto-derives key and prefix",
                    title_uk="Створення проєкту лише з назвою автогенерує ключ і префікс",
                    desc_en="Name initials become case_key_prefix (e.g. Kyiv Auto Concert → KAC).",
                    pre_en="Admin UI project create allows name-only.",
                    priority="high",
                    tags=["regression", "e2e", "projects", "case-keys"],
                    external_key="e2e:projects-cases.spec.ts",
                    steps=[
                        step(
                            "Create project named like 'Kyiv Auto Concert {unique}'.",
                            "Project created; case_key_prefix derived from initials; project key auto-assigned if empty.",
                        ),
                    ],
                ),
                case(
                    "Edit case key prefix before any cases exist",
                    title_uk="Редагувати префікс ключів кейсів до появи кейсів",
                    desc_en="Admins may change prefix when next_case_number is 0 / no cases.",
                    pre_en="Empty project as admin.",
                    priority="high",
                    tags=["regression", "api", "projects", "case-keys"],
                    steps=[
                        step(
                            "PATCH project case_key_prefix to a valid 2–8 char uppercase code.",
                            "Prefix updated; first created case uses NEW-1.",
                            "prefix: QA",
                        ),
                    ],
                ),
                case(
                    "Reject invalid case key prefix",
                    title_uk="Відхилити невалідний префікс ключів",
                    desc_en="Negative: prefix must match allowed pattern.",
                    pre_en="Empty project.",
                    priority="medium",
                    tags=["regression", "api", "projects", "negative"],
                    case_type="negative",
                    steps=[
                        step(
                            "PATCH case_key_prefix to '1' or lowercase 'ab' or too-long string.",
                            "400 validation error; prefix unchanged.",
                        ),
                    ],
                ),
                case(
                    "Cannot change prefix after cases exist",
                    title_uk="Не можна змінити префікс після появи кейсів",
                    desc_en="Edge: prefix locked once cases/tombstones exist.",
                    pre_en="Project with at least one case.",
                    priority="medium",
                    tags=["regression", "api", "projects", "edge"],
                    case_type="edge",
                    steps=[
                        step(
                            "Attempt PATCH case_key_prefix to another value.",
                            "Request rejected; existing case keys unchanged.",
                        ),
                    ],
                ),
            ],
        ),
        # ── 05 Case keys ────────────────────────────────────────────────
        suite(
            "05. Case keys",
            "05. Ключі кейсів",
            cases=[
                case(
                    "Auto-allocated keys increment PREFIX-N",
                    title_uk="Автоключі інкрементують PREFIX-N",
                    desc_en="Creating cases without key uses monotonic counter.",
                    pre_en="Project with known prefix; create via UI needing only title.",
                    priority="critical",
                    tags=["smoke", "regression", "e2e", "case-keys"],
                    external_key="e2e:projects-cases.spec.ts",
                    steps=[
                        step(
                            "Create case with title only via getByRole('button', {name: /Create case|Створити кейс/i}).",
                            "New case key is PREFIX-(n+1) vs previous max.",
                        ),
                        step(
                            "Create a second case the same way.",
                            "Key is previous+1; no gaps introduced by successful creates.",
                        ),
                    ],
                ),
                case(
                    "Deleted case key is never reused",
                    title_uk="Ключ видаленого кейсу ніколи не перевикористовується",
                    desc_en="Tombstones keep numbers retired.",
                    pre_en="Project admin/architect; at least one case.",
                    priority="critical",
                    tags=["smoke", "regression", "api", "case-keys"],
                    steps=[
                        step(
                            "Note case key PREFIX-N; delete the case; create a new case.",
                            "New key is PREFIX-(N+1) or higher; PREFIX-N does not reappear.",
                        ),
                    ],
                ),
                case(
                    "Manual key collision rejected",
                    title_uk="Колізія ручного ключа відхиляється",
                    desc_en="Negative: duplicate key in project returns conflict.",
                    pre_en="Existing case KEY-1.",
                    priority="medium",
                    tags=["regression", "api", "case-keys", "negative"],
                    case_type="negative",
                    steps=[
                        step(
                            "POST case with same key as existing.",
                            "409/400; original case unchanged.",
                        ),
                    ],
                ),
            ],
        ),
        # ── 06 Folders ──────────────────────────────────────────────────
        suite(
            "06. Folders",
            "06. Теки",
            children=[
                suite(
                    "CRUD & nesting",
                    "CRUD і вкладеність",
                    cases=[
                        case(
                            "Create nested folder via folder tree",
                            title_uk="Створити вкладену теку через дерево",
                            desc_en="New folder under selected parent appears in tree with counts.",
                            pre_en="Cases page; project selected.",
                            priority="critical",
                            tags=["smoke", "regression", "e2e", "folders"],
                            external_key="e2e:projects-cases.spec.ts",
                            steps=[
                                step(
                                    "In region Case folders|Теки кейсів, click New folder|Нова тека; name it.",
                                    "Folder button appears in .folder-tree under parent.",
                                    "Folder name: Nested QA",
                                ),
                            ],
                        ),
                        case(
                            "Rename folder",
                            title_uk="Перейменувати теку",
                            desc_en="Rename updates tree label without moving cases.",
                            pre_en="Existing folder with cases.",
                            priority="high",
                            tags=["regression", "e2e", "folders"],
                            steps=[
                                step(
                                    "Rename folder via tree action / PATCH suite.",
                                    "New name visible; case count unchanged; cases still listed when selected.",
                                ),
                            ],
                        ),
                        case(
                            "Move folder via drag-drop or Move to",
                            title_uk="Перемістити теку drag-drop або Move to",
                            desc_en="Reparent suite; mobile uses Move to.",
                            pre_en="Two sibling folders A and B.",
                            priority="high",
                            tags=["regression", "e2e", "folders"],
                            steps=[
                                step(
                                    "Drag folder A onto B (desktop) or use Move to on mobile.",
                                    "A becomes child of B; URL/filter still works; no cases lost.",
                                ),
                            ],
                        ),
                        case(
                            "Delete empty folder",
                            title_uk="Видалити порожню теку",
                            desc_en="Empty folder deletes cleanly.",
                            pre_en="Empty folder.",
                            priority="medium",
                            tags=["regression", "folders"],
                            steps=[
                                step(
                                    "Delete the empty folder.",
                                    "Folder removed from tree; API GET suites no longer lists it.",
                                ),
                            ],
                        ),
                        case(
                            "Delete non-empty folder with move_to_parent",
                            title_uk="Видалити непорожню теку з move_to_parent",
                            desc_en="Cases move to parent; keys preserved.",
                            pre_en="Folder with ≥1 case.",
                            priority="high",
                            tags=["regression", "api", "folders"],
                            steps=[
                                step(
                                    "DELETE suite with on_nonempty=move_to_parent.",
                                    "Suite gone; cases now under parent; keys unchanged.",
                                ),
                            ],
                        ),
                        case(
                            "Delete non-empty folder with delete_cases retires keys",
                            title_uk="Видалення теки з delete_cases ізолює ключі",
                            desc_en="Deleting cases via folder delete retires keys.",
                            pre_en="Disposable folder+cases; architect/admin.",
                            priority="high",
                            tags=["regression", "api", "folders", "case-keys"],
                            steps=[
                                step(
                                    "DELETE suite with on_nonempty=delete_cases; create a new case.",
                                    "Deleted keys not reused; counter continues.",
                                ),
                            ],
                        ),
                        case(
                            "Folder case counts include nested children",
                            title_uk="Лічильники кейсів у теці включають нащадків",
                            desc_en="Selecting parent filters list including subfolders.",
                            pre_en="Parent with child folder that has cases.",
                            priority="medium",
                            tags=["regression", "e2e", "folders"],
                            external_key="e2e:projects-cases.spec.ts",
                            steps=[
                                step(
                                    "Click parent folder in tree; observe list and count badge.",
                                    "List shows cases from descendants; count matches.",
                                ),
                            ],
                        ),
                    ],
                ),
            ],
        ),
        # ── 07 Test cases ───────────────────────────────────────────────
        suite(
            "07. Test cases",
            "07. Тест-кейси",
            children=[
                suite(
                    "CRUD & steps",
                    "CRUD і кроки",
                    cases=[
                        case(
                            "Create case with steps via UI",
                            title_uk="Створити кейс із кроками через UI",
                            desc_en="Edit case, Add step, Save persists actions and expected results.",
                            pre_en="Project with suite; admin/TAE.",
                            priority="critical",
                            tags=["smoke", "regression", "e2e", "cases"],
                            external_key="e2e:projects-cases.spec.ts",
                            steps=[
                                step(
                                    "Create case; open it; click Edit; Add step; fill action and expected; Save.",
                                    "Steps visible on Details; GET case returns steps in order.",
                                ),
                            ],
                        ),
                        case(
                            "Update case title and description",
                            title_uk="Оновити назву і опис кейсу",
                            desc_en="PATCH/edit bumps version.",
                            pre_en="Existing case.",
                            priority="high",
                            tags=["regression", "e2e", "cases"],
                            steps=[
                                step(
                                    "Edit title/description; Save.",
                                    "New title in list; version incremented on Details.",
                                ),
                            ],
                        ),
                        case(
                            "Delete case removes it from list",
                            title_uk="Видалення кейсу прибирає його зі списку",
                            desc_en="Admin/architect delete.",
                            pre_en="Disposable case.",
                            priority="high",
                            tags=["regression", "api", "cases"],
                            steps=[
                                step(
                                    "Delete case; search by old key.",
                                    "Not found; key retired.",
                                ),
                            ],
                        ),
                        case(
                            "Reorder steps positions",
                            title_uk="Змінити порядок кроків",
                            desc_en="Step positions remain unique and ordered.",
                            pre_en="Case with ≥2 steps.",
                            priority="medium",
                            tags=["regression", "cases"],
                            steps=[
                                step(
                                    "Move second step above first; Save.",
                                    "GET case steps show swapped positions 0..n.",
                                ),
                            ],
                        ),
                        case(
                            "Case with empty steps list is allowed on create",
                            title_uk="Кейс без кроків дозволений при створенні",
                            desc_en="Edge: title-only create has zero steps until edited.",
                            pre_en="Create case UI.",
                            priority="low",
                            tags=["regression", "cases", "edge"],
                            case_type="edge",
                            steps=[
                                step(
                                    "Create case with title only; open detail.",
                                    "Case exists; steps array empty; can add steps later.",
                                ),
                            ],
                        ),
                    ],
                ),
                suite(
                    "Tags & priority",
                    "Теги і пріоритет",
                    cases=[
                        case(
                            "Assign tags and priority on case",
                            title_uk="Призначити теги і пріоритет кейсу",
                            desc_en="tag_names and priority persist.",
                            pre_en="Editable case.",
                            priority="high",
                            tags=["regression", "e2e", "cases"],
                            steps=[
                                step(
                                    "Set priority critical and tags smoke,regression; Save.",
                                    "Badges show priority; tags filterable in list.",
                                    "priority: critical; tags: smoke,regression",
                                ),
                            ],
                        ),
                        case(
                            "Filter cases by tag",
                            title_uk="Фільтрувати кейси за тегом",
                            desc_en="Tag filter narrows list.",
                            pre_en="Cases with distinct tags.",
                            priority="medium",
                            tags=["regression", "cases"],
                            steps=[
                                step(
                                    "Apply tag filter for a known tag.",
                                    "Only matching cases listed.",
                                ),
                            ],
                        ),
                        case(
                            "Bulk mark ready",
                            title_uk="Масово позначити готовими",
                            desc_en="Bulk action Mark ready|Позначити готовим.",
                            pre_en="Draft cases selectable.",
                            priority="medium",
                            tags=["regression", "e2e", "cases"],
                            external_key="e2e:projects-cases.spec.ts",
                            steps=[
                                step(
                                    "Select cases via getByLabel('Select {key}'); click Mark ready.",
                                    "Selected cases status becomes ready.",
                                ),
                            ],
                        ),
                    ],
                ),
                suite(
                    "Search & filters",
                    "Пошук і фільтри",
                    cases=[
                        case(
                            "FTS search finds case by title token",
                            title_uk="FTS-пошук знаходить кейс за токеном назви",
                            desc_en="Search cases label filters via FTS/trigram.",
                            pre_en="Case with distinctive title.",
                            priority="critical",
                            tags=["smoke", "regression", "e2e", "cases"],
                            external_key="e2e:projects-cases.spec.ts",
                            steps=[
                                step(
                                    "Fill getByLabel(/Search cases|Пошук кейсів/i) with a unique token from title.",
                                    "Matching case visible; unrelated cases hidden.",
                                ),
                            ],
                        ),
                        case(
                            "Status filter draft vs ready",
                            title_uk="Фільтр статусу draft vs ready",
                            desc_en="Status filter selectOption.",
                            pre_en="Mix of draft and ready cases.",
                            priority="high",
                            tags=["regression", "e2e", "cases"],
                            external_key="e2e:projects-cases.spec.ts",
                            steps=[
                                step(
                                    "Select Status filter draft.",
                                    "Only draft cases listed.",
                                ),
                            ],
                        ),
                        case(
                            "Search with no matches shows empty state",
                            title_uk="Пошук без збігів показує порожній стан",
                            desc_en="Negative/edge empty results.",
                            pre_en="Project with cases.",
                            priority="low",
                            tags=["regression", "cases", "edge"],
                            case_type="edge",
                            steps=[
                                step(
                                    "Search for zzz-no-such-case-token-999.",
                                    "Empty list / no-results message; no error toast crash.",
                                ),
                            ],
                        ),
                    ],
                ),
                suite(
                    "History",
                    "Історія",
                    cases=[
                        case(
                            "Case History/Results tab shows cross-run results",
                            title_uk="Вкладка History/Results показує результати з прогонів",
                            desc_en="After executions, case tab lists history.",
                            pre_en="Case executed in a run with status changes.",
                            priority="high",
                            tags=["regression", "e2e", "cases", "runs"],
                            external_key="e2e:result-history.spec.ts",
                            steps=[
                                step(
                                    "Open case; select History / Results tab.",
                                    "Timeline/list shows statuses with timestamps; pass-rate summary present.",
                                ),
                            ],
                        ),
                        case(
                            "Case version history on Details after edit",
                            title_uk="Історія версій на Details після редагування",
                            desc_en="Edit/version history remains on Details tab.",
                            pre_en="Case edited at least once.",
                            priority="medium",
                            tags=["regression", "cases"],
                            steps=[
                                step(
                                    "Open Details; inspect version indicator / history.",
                                    "Version ≥2 after edit; prior version metadata available via API if exposed.",
                                ),
                            ],
                        ),
                    ],
                ),
            ],
        ),
        # ── 08 Requirements ─────────────────────────────────────────────
        suite(
            "08. Requirements",
            "08. Вимоги",
            cases=[
                case(
                    "Create requirement and link to cases",
                    title_uk="Створити вимогу і прив'язати до кейсів",
                    desc_en="Requirements page create + link.",
                    pre_en="Architect/admin; existing cases.",
                    priority="critical",
                    tags=["smoke", "regression", "e2e", "requirements"],
                    external_key="e2e:imports-reports.spec.ts",
                    steps=[
                        step(
                            "Open /requirements; create requirement with title.",
                            "Requirement listed.",
                            "title: VQA coverage REQ",
                        ),
                        step(
                            "Link requirement to one or more case keys.",
                            "GET case requirements includes the requirement; coverage report counts it.",
                        ),
                    ],
                ),
                case(
                    "Unlink requirement from case",
                    title_uk="Відв'язати вимогу від кейсу",
                    desc_en="Unlink removes association only.",
                    pre_en="Linked requirement+case.",
                    priority="medium",
                    tags=["regression", "api", "requirements"],
                    steps=[
                        step(
                            "POST requirements:unlink for the pair.",
                            "Link gone; requirement and case still exist.",
                        ),
                    ],
                ),
                case(
                    "ALM import of remote requirement (OpenProject/Jira)",
                    title_uk="ALM-імпорт віддаленої вимоги (OpenProject/Jira)",
                    desc_en="With integration configured, search/import remote items.",
                    pre_en="Project integration fixture configured (or placeholder).",
                    priority="medium",
                    tags=["regression", "requirements", "integrations"],
                    steps=[
                        step(
                            "Open Integrations; use remote search/import for a fixture key.",
                            "Requirement created with external_key from remote.",
                        ),
                    ],
                ),
            ],
        ),
        # ── 09 Test runs ────────────────────────────────────────────────
        suite(
            "09. Test runs",
            "09. Тест-прогони",
            children=[
                suite(
                    "Create & execute",
                    "Створення і виконання",
                    cases=[
                        case(
                            "Create plan and start a run",
                            title_uk="Створити план і стартувати прогін",
                            desc_en="Runs page Add plan + Create run.",
                            pre_en="Project with cases.",
                            priority="critical",
                            tags=["smoke", "regression", "e2e", "runs"],
                            external_key="e2e:plans-runs.spec.ts",
                            steps=[
                                step(
                                    "Open Runs; Add plan with name; Create run from plan/cases.",
                                    "Heading with run name visible; run items listed under list 'Run cases'.",
                                    "plan/run name unique",
                                ),
                            ],
                        ),
                        case(
                            "Mark run item Passed",
                            title_uk="Позначити елемент прогону Passed",
                            desc_en="Result status group button Passed.",
                            pre_en="Open run with untested item.",
                            priority="critical",
                            tags=["smoke", "regression", "e2e", "runs"],
                            external_key="e2e:plans-runs.spec.ts",
                            steps=[
                                step(
                                    "Select a case in run; click getByRole('group',{name:'Result status'}).getByRole('button',{name:'Passed'}).",
                                    "Status pill shows Passed; history entry created.",
                                ),
                            ],
                        ),
                        case(
                            "Mark run item Failed, Blocked, Skipped",
                            title_uk="Позначити Failed, Blocked, Skipped",
                            desc_en="All execution statuses work and append history.",
                            pre_en="Run with items.",
                            priority="critical",
                            tags=["smoke", "regression", "e2e", "runs"],
                            external_key="e2e:result-history.spec.ts",
                            steps=[
                                step(
                                    "Sequentially set status Passed → Failed → Blocked → Skipped on one case.",
                                    "Run timeline shows each transition; case History tab mirrors statuses.",
                                ),
                            ],
                        ),
                        case(
                            "Attach note and camera photo to result",
                            title_uk="Додати нотатку і фото з камери до результату",
                            desc_en="Mobile/desktop attachment on result.",
                            pre_en="Run open; browser grants file chooser for fixture image.",
                            priority="medium",
                            tags=["regression", "e2e", "runs", "mobile"],
                            external_key="e2e:plans-runs.spec.ts",
                            steps=[
                                step(
                                    "Add comment and Attach photo from camera (file input).",
                                    "Attachment listed on result; comment persisted.",
                                ),
                            ],
                        ),
                        case(
                            "Close completed run",
                            title_uk="Закрити завершений прогін",
                            desc_en="Closing run sets finished_at / completed status.",
                            pre_en="In-progress run.",
                            priority="medium",
                            tags=["regression", "api", "runs"],
                            steps=[
                                step(
                                    "Invoke close/complete run action.",
                                    "status completed|closed; finished_at set.",
                                ),
                            ],
                        ),
                        case(
                            "Cannot execute on closed run",
                            title_uk="Не можна виконувати закритий прогін",
                            desc_en="Negative edge.",
                            pre_en="Closed run.",
                            priority="low",
                            tags=["regression", "runs", "negative"],
                            case_type="negative",
                            steps=[
                                step(
                                    "POST execute result on closed run item.",
                                    "API rejects with 4xx; status unchanged.",
                                ),
                            ],
                        ),
                    ],
                ),
            ],
        ),
        # ── 10 Automation & CI ──────────────────────────────────────────
        suite(
            "10. Automation & CI",
            "10. Автоматизація та CI",
            cases=[
                case(
                    "Upload JUnit XML into target run",
                    title_uk="Завантажити JUnit XML у цільовий прогін",
                    desc_en="Runs page JUnit upload maps skipped→Skipped, error→Failed.",
                    pre_en="Run with matching case names/keys; fixture XML.",
                    priority="critical",
                    tags=["smoke", "regression", "e2e", "api", "ci"],
                    external_key="e2e:smoke.spec.ts",
                    steps=[
                        step(
                            "Select Upload target run; set JUnit XML file; upload.",
                            "Ingest job succeeds; run item statuses updated per JUnit.",
                        ),
                    ],
                ),
                case(
                    "Ingest via API token (CI upload)",
                    title_uk="Ingest через API-токен (CI upload)",
                    desc_en="POST ingest with Authorization token from Admin→Tokens.",
                    pre_en="API token with execute/upload permission; run id.",
                    priority="critical",
                    tags=["smoke", "regression", "api", "ci"],
                    external_key="e2e:imports-reports.spec.ts",
                    steps=[
                        step(
                            "POST /api/v1/runs/{id}/ingest with JUnit body and Bearer vulyk_at_…",
                            "200/202; results applied; unauthorized without token → 401.",
                        ),
                    ],
                ),
                case(
                    "Vulyk JSON bulk results ingest",
                    title_uk="Ingest масових результатів у форматі Vulyk JSON",
                    desc_en="results:bulk endpoint.",
                    pre_en="Run + token.",
                    priority="high",
                    tags=["regression", "api", "ci"],
                    steps=[
                        step(
                            "POST /api/v1/runs/{id}/results:bulk with JSON statuses.",
                            "Items updated; job id retrievable via GET /api/v1/jobs/{id}.",
                        ),
                    ],
                ),
                case(
                    "Save CI trigger config (GitHub/GitLab)",
                    title_uk="Зберегти CI trigger config (GitHub/GitLab)",
                    desc_en="CI page provider/name/JSON config.",
                    pre_en="Admin or TAE.",
                    priority="high",
                    tags=["regression", "e2e", "ci"],
                    external_key="e2e:integrations-llm.spec.ts",
                    steps=[
                        step(
                            "Open /ci; fill CI provider, name, JSON; save.",
                            "Config listed; dispatch history empty until triggered.",
                        ),
                    ],
                ),
                case(
                    "Dispatch CI config",
                    title_uk="Запустити dispatch CI-конфігу",
                    desc_en="Trigger dispatch records history (may use mock).",
                    pre_en="CI config saved; credentials/fixture allow dispatch.",
                    priority="medium",
                    tags=["regression", "api", "ci"],
                    steps=[
                        step(
                            "POST dispatch for config id.",
                            "Dispatch history entry created with status.",
                        ),
                    ],
                ),
                case(
                    "CLI reporter uploads JUnit with --create-run",
                    title_uk="CLI reporter завантажує JUnit з --create-run",
                    desc_en="vulyk-cli report --file --format junit --create-run.",
                    pre_en="CLI built; API token and project key set.",
                    priority="medium",
                    tags=["regression", "cli", "ci"],
                    steps=[
                        step(
                            "Run cargo run -p vulyk-cli -- report --file report.xml --format junit --create-run --project-key VULYK_QA.",
                            "New run created; results ingested.",
                        ),
                    ],
                ),
            ],
        ),
        # ── 11 Reports ──────────────────────────────────────────────────
        suite(
            "11. Reports",
            "11. Звіти",
            cases=[
                case(
                    "Reports dashboard shows KPIs",
                    title_uk="Дашборд Reports показує KPI",
                    desc_en="Reports page headings and .kpi-value elements.",
                    pre_en="Project with runs/results.",
                    priority="critical",
                    tags=["smoke", "regression", "e2e", "reports"],
                    external_key="e2e:imports-reports.spec.ts",
                    steps=[
                        step(
                            "Open Reports via Primary nav.",
                            "Heading Reports visible; KPI values rendered.",
                        ),
                    ],
                ),
                case(
                    "Export reports CSV",
                    title_uk="Експорт звітів у CSV",
                    desc_en="Export CSV button downloads file.",
                    pre_en="Reports page.",
                    priority="high",
                    tags=["regression", "e2e", "reports"],
                    external_key="e2e:imports-reports.spec.ts",
                    steps=[
                        step(
                            "Click Export CSV.",
                            "Download starts; content has expected headers (or API /reports/export 200).",
                        ),
                    ],
                ),
                case(
                    "Requirement coverage report",
                    title_uk="Звіт покриття вимог",
                    desc_en="GET coverage includes linked requirements.",
                    pre_en="Requirements linked to cases with results.",
                    priority="medium",
                    tags=["regression", "api", "reports"],
                    steps=[
                        step(
                            "GET /api/v1/projects/{id}/reports/coverage.",
                            "JSON lists requirements with coverage_percent fields.",
                        ),
                    ],
                ),
                case(
                    "Flaky detection report endpoint",
                    title_uk="Ендпоінт звіту flaky",
                    desc_en="Flaky report returns cases with oscillating statuses.",
                    pre_en="Case with Pass/Fail history across runs.",
                    priority="low",
                    tags=["regression", "api", "reports"],
                    steps=[
                        step(
                            "GET …/reports/flaky.",
                            "200 JSON array/object; flaky candidates listed or empty list.",
                        ),
                    ],
                ),
                case(
                    "Trends report for pass/fail over time",
                    title_uk="Trends-звіт pass/fail у часі",
                    desc_en="Trends endpoint for charts.",
                    pre_en="Multiple runs dated.",
                    priority="medium",
                    tags=["regression", "api", "reports"],
                    steps=[
                        step(
                            "GET …/reports/trends.",
                            "200 with time series points suitable for charts.",
                        ),
                    ],
                ),
            ],
        ),
        # ── 12 Import ───────────────────────────────────────────────────
        suite(
            "12. Import",
            "12. Імпорт",
            children=[
                suite(
                    "Xray / SquashTM / TestLink",
                    "Xray / SquashTM / TestLink",
                    cases=[
                        case(
                            "Import TestLink XML sample",
                            title_uk="Імпорт зразка TestLink XML",
                            desc_en="Import page TestLink fixture.",
                            pre_en="Architect/admin; e2e/fixtures/testlink-sample.xml.",
                            priority="critical",
                            tags=["smoke", "regression", "e2e", "import"],
                            external_key="e2e:imports-reports.spec.ts",
                            steps=[
                                step(
                                    "Open /import; select Importer TestLink; Format XML; paste/upload Source content; run import.",
                                    "Cases created with auto keys; original IDs stored as external_key.",
                                ),
                            ],
                        ),
                        case(
                            "Import Xray JSON sample",
                            title_uk="Імпорт зразка Xray JSON",
                            desc_en="Xray importer.",
                            pre_en="xray-sample.json fixture.",
                            priority="high",
                            tags=["regression", "e2e", "import"],
                            external_key="e2e:imports-reports.spec.ts",
                            steps=[
                                step(
                                    "Import with Importer Xray and JSON content.",
                                    "Preview then import creates cases; no crash on re-import handling.",
                                ),
                            ],
                        ),
                        case(
                            "Import SquashTM XML sample",
                            title_uk="Імпорт зразка SquashTM XML",
                            desc_en="Squash importer.",
                            pre_en="squash-sample.xml.",
                            priority="high",
                            tags=["regression", "e2e", "import"],
                            external_key="e2e:imports-reports.spec.ts",
                            steps=[
                                step(
                                    "Import SquashTM XML via Import page.",
                                    "Suites/cases created reflecting nested structure when present.",
                                ),
                            ],
                        ),
                        case(
                            "Import preview does not persist",
                            title_uk="Preview імпорту не зберігає дані",
                            desc_en="imports:preview is dry-run.",
                            pre_en="Valid import payload.",
                            priority="medium",
                            tags=["regression", "api", "import"],
                            steps=[
                                step(
                                    "POST imports:preview then GET cases count.",
                                    "Preview returns draft summary; case count unchanged until POST imports.",
                                ),
                            ],
                        ),
                        case(
                            "Reject unknown importer kind",
                            title_uk="Відхилити невідомий тип імпортера",
                            desc_en="Negative validation.",
                            pre_en="Import API.",
                            priority="low",
                            tags=["regression", "api", "import", "negative"],
                            case_type="negative",
                            steps=[
                                step(
                                    "POST imports with importer=not-a-real-importer.",
                                    "400 error; no cases created.",
                                ),
                            ],
                        ),
                        case(
                            "List available importers",
                            title_uk="Список доступних імпортерів",
                            desc_en="GET /api/v1/importers includes csv,xray,zephyr,squash,testlink,testrail,qase,qtest,kiwi.",
                            pre_en="Authenticated user with import permission.",
                            priority="medium",
                            tags=["api", "import"],
                            steps=[
                                step(
                                    "GET /api/v1/importers.",
                                    "JSON lists expected importer ids including xray, squash, testlink.",
                                ),
                            ],
                        ),
                    ],
                ),
            ],
        ),
        # ── 13 LLM ──────────────────────────────────────────────────────
        suite(
            "13. LLM providers",
            "13. LLM-провайдери",
            cases=[
                case(
                    "Admin configures mock LLM provider",
                    title_uk="Admin налаштовує mock LLM-провайдер",
                    desc_en="Admin LLM tab saves mock provider for demos/tests.",
                    pre_en=f"System admin. {ADMIN}.",
                    priority="critical",
                    tags=["smoke", "regression", "e2e", "llm"],
                    external_key="e2e:integrations-llm.spec.ts",
                    steps=[
                        step(
                            "Open Admin → LLM; add/select mock provider; save project config.",
                            "Config listed; subsequent Generate/Chat use mock without network.",
                        ),
                    ],
                ),
                case(
                    "List LLM provider kinds",
                    title_uk="Список типів LLM-провайдерів",
                    desc_en="GET /api/v1/llm/providers returns mock, ollama/openai-compatible, openrouter, anthropic, gemini, openai, xai.",
                    pre_en="Authenticated admin/architect.",
                    priority="medium",
                    tags=["api", "llm"],
                    steps=[
                        step(
                            "GET /api/v1/llm/providers.",
                            "Response includes mock and at least one remote kind.",
                        ),
                    ],
                ),
                case(
                    "Reject LLM config with missing secrets for remote provider",
                    title_uk="Відхилити LLM-конфіг без секретів для remote",
                    desc_en="Negative validation.",
                    pre_en="Admin LLM form.",
                    priority="medium",
                    tags=["regression", "llm", "negative"],
                    case_type="negative",
                    steps=[
                        step(
                            "Save OpenAI-compatible config without API key.",
                            "Validation error; config not activated.",
                        ),
                    ],
                ),
                case(
                    "RAG over tests corpus",
                    title_uk="RAG по корпусу тестів",
                    desc_en="Planned: retrieval-augmented Q&A over cases.",
                    pre_en="N/A — architecture lists RAG as future.",
                    priority="low",
                    status="draft",
                    tags=["llm", "planned"],
                    case_type="functional",
                    steps=[
                        step(
                            "Attempt advanced RAG grounding controls in Chat (when shipped).",
                            "Citations to case keys appear in answers. Currently planned — mark blocked until shipped.",
                        ),
                    ],
                ),
            ],
        ),
        # ── 14 Chat & Generate ──────────────────────────────────────────
        suite(
            "14. Chat & Generate",
            "14. Chat і Generate",
            cases=[
                case(
                    "Generate drafts cases via Generate page",
                    title_uk="Generate створює чернетки кейсів",
                    desc_en="POST generate-cases with mock provider.",
                    pre_en="Mock LLM configured; user with llm_assist.",
                    priority="critical",
                    tags=["smoke", "regression", "e2e", "llm"],
                    external_key="e2e:llm-chat.spec.ts",
                    steps=[
                        step(
                            "Open /generate; enter a feature description; submit generate cases.",
                            "Draft cases returned/created; titles non-empty.",
                            "Feature: login rate limiting",
                        ),
                    ],
                ),
                case(
                    "Generate Playwright automation code",
                    title_uk="Generate генерує Playwright-код",
                    desc_en="generate-code with playwright target.",
                    pre_en="Mock LLM; existing case.",
                    priority="high",
                    tags=["regression", "e2e", "llm"],
                    external_key="e2e:llm-chat.spec.ts",
                    steps=[
                        step(
                            "On Generate, choose generate code / playwright for a case.",
                            "Code snippet returned containing test()/expect patterns.",
                        ),
                    ],
                ),
                case(
                    "Chat ask over project cases",
                    title_uk="Chat ask по кейсах проєкту",
                    desc_en="Chat page ask endpoint.",
                    pre_en="Mock LLM; project with cases (e.g. Vulyk QA).",
                    priority="critical",
                    tags=["smoke", "regression", "e2e", "llm"],
                    external_key="e2e:llm-chat.spec.ts",
                    steps=[
                        step(
                            "Open /chat; ask a question about existing cases.",
                            "Assistant reply visible; no unhandled error.",
                            "How many smoke-tagged cases exist?",
                        ),
                    ],
                ),
                case(
                    "Generate/Chat hidden for Viewer",
                    title_uk="Generate/Chat сховані для Viewer",
                    desc_en="RBAC gate.",
                    pre_en="viewer role.",
                    priority="medium",
                    tags=["regression", "rbac", "llm", "negative"],
                    case_type="negative",
                    steps=[
                        step(
                            "As viewer, inspect nav and POST generate-cases.",
                            "Nav hidden; API 403.",
                        ),
                    ],
                ),
            ],
        ),
        # ── 15 MCP ──────────────────────────────────────────────────────
        suite(
            "15. MCP",
            "15. MCP",
            cases=[
                case(
                    "MCP search_cases returns project cases",
                    title_uk="MCP search_cases повертає кейси проєкту",
                    desc_en="stdio/HTTP MCP tool search_cases.",
                    pre_en="MCP server running with API token; project key VULYK_QA.",
                    priority="high",
                    tags=["regression", "api", "mcp"],
                    steps=[
                        step(
                            "Call MCP tool search_cases with a query token from a known case title.",
                            "Results include matching VQA-* keys.",
                        ),
                    ],
                ),
                case(
                    "MCP create_case and get_case round-trip",
                    title_uk="MCP create_case і get_case round-trip",
                    desc_en="create_case then get_case by id/key.",
                    pre_en="MCP auth token with case create permission.",
                    priority="high",
                    tags=["regression", "mcp"],
                    steps=[
                        step(
                            "create_case with title; then get_case.",
                            "Returned case has auto key and same title.",
                        ),
                    ],
                ),
                case(
                    "MCP list_runs and ingest_results",
                    title_uk="MCP list_runs і ingest_results",
                    desc_en="Automation tools for runs.",
                    pre_en="Existing run; MCP token.",
                    priority="medium",
                    tags=["regression", "mcp", "ci"],
                    steps=[
                        step(
                            "list_runs; ingest_results for a case status.",
                            "Run list non-empty; result status updated.",
                        ),
                    ],
                ),
                case(
                    "MCP ask_tests uses LLM assist",
                    title_uk="MCP ask_tests використовує LLM",
                    desc_en="ask_tests tool.",
                    pre_en="Mock LLM + MCP.",
                    priority="medium",
                    tags=["regression", "mcp", "llm"],
                    steps=[
                        step(
                            "Call ask_tests with a natural language question.",
                            "Non-empty answer string returned.",
                        ),
                    ],
                ),
                case(
                    "MCP HTTP transport binds configured address",
                    title_uk="MCP HTTP-транспорт слухає налаштовану адресу",
                    desc_en="VULYK_MCP_TRANSPORT=http /mcp endpoint.",
                    pre_en="MCP configured for HTTP on VULYK_MCP_BIND.",
                    priority="low",
                    tags=["regression", "mcp"],
                    steps=[
                        step(
                            "GET or initialize MCP HTTP endpoint /mcp.",
                            "Server responds (not connection refused).",
                        ),
                    ],
                ),
            ],
        ),
        # ── 16 API tokens & OpenAPI ─────────────────────────────────────
        suite(
            "16. API tokens & OpenAPI",
            "16. API-токени та OpenAPI",
            cases=[
                case(
                    "Create personal API token",
                    title_uk="Створити персональний API-токен",
                    desc_en="Admin→Tokens creates vulyk_at_… secret shown once.",
                    pre_en="User with api_tokens permission (admin/TAE).",
                    priority="critical",
                    tags=["smoke", "regression", "e2e", "api", "tokens"],
                    steps=[
                        step(
                            "Open Admin → Tokens; create token with label.",
                            "Token value displayed once starting with vulyk_at_; list shows label without full secret.",
                        ),
                    ],
                ),
                case(
                    "Revoke API token rejects subsequent calls",
                    title_uk="Відкликаний токен відхиляє наступні виклики",
                    desc_en="Negative after revoke.",
                    pre_en="Valid token.",
                    priority="high",
                    tags=["regression", "api", "tokens", "negative"],
                    case_type="negative",
                    steps=[
                        step(
                            "Revoke token; GET /api/v1/projects with that Bearer.",
                            "401 Unauthorized.",
                        ),
                    ],
                ),
                case(
                    "OpenAPI Scalar docs available at /api/docs",
                    title_uk="OpenAPI Scalar-документація на /api/docs",
                    desc_en="API docs UI loads.",
                    pre_en="API running.",
                    priority="high",
                    tags=["smoke", "regression", "api", "openapi"],
                    steps=[
                        step(
                            "Open http://{api}/api/docs in browser (or GET).",
                            "Scalar/OpenAPI UI loads with paths like /api/v1/projects visible.",
                        ),
                    ],
                ),
                case(
                    "OpenAPI documents auth and cases endpoints",
                    title_uk="OpenAPI документує auth і cases",
                    desc_en="Spec includes major tags.",
                    pre_en="/api/docs or openapi JSON.",
                    priority="medium",
                    tags=["api", "openapi"],
                    steps=[
                        step(
                            "Inspect OpenAPI document for /api/v1/auth/login and cases routes.",
                            "Operations present with security schemes for bearer/token.",
                        ),
                    ],
                ),
            ],
        ),
        # ── 17 i18n ─────────────────────────────────────────────────────
        suite(
            "17. i18n",
            "17. i18n",
            cases=[
                case(
                    "Switch UI locale to Ukrainian",
                    title_uk="Перемкнути локаль UI на українську",
                    desc_en="Locale select updates nav labels to UK.",
                    pre_en="Signed-in user.",
                    priority="high",
                    tags=["regression", "e2e", "i18n"],
                    steps=[
                        step(
                            "Change locale to uk via locale control.",
                            "Primary nav shows Кейси / Прогони / Звіти (as applicable); html lang reflects uk.",
                        ),
                    ],
                ),
                case(
                    "Switch UI locale back to English",
                    title_uk="Повернути локаль UI на англійську",
                    desc_en="EN labels restore.",
                    pre_en="UI currently uk.",
                    priority="medium",
                    tags=["regression", "i18n"],
                    steps=[
                        step(
                            "Select locale en.",
                            "Nav shows Cases / Runs / Reports.",
                        ),
                    ],
                ),
                case(
                    "Instance default locale from setup applies to new sessions",
                    title_uk="Локаль інстансу з setup застосовується до нових сесій",
                    desc_en="default_locale setting.",
                    pre_en="Admin can set default locale.",
                    priority="low",
                    tags=["regression", "i18n", "setup"],
                    steps=[
                        step(
                            "Set default locale uk in Admin Settings; open fresh browser profile login.",
                            "Default strings prefer UK until user overrides.",
                        ),
                    ],
                ),
                case(
                    "Bilingual sample content picks locale",
                    title_uk="Двомовний sample-контент обирає локаль",
                    desc_en="Seeded Vulyk QA / Kyiv names follow instance locale at seed time.",
                    pre_en="Re-seed or inspect existing; locale was en at seed.",
                    priority="low",
                    tags=["regression", "i18n", "setup"],
                    steps=[
                        step(
                            "Inspect seeded project/suite names.",
                            "EN locale seed stores English names; UK locale seed stores Ukrainian names.",
                        ),
                    ],
                ),
            ],
        ),
        # ── 18 Appearance ───────────────────────────────────────────────
        suite(
            "18. Appearance",
            "18. Вигляд",
            cases=[
                case(
                    "Apply Light, Dark, High contrast, and Sepia themes",
                    title_uk="Застосувати теми Light, Dark, High contrast і Sepia",
                    desc_en="Appearance page theme choices persist on html data-theme.",
                    pre_en="Open /appearance.",
                    priority="critical",
                    tags=["smoke", "regression", "e2e", "appearance"],
                    external_key="e2e:appearance.spec.ts",
                    steps=[
                        step(
                            "Select each theme: Light, Dark, High contrast, Sepia.",
                            "html[data-theme] / data-theme-choice updates; UI remains readable; preference persists after reload.",
                        ),
                    ],
                ),
                case(
                    "System theme follows prefers-color-scheme",
                    title_uk="Системна тема слідує prefers-color-scheme",
                    desc_en="System option.",
                    pre_en="Appearance System selected.",
                    priority="medium",
                    tags=["regression", "appearance"],
                    steps=[
                        step(
                            "Emulate dark color scheme in browser; reload.",
                            "Effective theme is dark (unless high-contrast preference overrides).",
                        ),
                    ],
                ),
                case(
                    "Font scale and density controls",
                    title_uk="Масштаб шрифту і щільність",
                    desc_en="Font size steps and Comfortable/Compact.",
                    pre_en="/appearance.",
                    priority="medium",
                    tags=["regression", "e2e", "appearance"],
                    external_key="e2e:appearance.spec.ts",
                    steps=[
                        step(
                            "Increase font size; set density Compact.",
                            "data-density and font scaling CSS variables applied; layout usable.",
                        ),
                    ],
                ),
                case(
                    "Status badges show icon and label in all themes",
                    title_uk="Статусні бейджі з іконкою і підписом у всіх темах",
                    desc_en="Okabe–Ito palette accessibility.",
                    pre_en="Run with varied statuses; cycle themes.",
                    priority="high",
                    tags=["regression", "e2e", "appearance", "a11y"],
                    external_key="e2e:appearance.spec.ts",
                    steps=[
                        step(
                            "For each theme, inspect .status-pill elements.",
                            "Each pill has icon + text label (Passed/Failed/…); axe checks pass for theme page.",
                        ),
                    ],
                ),
                case(
                    "Reduced motion preference honoured",
                    title_uk="Повага до reduced motion",
                    desc_en="Appearance reduced motion setting.",
                    pre_en="/appearance.",
                    priority="low",
                    tags=["regression", "appearance", "a11y"],
                    steps=[
                        step(
                            "Set reduced motion to always reduce.",
                            "Animations minimized; setting persists.",
                        ),
                    ],
                ),
            ],
        ),
        # ── 19 PWA & mobile ─────────────────────────────────────────────
        suite(
            "19. PWA & mobile",
            "19. PWA і мобільний",
            cases=[
                case(
                    "Mobile bottom nav Cases/Runs/Reports/Admin",
                    title_uk="Мобільна нижня навігація Cases/Runs/Reports/Admin",
                    desc_en="Narrow viewport shows Mobile primary nav.",
                    pre_en="Chromium mobile viewport; signed in.",
                    priority="critical",
                    tags=["smoke", "regression", "e2e", "mobile", "pwa"],
                    external_key="e2e:pwa-mobile.spec.ts",
                    steps=[
                        step(
                            "Set viewport to mobile; locate navigation name Mobile primary.",
                            "Bottom nav links for Cases, Runs, Reports (Admin if permitted) work.",
                        ),
                    ],
                ),
                case(
                    "Suite drawer on mobile",
                    title_uk="Drawer тек на мобільному",
                    desc_en="#suite-drawer / folder access.",
                    pre_en="Mobile viewport; Cases page.",
                    priority="high",
                    tags=["regression", "e2e", "mobile", "folders"],
                    external_key="e2e:pwa-mobile.spec.ts",
                    steps=[
                        step(
                            "Open suite drawer; select a folder.",
                            "Drawer opens; selecting folder filters cases; backdrop dismisses.",
                        ),
                    ],
                ),
                case(
                    "Create case on mobile layout",
                    title_uk="Створити кейс у мобільному layout",
                    desc_en="Create case usable on small screen.",
                    pre_en="Mobile viewport; permission to create.",
                    priority="high",
                    tags=["regression", "e2e", "mobile"],
                    external_key="e2e:pwa-mobile.spec.ts",
                    steps=[
                        step(
                            "Create case with title on mobile.",
                            "Case appears in list; no overlapping controls blocking submit.",
                        ),
                    ],
                ),
                case(
                    "Web app manifest and service worker register",
                    title_uk="Маніфест PWA і реєстрація service worker",
                    desc_en="manifest + SW for installability.",
                    pre_en="Frontend built with PWA assets.",
                    priority="high",
                    tags=["regression", "e2e", "pwa"],
                    external_key="e2e:pwa-mobile.spec.ts",
                    steps=[
                        step(
                            "GET manifest.webmanifest (or link rel=manifest); check serviceWorker ready.",
                            "Manifest has name/icons; SW registered.",
                        ),
                    ],
                ),
                case(
                    "Offline fallback page is available",
                    title_uk="Офлайн fallback-сторінка доступна",
                    desc_en="offline.html served when offline.",
                    pre_en="SW active.",
                    priority="medium",
                    tags=["regression", "e2e", "pwa"],
                    external_key="e2e:pwa-mobile.spec.ts",
                    steps=[
                        step(
                            "Open offline.html or go offline and navigate.",
                            "Offline page renders with recoverable messaging.",
                        ),
                    ],
                ),
                case(
                    "Offline queue syncs step results when back online",
                    title_uk="Офлайн-черга синхронізує результати кроків онлайн",
                    desc_en="IndexedDB queue for results.",
                    pre_en="PWA; run open; go offline.",
                    priority="medium",
                    tags=["regression", "pwa", "runs"],
                    steps=[
                        step(
                            "While offline, mark a result; go online.",
                            "Queued result syncs; server status matches.",
                        ),
                    ],
                ),
            ],
        ),
        # ── 20 Performance ──────────────────────────────────────────────
        suite(
            "20. Performance",
            "20. Продуктивність",
            cases=[
                case(
                    "Cases list remains usable with ~20k cases",
                    title_uk="Список кейсів лишається робочим при ~20k кейсів",
                    desc_en="Pagination/virtualization: list/search must respond under large project.",
                    pre_en="Project populated with ~20 000 cases (perf fixture or generated). May be heavy for CI — run in perf env.",
                    priority="high",
                    tags=["performance", "cases", "regression"],
                    case_type="performance",
                    steps=[
                        step(
                            "Open Cases for the large project; wait for first page.",
                            "First contentful list within acceptable budget (e.g. <3s on CI hardware); UI remains interactive.",
                        ),
                        step(
                            "Paginate or scroll to load more.",
                            "Subsequent pages load without browser freeze; memory stable enough to continue.",
                        ),
                    ],
                ),
                case(
                    "FTS search on 20k cases returns quickly",
                    title_uk="FTS-пошук по 20k кейсах швидкий",
                    desc_en="Search API/UI latency under load.",
                    pre_en="~20k cases; distinctive token in a subset.",
                    priority="high",
                    tags=["performance", "cases", "api"],
                    case_type="performance",
                    steps=[
                        step(
                            "Search for a selective token via UI or GET cases?q=.",
                            "Results return within budget (e.g. <2s); hit count correct for the token.",
                        ),
                    ],
                ),
                case(
                    "Folder tree with deep nesting stays responsive",
                    title_uk="Дерево тек з глибокою вкладеністю лишається чуйним",
                    desc_en="Edge: many nested suites.",
                    pre_en="Project with deep folder hierarchy (≥5 levels) and counts.",
                    priority="medium",
                    tags=["performance", "folders", "edge"],
                    case_type="edge",
                    steps=[
                        step(
                            "Expand nested folders in .folder-tree.",
                            "Expand/collapse remains responsive; selecting leaf filters cases.",
                        ),
                    ],
                ),
                case(
                    "Cursor pagination for huge lists",
                    title_uk="Cursor-пагінація для великих списків",
                    desc_en="Planned architecture item for large collections.",
                    pre_en="N/A until cursor pagination ships.",
                    priority="low",
                    status="draft",
                    tags=["performance", "planned", "api"],
                    case_type="performance",
                    steps=[
                        step(
                            "Request cases with cursor-based page token (when API supports it).",
                            "Stable ordering without offset drift. Currently planned.",
                        ),
                    ],
                ),
            ],
        ),
    ]

    # Extra edge/negative cases sprinkled to reach ~150+ quality total — add under Auth/API misc
    misc = suite(
        "21. Platform & health",
        "21. Платформа і health",
        cases=[
            case(
                "Health endpoint returns ok",
                title_uk="Health-ендпоінт повертає ok",
                desc_en="GET /health for probes.",
                pre_en="API up.",
                priority="critical",
                tags=["smoke", "api", "platform"],
                steps=[step("GET /health.", "200 JSON status ok.")],
            ),
            case(
                "Ready endpoint checks database",
                title_uk="Ready-ендпоінт перевіряє БД",
                desc_en="GET /ready.",
                pre_en="API+Postgres up.",
                priority="high",
                tags=["smoke", "api", "platform"],
                steps=[step("GET /ready.", "200 when DB reachable; non-ready when DB down.")],
            ),
            case(
                "Roles matrix endpoint documents permissions",
                title_uk="Ендпоінт матриці ролей документує дозволи",
                desc_en="GET /api/v1/roles/matrix.",
                pre_en="Authenticated.",
                priority="low",
                tags=["api", "rbac"],
                steps=[
                    step(
                        "GET /api/v1/roles/matrix.",
                        "Matrix includes Admin, Test Architect, Test Automation Engineer, Developer, Viewer.",
                    )
                ],
            ),
            case(
                "Custom field definitions UI",
                title_uk="UI визначення кастомних полів",
                desc_en="API supports custom fields; dedicated admin UI to define schemas is partial/planned.",
                pre_en="N/A.",
                priority="low",
                status="draft",
                tags=["cases", "planned", "admin"],
                steps=[
                    step(
                        "Open admin UI to define custom field schemas (when shipped).",
                        "Fields appear on case detail for values. Currently API-only — planned UI.",
                    )
                ],
            ),
            case(
                "Result partitioning for very large histories",
                title_uk="Partitioning результатів для великих історій",
                desc_en="Planned scalability item from architecture.",
                pre_en="N/A.",
                priority="low",
                status="draft",
                tags=["planned", "performance", "runs"],
                case_type="performance",
                steps=[
                    step(
                        "Verify partitioned storage for test_results when implemented.",
                        "Queries remain fast on huge histories. Currently planned.",
                    )
                ],
            ),
            case(
                "GitHub Action uploads results to Vulyk",
                title_uk="GitHub Action завантажує результати в Vulyk",
                desc_en="action/action.yml reporter integration.",
                pre_en="Workflow using Vulyk action with token secrets.",
                priority="medium",
                tags=["regression", "ci"],
                steps=[
                    step(
                        "Run workflow job that invokes the Vulyk GitHub Action with JUnit path.",
                        "Action exits 0; run in Vulyk shows ingested results.",
                    )
                ],
            ),
            case(
                "Webhook receiver accepts GitHub/GitLab payloads",
                title_uk="Webhook приймає payload GitHub/GitLab",
                desc_en="POST /api/v1/webhooks/github|gitlab.",
                pre_en="CI config with webhook secret.",
                priority="medium",
                tags=["regression", "api", "ci"],
                steps=[
                    step(
                        "POST signed webhook payload to github/gitlab endpoint.",
                        "200/202; invalid signature rejected.",
                    )
                ],
            ),
            case(
                "Admin global integrations placeholders",
                title_uk="Глобальні інтеграції адміна (placeholders)",
                desc_en="Admin → Integrations can add placeholder configs.",
                pre_en="System admin.",
                priority="low",
                tags=["regression", "e2e", "integrations"],
                external_key="e2e:integrations-llm.spec.ts",
                steps=[
                    step(
                        "Open Admin Integrations; add placeholder/fixture integration.",
                        "Config saved and listed.",
                    )
                ],
            ),
            case(
                "Defect create from failed result via ALM",
                title_uk="Створення дефекту з failed-результату через ALM",
                desc_en="Create defect linked to result when integration configured.",
                pre_en="Jira/OpenProject integration; failed result.",
                priority="medium",
                tags=["regression", "integrations", "runs"],
                steps=[
                    step(
                        "From failed result, create defect.",
                        "Defect external_key returned; link stored on result.",
                    )
                ],
            ),
            case(
                "Retest status available after Failed",
                title_uk="Статус Retest доступний після Failed",
                desc_en="Execution status Retest in addition to Passed/Failed/Blocked/Skipped.",
                pre_en="Run item previously Failed.",
                priority="medium",
                tags=["regression", "e2e", "runs"],
                steps=[
                    step(
                        "Set result status to Retest.",
                        "Status pill Retest; history includes Retest entry.",
                    )
                ],
            ),
            case(
                "Step-level results within a run item",
                title_uk="Результати на рівні кроків у елементі прогону",
                desc_en="Record per-step pass/fail when case has steps.",
                pre_en="Case with steps in an active run.",
                priority="medium",
                tags=["regression", "runs"],
                steps=[
                    step(
                        "Mark individual steps passed/failed in run UI.",
                        "Step results persisted; overall status can derive or stay manual.",
                    )
                ],
            ),
            case(
                "Assign run item to a user",
                title_uk="Призначити елемент прогону користувачу",
                desc_en="Assignee field on run items.",
                pre_en="Run; another project member.",
                priority="low",
                tags=["regression", "runs"],
                steps=[
                    step(
                        "Set assignee on a run item.",
                        "Assignee displayed; filterable if UI supports.",
                    )
                ],
            ),
            case(
                "Project switcher changes active project context",
                title_uk="Перемикач проєктів змінює активний контекст",
                desc_en=".project-select switches cases/runs scope.",
                pre_en="User member of ≥2 projects (Kyiv + Vulyk QA).",
                priority="high",
                tags=["smoke", "regression", "e2e", "projects"],
                external_key="e2e:sample-project.spec.ts",
                steps=[
                    step(
                        "Select Kyiv Open-Air Concert 2026 then Vulyk QA in .project-select.",
                        "Folder tree and case keys switch (KC-* vs VQA-*).",
                    )
                ],
            ),
            case(
                "Demo user membership on sample projects is TAE",
                title_uk="Demo-користувач має роль TAE на sample-проєктах",
                desc_en="Seeder grants test_automation_engineer to demo email.",
                pre_en=f"{DEMO}; samples seeded.",
                priority="medium",
                tags=["regression", "api", "setup", "rbac"],
                steps=[
                    step(
                        "As demo user, GET memberships / attempt create case on VULYK_QA.",
                        "Create case allowed; delete case forbidden (403).",
                    )
                ],
            ),
            case(
                "Idempotent re-seed does not duplicate sample projects",
                title_uk="Ідемпотентний повторний сід не дублює sample-проєкти",
                desc_en="Second boot with VULYK_SEED_SAMPLE=1 skips existing keys.",
                pre_en="Samples already present; restart API.",
                priority="high",
                tags=["smoke", "regression", "api", "setup"],
                steps=[
                    step(
                        "Restart API (seed::run); count projects with keys KYIV_CONCERT_2026 and VULYK_QA.",
                        "Exactly one of each; case counts unchanged.",
                    )
                ],
            ),
        ],
    )
    suites.append(misc)

    requirements = [
        {
            "external_key": "VQA-REQ-01",
            "title": L("First-run setup & sample seeding", "Перший запуск і сід sample"),
            "source": "sample",
            "case_keys": [f"VQA-{i:03d}" for i in (1, 3, 6, 7)],
        },
        {
            "external_key": "VQA-REQ-02",
            "title": L("Authentication & SSO", "Автентифікація і SSO"),
            "source": "sample",
            "case_keys": [f"VQA-{i:03d}" for i in (8, 9, 11, 14, 16, 17)],
        },
        {
            "external_key": "VQA-REQ-03",
            "title": L("RBAC for all system roles", "RBAC для всіх ролей"),
            "source": "sample",
            "case_keys": [f"VQA-{i:03d}" for i in (19, 22, 25, 27, 29)],
        },
        {
            "external_key": "VQA-REQ-04",
            "title": L("Projects, keys, folders, cases", "Проєкти, ключі, теки, кейси"),
            "source": "sample",
            "case_keys": [f"VQA-{i:03d}" for i in (31, 35, 38, 45, 50)],
        },
        {
            "external_key": "VQA-REQ-05",
            "title": L("Runs, automation, reports, import", "Прогони, автоматизація, звіти, імпорт"),
            "source": "sample",
            "case_keys": [f"VQA-{i:03d}" for i in (58, 60, 64, 70, 75)],
        },
        {
            "external_key": "VQA-REQ-06",
            "title": L("LLM, Chat, Generate, MCP", "LLM, Chat, Generate, MCP"),
            "source": "sample",
            "case_keys": [f"VQA-{i:03d}" for i in (81, 85, 87, 89)],
        },
        {
            "external_key": "VQA-REQ-07",
            "title": L("i18n, appearance, PWA, performance", "i18n, вигляд, PWA, продуктивність"),
            "source": "sample",
            "case_keys": [f"VQA-{i:03d}" for i in (97, 101, 106, 112)],
        },
    ]

    runs = [
        {
            "name": L("Vulyk QA smoke", "Vulyk QA smoke"),
            "description": L(
                "Smoke subset across setup, auth, cases, runs.",
                "Smoke-підмножина: setup, auth, cases, runs.",
            ),
            "status": "completed",
            "results": [
                {"case_key": "VQA-001", "status": "Passed", "comment": L("Wizard path OK")},
                {"case_key": "VQA-006", "status": "Passed", "comment": L("Both samples present")},
                {"case_key": "VQA-008", "status": "Passed", "comment": L("Admin login")},
                {"case_key": "VQA-017", "status": "Passed", "comment": L("Logout OK")},
                {"case_key": "VQA-029", "status": "Passed", "comment": L("Viewer gated")},
                {"case_key": "VQA-031", "status": "Passed"},
                {"case_key": "VQA-035", "status": "Passed"},
                {"case_key": "VQA-045", "status": "Passed"},
                {"case_key": "VQA-050", "status": "Passed"},
                {"case_key": "VQA-058", "status": "Passed"},
                {"case_key": "VQA-060", "status": "Passed"},
                {"case_key": "VQA-064", "status": "Passed"},
                {
                    "case_key": "VQA-009",
                    "status": "Passed",
                    "history": [
                        {"status": "Failed", "message": L("Wrong password attempt documented")},
                        {"status": "Passed", "message": L("Negative assertion held")},
                    ],
                },
            ],
        },
        {
            "name": L("Vulyk QA platform regression", "Vulyk QA platform regression"),
            "description": L(
                "In-progress broader regression including LLM and import.",
                "Ширша регресія в процесі: LLM та імпорт.",
            ),
            "status": "in_progress",
            "results": [
                {"case_key": "VQA-075", "status": "Passed"},
                {"case_key": "VQA-076", "status": "Passed"},
                {"case_key": "VQA-081", "status": "Passed"},
                {"case_key": "VQA-085", "status": "Untested"},
                {"case_key": "VQA-087", "status": "Untested"},
                {"case_key": "VQA-093", "status": "Passed"},
                {"case_key": "VQA-101", "status": "Passed"},
                {"case_key": "VQA-106", "status": "Blocked", "comment": L("Awaiting mobile device lab")},
            ],
        },
    ]

    return {
        "version": 1,
        "project": {
            "key": "VULYK_QA",
            "name": L("Vulyk QA", "Vulyk QA"),
            "description": L(
                "Pre-seeded QA project covering Vulyk's own product surfaces: setup, auth, RBAC, projects, cases, runs, import, LLM, MCP, i18n, appearance, PWA, and performance. Case keys use prefix VQA. Linked to Playwright specs via external_key where covered.",
                "Попередньо засіяний QA-проєкт, що покриває функціональність самого Vulyk: setup, auth, RBAC, проєкти, кейси, прогони, імпорт, LLM, MCP, i18n, вигляд, PWA та продуктивність. Ключі з префіксом VQA. Прив'язка до Playwright через external_key.",
            ),
        },
        "suites": suites,
        "requirements": requirements,
        "runs": runs,
    }


def count_cases(suites) -> list[tuple[str, int]]:
    rows = []

    def walk(s, path):
        name = s["name"]["en"]
        p = f"{path} / {name}" if path else name
        n = len(s.get("cases") or [])
        child_n = 0
        for ch in s.get("children") or []:
            child_n += walk(ch, p)
        total = n + child_n
        if n or not (s.get("children") or []):
            rows.append((p, n, total))
        return total

    for s in suites:
        walk(s, "")
    return rows


def main() -> None:
    data = build()
    # Fix requirement case_keys to only existing keys
    all_keys = []

    def collect(suites):
        for s in suites:
            for c in s.get("cases") or []:
                all_keys.append(c["key"])
            collect(s.get("children") or [])

    collect(data["suites"])
    keyset = set(all_keys)
    for req in data["requirements"]:
        req["case_keys"] = [k for k in req["case_keys"] if k in keyset]
        # Ensure at least a few links: pick by area prefix numbers if filtered empty
        if len(req["case_keys"]) < 2:
            # attach first smoke cases as fallback — should not happen if indices match
            req["case_keys"] = [k for k in all_keys if k in keyset][:3]

    # Validate
    assert len(all_keys) == len(set(all_keys)), "duplicate keys"
    assert 120 <= len(all_keys) <= 200, f"case count {len(all_keys)} out of range"
    for s in data["suites"]:
        def check(suite):
            for c in suite.get("cases") or []:
                assert c["steps"], c["key"]
                for st in c["steps"]:
                    assert st["expected_result"]["en"].strip(), c["key"]
            for ch in suite.get("children") or []:
                check(ch)
        check(s)

    OUT.write_text(json.dumps(data, ensure_ascii=False, indent=2) + "\n", encoding="utf-8")
    print(f"Wrote {OUT} with {len(all_keys)} cases")
    # Area totals (top-level)
    for s in data["suites"]:
        def total(suite):
            n = len(suite.get("cases") or [])
            for ch in suite.get("children") or []:
                n += total(ch)
            return n
        print(f"  {s['name']['en']}: {total(s)}")


if __name__ == "__main__":
    main()
