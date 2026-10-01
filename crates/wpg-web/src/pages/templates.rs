//! `/templates`: search, tag / source / favourite filters, recent, duplicate,
//! delete, export, import, YAML view / edit with lint.

use std::collections::BTreeSet;

use leptos::html;
use leptos::prelude::*;

use crate::api::{self, ApiError, LintResponse, TemplateQuery, TemplateSummary};
use crate::components::{
    use_toasts, Confirm, Empty, ErrorBox, Icon, IconName, Loading, Modal, PageHeader, ViewOnlyNote,
    BTN, BTN_GHOST, BTN_PRIMARY, INPUT,
};
use crate::i18n::{tr, use_i18n};
use crate::ws::use_live;

#[component]
pub(crate) fn TemplatesPage() -> impl IntoView {
    let i18n = use_i18n();
    let live = use_live();
    let toasts = use_toasts();
    let q = RwSignal::new(String::new());
    let tag = RwSignal::new(None::<String>);
    let source = RwSignal::new(None::<String>);
    let favorites = RwSignal::new(false);
    let reload = RwSignal::new(0u32);

    let list = LocalResource::new(move || {
        live.templates_rev.track();
        reload.track();
        let query = TemplateQuery {
            q: q.get(),
            tag: tag.get(),
            favorite: favorites.get(),
            source: source.get(),
        };
        async move { api::templates(query).await }
    });
    // Tag chips from the unfiltered library.
    let all = LocalResource::new(move || {
        live.templates_rev.track();
        reload.track();
        api::templates(TemplateQuery::default())
    });
    let tags = move || {
        all.get()
            .and_then(Result::ok)
            .map(|l| {
                l.templates
                    .iter()
                    .flat_map(|t| t.tags.iter().cloned())
                    .collect::<BTreeSet<_>>()
            })
            .unwrap_or_default()
    };
    let recent = move || {
        all.get()
            .and_then(Result::ok)
            .map(|l| {
                l.recent
                    .iter()
                    .filter_map(|id| l.templates.iter().find(|t| &t.id == id).cloned())
                    .take(6)
                    .collect::<Vec<_>>()
            })
            .unwrap_or_default()
    };

    let editing = RwSignal::new(None::<TemplateSummary>);
    let deleting = RwSignal::new(None::<TemplateSummary>);
    let busy = RwSignal::new(false);
    let refresh = move || reload.update(|r| *r += 1);

    // Import.
    let file_input = NodeRef::<html::Input>::new();
    let on_file = move |_| {
        #[cfg(target_arch = "wasm32")]
        if let Some(input) = file_input.get() {
            leptos::task::spawn_local(async move {
                if let Some((_, text)) = crate::dom::read_first_file(&input).await {
                    match api::import_template(text).await {
                        Ok(d) => {
                            toasts.ok(i18n.ta("ui-tpl-imported", &[("id", d.summary.name)]));
                            refresh();
                        }
                        Err(e) => toasts.error(e.message(i18n)),
                    }
                }
                input.set_value("");
            });
        }
    };

    let act = move |id: String, what: &'static str| {
        leptos::task::spawn_local(async move {
            let r = match what {
                "duplicate" => api::duplicate_template(&id).await.map(|d| {
                    toasts.ok(i18n.ta("ui-tpl-duplicated", &[("id", d.summary.name)]));
                }),
                _ => Ok(()),
            };
            match r {
                Ok(()) => refresh(),
                Err(e) => toasts.error(e.message(i18n)),
            }
        });
    };
    let toggle_fav = move |t: TemplateSummary| {
        leptos::task::spawn_local(async move {
            match api::set_favorite(&t.id, !t.favorite).await {
                Ok(()) => refresh(),
                Err(e) => toasts.error(e.message(i18n)),
            }
        });
    };
    let do_delete = move || {
        let Some(t) = deleting.get_untracked() else {
            return;
        };
        busy.set(true);
        leptos::task::spawn_local(async move {
            match api::delete_template(&t.id).await {
                Ok(()) => {
                    toasts.ok(i18n.ta("ui-tpl-deleted", &[("id", t.name.clone())]));
                    refresh();
                }
                Err(e) => toasts.error(e.message(i18n)),
            }
            busy.set(false);
            deleting.set(None);
        });
    };

    let source_btn = move |value: Option<&'static str>, key: &'static str| {
        let v = value.map(str::to_owned);
        let v2 = v.clone();
        view! {
            <button
                type="button"
                aria-pressed=move || (source.get() == v).to_string()
                class="rounded-full border border-slate-300 px-2.5 py-0.5 text-xs aria-pressed:border-sky-600 aria-pressed:bg-sky-600 aria-pressed:text-white dark:border-slate-700"
                on:click=move |_| source.set(v2.clone())
            >
                {tr(key)}
            </button>
        }
    };

    view! {
        <PageHeader title=tr("ui-nav-templates")>
            <input
                node_ref=file_input
                id="tpl-import"
                type="file"
                accept=".yaml,.yml,.json"
                class="sr-only"
                on:change=on_file
            />
            <label for="tpl-import" class=format!("{BTN} cursor-pointer max-md:hidden")>
                <Icon name=IconName::Upload class="h-4 w-4" />
                {tr("ui-tpl-import")}
            </label>
            <a href="/builder" class=format!("{BTN_PRIMARY} max-md:hidden")>
                <Icon name=IconName::Plus class="h-4 w-4" />
                {tr("ui-tpl-new")}
            </a>
        </PageHeader>
        <ViewOnlyNote />

        // Filters
        <div class="mb-4 space-y-3">
            <div class="flex flex-wrap items-center gap-3">
                <div class="relative min-w-60 flex-1">
                    <span class="pointer-events-none absolute inset-y-0 left-2 flex items-center text-slate-400">
                        <Icon name=IconName::Search class="h-4 w-4" />
                    </span>
                    <input
                        type="search"
                        class=format!("{INPUT} pl-8")
                        placeholder=tr("ui-tpl-search")
                        aria-label=tr("ui-tpl-search")
                        prop:value=move || q.get()
                        on:input=move |ev| q.set(event_target_value(&ev))
                    />
                </div>
                <div class="flex items-center gap-1.5" role="group" aria-label=tr("ui-tpl-source")>
                    {source_btn(None, "ui-tpl-source-all")}
                    {source_btn(Some("library"), "ui-tpl-source-library")}
                    {source_btn(Some("user"), "ui-tpl-source-user")}
                </div>
                <label class="flex items-center gap-1.5 text-sm">
                    <input
                        type="checkbox"
                        prop:checked=move || favorites.get()
                        on:change=move |ev| favorites.set(event_target_checked(&ev))
                    />
                    {tr("ui-tpl-favorites-only")}
                </label>
            </div>
            <div class="flex flex-wrap items-center gap-1.5" role="group" aria-label=tr("ui-tpl-tags")>
                <span class="text-xs text-slate-500">{tr("ui-tpl-tags")}</span>
                {move || {
                    tags()
                        .into_iter()
                        .map(|t| {
                            let t2 = t.clone();
                            let t3 = t.clone();
                            view! {
                                <button
                                    type="button"
                                    aria-pressed=move || (tag.get().as_deref() == Some(t2.as_str())).to_string()
                                    class="rounded-full bg-slate-100 px-2 py-0.5 text-xs text-slate-700 hover:bg-slate-200 aria-pressed:bg-sky-600 aria-pressed:text-white dark:bg-slate-800 dark:text-slate-300"
                                    on:click=move |_| {
                                        tag.update(|cur| {
                                            *cur = if cur.as_deref() == Some(t3.as_str()) { None } else { Some(t3.clone()) };
                                        });
                                    }
                                >
                                    {"#"}{t}
                                </button>
                            }
                        })
                        .collect_view()
                }}
            </div>
            <Show when=move || !recent().is_empty()>
                <div class="flex flex-wrap items-center gap-1.5 text-xs">
                    <span class="text-slate-500">{tr("ui-tpl-recent")}</span>
                    {move || {
                        recent()
                            .into_iter()
                            .map(|t| {
                                let t2 = t.clone();
                                view! {
                                    <button
                                        type="button"
                                        class="rounded border border-slate-200 px-1.5 py-0.5 hover:border-sky-500 dark:border-slate-700"
                                        on:click=move |_| editing.set(Some(t2.clone()))
                                    >
                                        {t.name}
                                    </button>
                                }
                            })
                            .collect_view()
                    }}
                </div>
            </Show>
        </div>

        {move || match list.get() {
            None => view! { <Loading /> }.into_any(),
            Some(Err(e)) => view! { <ErrorBox error=e /> }.into_any(),
            Some(Ok(l)) if l.templates.is_empty() => view! { <Empty text=tr("ui-tpl-empty") /> }.into_any(),
            Some(Ok(l)) => {
                view! {
                    <ul class="grid gap-3 md:grid-cols-2 2xl:grid-cols-3">
                        {l.templates
                            .into_iter()
                            .map(|t| {
                                let fav = t.favorite;
                                let t_fav = t.clone();
                                let t_edit = t.clone();
                                let t_del = t.clone();
                                let id_dup = t.id.clone();
                                let badge = if t.source == "user" {
                                    "bg-violet-100 text-violet-800 dark:bg-violet-500/15 dark:text-violet-300"
                                } else {
                                    "bg-slate-100 text-slate-700 dark:bg-slate-700/50 dark:text-slate-300"
                                };
                                let source_key = if t.source == "user" { "ui-tpl-source-user" } else { "ui-tpl-source-library" };
                                view! {
                                    <li class="flex flex-col rounded-lg border border-slate-200 bg-white p-4 shadow-sm dark:border-slate-800 dark:bg-slate-900">
                                        <div class="flex items-start gap-2">
                                            <button
                                                type="button"
                                                class=move || if fav { "text-amber-500 hover:text-amber-600" } else { "text-slate-300 hover:text-amber-500 dark:text-slate-600" }
                                                aria-pressed=fav.to_string()
                                                aria-label=tr("ui-tpl-favorite")
                                                title=tr("ui-tpl-favorite")
                                                on:click=move |_| toggle_fav(t_fav.clone())
                                            >
                                                <Icon name=IconName::Star filled=fav />
                                            </button>
                                            <div class="min-w-0 flex-1">
                                                <h2 class="truncate font-medium">{t.name.clone()}</h2>
                                                <p class="truncate font-mono text-xs text-slate-400">{t.path.clone()}</p>
                                            </div>
                                            <span class=format!("shrink-0 rounded px-1.5 py-0.5 text-[11px] font-medium {badge}")>
                                                {move || i18n.t(source_key)}
                                            </span>
                                        </div>
                                        <p class="mt-2 line-clamp-2 text-sm text-slate-600 dark:text-slate-400">
                                            {t.description.clone().unwrap_or_default()}
                                        </p>
                                        {t.error.clone().map(|e| view! { <p class="mt-1 text-xs text-rose-600" role="alert">{e}</p> })}
                                        <p class="mt-2 truncate font-mono text-xs text-slate-500">{t.protocols.join(" / ")}</p>
                                        <div class="mt-1 flex flex-wrap gap-1">
                                            {t.tags.iter().map(|g| view! { <span class="text-xs text-sky-700 dark:text-sky-400">{format!("#{g}")}</span> }).collect_view()}
                                        </div>
                                        <div class="mt-3 flex flex-wrap items-center gap-1 border-t border-slate-100 pt-2 dark:border-slate-800">
                                            <button type="button" class=BTN_GHOST title=tr("ui-tpl-view") aria-label=tr("ui-tpl-view") on:click=move |_| editing.set(Some(t_edit.clone()))>
                                                <Icon name=IconName::Edit class="h-4 w-4" />
                                            </button>
                                            <a class=BTN_GHOST href=format!("/builder?template={}", api::enc(&t.id)) title=tr("ui-tpl-open-builder") aria-label=tr("ui-tpl-open-builder")>
                                                <Icon name=IconName::Builder class="h-4 w-4" />
                                            </a>
                                            <a class=BTN_GHOST href=format!("/jobs?new={}", api::enc(&t.id)) title=tr("ui-tpl-new-job") aria-label=tr("ui-tpl-new-job")>
                                                <Icon name=IconName::Play class="h-4 w-4" />
                                            </a>
                                            <a class=BTN_GHOST href=api::export_url(&t.id) download="" rel="external" title=tr("ui-tpl-export") aria-label=tr("ui-tpl-export")>
                                                <Icon name=IconName::Download class="h-4 w-4" />
                                            </a>
                                            <button type="button" class=format!("{BTN_GHOST} max-md:hidden") title=tr("ui-tpl-duplicate") aria-label=tr("ui-tpl-duplicate") on:click=move |_| act(id_dup.clone(), "duplicate")>
                                                <Icon name=IconName::Copy class="h-4 w-4" />
                                            </button>
                                            {t.writable.then(|| view! {
                                                <button type="button" class=format!("{BTN_GHOST} ml-auto hover:text-rose-600 max-md:hidden") title=tr("ui-delete") aria-label=tr("ui-delete") on:click=move |_| deleting.set(Some(t_del.clone()))>
                                                    <Icon name=IconName::Trash class="h-4 w-4" />
                                                </button>
                                            })}
                                        </div>
                                    </li>
                                }
                            })
                            .collect_view()}
                    </ul>
                }
                    .into_any()
            }
        }}

        {move || editing.get().map(|t| view! {
            <YamlEditor summary=t on_close=move || editing.set(None) on_saved=move || refresh() />
        })}
        <Show when=move || deleting.get().is_some()>
            <Confirm
                title=tr("ui-tpl-delete-title")
                confirm_label=tr("ui-delete")
                danger=true
                busy=busy
                on_confirm=move || do_delete()
                on_cancel=move || deleting.set(None)
            >
                <p>{move || i18n.ta("ui-tpl-delete-body", &[("id", deleting.get().map(|t| t.name).unwrap_or_default())])}</p>
            </Confirm>
        </Show>
    }
}

/// YAML view / edit with lint (`POST /api/lint`) and save (`PUT`).
#[component]
fn YamlEditor(
    summary: TemplateSummary,
    #[prop(into)] on_close: Callback<()>,
    #[prop(into)] on_saved: Callback<()>,
) -> impl IntoView {
    let i18n = use_i18n();
    let toasts = use_toasts();
    let id = StoredValue::new(summary.id.clone());
    let writable = summary.writable;
    let text = RwSignal::new(None::<String>);
    let load_error = RwSignal::new(None::<ApiError>);
    let lint = RwSignal::new(None::<Result<LintResponse, ApiError>>);
    let save_error = RwSignal::new(None::<ApiError>);
    let busy = RwSignal::new(false);
    let dirty = RwSignal::new(false);

    leptos::task::spawn_local(async move {
        match api::export_template(&id.get_value()).await {
            Ok(t) => text.set(Some(t)),
            Err(e) => load_error.set(Some(e)),
        }
    });

    let run_lint = move || {
        let Some(t) = text.get_untracked() else {
            return;
        };
        busy.set(true);
        leptos::task::spawn_local(async move {
            lint.set(Some(api::lint_yaml(&t).await));
            busy.set(false);
        });
    };
    let save = move || {
        let Some(t) = text.get_untracked() else {
            return;
        };
        busy.set(true);
        save_error.set(None);
        leptos::task::spawn_local(async move {
            match api::update_template_yaml(&id.get_value(), &t).await {
                Ok(_) => {
                    dirty.set(false);
                    toasts.ok(i18n.t("ui-tpl-saved"));
                    on_saved.run(());
                }
                Err(e) => save_error.set(Some(e)),
            }
            busy.set(false);
        });
    };

    view! {
        <Modal title=summary.name.clone() on_close=on_close wide=true>
            {move || load_error.get().map(|e| view! { <ErrorBox error=e /> })}
            {move || match text.get() {
                None if load_error.with(Option::is_none) => view! { <Loading /> }.into_any(),
                None => ().into_any(),
                Some(_) => view! {
                    <label for="tpl-yaml" class="sr-only">{tr("ui-tpl-yaml")}</label>
                    <textarea
                        id="tpl-yaml"
                        class="h-[50vh] w-full resize-y rounded-md border border-slate-300 bg-slate-50 p-3 font-mono text-xs leading-5 dark:border-slate-700 dark:bg-slate-950"
                        spellcheck="false"
                        readonly=!writable
                        prop:value=move || text.get().unwrap_or_default()
                        on:input=move |ev| {
                            text.set(Some(event_target_value(&ev)));
                            dirty.set(true);
                        }
                    ></textarea>
                }.into_any(),
            }}
            {(!writable).then(|| view! { <p class="mt-1 text-xs text-slate-500">{tr("ui-tpl-read-only")}</p> })}
            {move || lint.get().map(|r| match r {
                Err(e) => view! { <ErrorBox error=e /> }.into_any(),
                Ok(l) if l.issues.is_empty() => view! {
                    <p class="mt-3 flex items-center gap-1 text-sm text-emerald-700 dark:text-emerald-400" role="status">
                        <Icon name=IconName::Check class="h-4 w-4" />
                        {tr("ui-lint-clean")}
                    </p>
                }.into_any(),
                Ok(l) => view! {
                    <div class="mt-3" role="status">
                        <p class="text-sm font-medium">
                            {move || i18n.ta("ui-lint-summary", &[("errors", l.errors.to_string()), ("warnings", l.warnings.to_string())])}
                        </p>
                        <ul class="mt-1 space-y-1 text-xs">
                            {l.issues.clone().into_iter().map(|i| {
                                let cls = if i.level == "error" { "text-rose-600 dark:text-rose-400" } else { "text-amber-600 dark:text-amber-400" };
                                view! {
                                    <li class="flex gap-2">
                                        <span class=format!("w-16 shrink-0 font-semibold uppercase {cls}")>{i.level.clone()}</span>
                                        <code class="shrink-0 font-mono text-slate-500">{i.path.clone()}</code>
                                        <span>{i.message.clone()}</span>
                                    </li>
                                }
                            }).collect_view()}
                        </ul>
                    </div>
                }.into_any(),
            })}
            {move || save_error.get().map(|e| view! { <div class="mt-3"><ErrorBox error=e /></div> })}
            <div class="mt-4 flex flex-wrap justify-end gap-2">
                <button type="button" class=BTN disabled=move || busy.get() || text.with(Option::is_none) on:click=move |_| run_lint()>
                    <Icon name=IconName::Check class="h-4 w-4" />
                    {tr("ui-lint-run")}
                </button>
                {writable.then(|| view! {
                    <button type="button" class=format!("{BTN_PRIMARY} max-md:hidden") disabled=move || busy.get() || !dirty.get() on:click=move |_| save()>
                        {tr("ui-save")}
                    </button>
                })}
            </div>
        </Modal>
    }
}
