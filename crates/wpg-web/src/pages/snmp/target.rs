//! The target form (agent address, version, community or v3 user) with saved
//! profiles. Profiles live in `localStorage` and never contain the community
//! or pass phrases.

use leptos::prelude::*;

use super::model::{
    apply_profile, parse_profiles, profiles_json, upsert_profile, Profile, AUTH_PROTOCOLS, LEVELS,
    PRIV_PROTOCOLS, PROFILES_KEY,
};
use super::use_snmp;
use crate::components::{Icon, IconName, BTN, BTN_GHOST, INPUT, LABEL};
use crate::i18n::{tr, use_i18n};
use crate::prefs;

fn load_profiles() -> Vec<Profile> {
    prefs::load(PROFILES_KEY).map_or_else(Vec::new, |t| parse_profiles(&t))
}

/// A labelled text input bound to one field of the target.
#[component]
pub(crate) fn TargetField(
    id: &'static str,
    label: &'static str,
    #[prop(into)] value: Signal<String>,
    #[prop(into)] on_input: Callback<String>,
    #[prop(optional)] password: bool,
    #[prop(optional)] placeholder: &'static str,
    #[prop(optional)] numeric: bool,
) -> impl IntoView {
    view! {
        <div>
            <label for=id class=LABEL>{tr(label)}</label>
            <input
                id=id
                class=INPUT
                type=if password { "password" } else { "text" }
                inputmode=if numeric { "numeric" } else { "text" }
                autocomplete=if password { "new-password" } else { "off" }
                spellcheck="false"
                placeholder=placeholder
                prop:value=move || value.get()
                on:input=move |ev| on_input.run(event_target_value(&ev))
            />
        </div>
    }
}

/// A labelled select over fixed values.
#[component]
pub(crate) fn TargetSelect(
    id: &'static str,
    label: &'static str,
    options: &'static [&'static str],
    #[prop(into)] value: Signal<String>,
    #[prop(into)] on_change: Callback<String>,
) -> impl IntoView {
    view! {
        <div>
            <label for=id class=LABEL>{tr(label)}</label>
            <select id=id class=INPUT on:change=move |ev| on_change.run(event_target_value(&ev))>
                {options
                    .iter()
                    .map(|o| {
                        let o = *o;
                        view! { <option value=o selected=move || value.get() == o>{o}</option> }
                    })
                    .collect_view()}
            </select>
        </div>
    }
}

/// The target fieldset.
#[component]
pub(crate) fn TargetForm() -> impl IntoView {
    let i18n = use_i18n();
    let ctx = use_snmp();
    let t = ctx.target;
    let profiles = RwSignal::new(load_profiles());
    let profile_name = RwSignal::new(String::new());

    macro_rules! field {
        ($f:ident) => {
            (
                Signal::derive(move || t.with(|x| x.$f.clone())),
                Callback::new(move |v: String| t.update(|x| x.$f = v)),
            )
        };
    }
    let (host, set_host) = field!(host);
    let (port, set_port) = field!(port);
    let (version, set_version) = field!(version);
    let (community, set_community) = field!(community);
    let (user, set_user) = field!(user);
    let (level, set_level) = field!(level);
    let (auth_protocol, set_auth_protocol) = field!(auth_protocol);
    let (auth_password, set_auth_password) = field!(auth_password);
    let (priv_protocol, set_priv_protocol) = field!(priv_protocol);
    let (priv_password, set_priv_password) = field!(priv_password);
    let (context_name, set_context_name) = field!(context_name);
    let (timeout_ms, set_timeout_ms) = field!(timeout_ms);
    let (retries, set_retries) = field!(retries);

    let v3 = move || version.get() == "v3";
    let save = move |_| {
        let name = profile_name.get_untracked();
        if name.trim().is_empty() {
            return;
        }
        profiles.update(|list| upsert_profile(list, &name, &t.get_untracked()));
        prefs::store(PROFILES_KEY, &profiles.with_untracked(|p| profiles_json(p)));
    };
    let load = move |name: String| {
        if let Some(p) = profiles.with_untracked(|l| l.iter().find(|p| p.name == name).cloned()) {
            t.update(|cur| *cur = apply_profile(cur, &p));
            profile_name.set(name);
        }
    };
    let delete = move |_| {
        let name = profile_name.get_untracked();
        profiles.update(|l| l.retain(|p| p.name != name));
        prefs::store(PROFILES_KEY, &profiles.with_untracked(|p| profiles_json(p)));
        profile_name.set(String::new());
    };

    view! {
        <fieldset class="space-y-3">
            <legend class="mb-2 text-base font-semibold">{tr("ui-snmp-target")}</legend>
            <div class="flex flex-wrap items-end gap-2">
                <div class="min-w-48">
                    <label for="snmp-profile" class=LABEL>{tr("ui-snmp-profile")}</label>
                    <select id="snmp-profile" class=INPUT on:change=move |ev| load(event_target_value(&ev))>
                        <option value="" selected=move || profile_name.with(String::is_empty)>{tr("ui-snmp-profile-choose")}</option>
                        {move || {
                            profiles
                                .get()
                                .into_iter()
                                .map(|p| {
                                    let n = p.name.clone();
                                    let n2 = p.name.clone();
                                    view! { <option value=n selected=move || profile_name.get() == n2>{p.name}</option> }
                                })
                                .collect_view()
                        }}
                    </select>
                </div>
                <div class="min-w-40">
                    <label for="snmp-profile-name" class=LABEL>{tr("ui-snmp-profile-name")}</label>
                    <input
                        id="snmp-profile-name"
                        class=INPUT
                        prop:value=move || profile_name.get()
                        on:input=move |ev| profile_name.set(event_target_value(&ev))
                    />
                </div>
                <button type="button" class=BTN on:click=save title=tr("ui-snmp-profile-save-hint")>
                    <Icon name=IconName::Check class="h-4 w-4" />
                    {tr("ui-snmp-profile-save")}
                </button>
                <button
                    type="button"
                    class=BTN_GHOST
                    aria-label=tr("ui-snmp-profile-delete")
                    title=tr("ui-snmp-profile-delete")
                    disabled=move || !profiles.with(|l| l.iter().any(|p| p.name == profile_name.get()))
                    on:click=delete
                >
                    <Icon name=IconName::Trash class="h-4 w-4" />
                </button>
            </div>
            <p class="text-xs text-slate-500">{tr("ui-snmp-profile-note")}</p>
            <div class="grid gap-3 sm:grid-cols-2 lg:grid-cols-6">
                <div class="sm:col-span-2">
                    <TargetField id="snmp-host" label="ui-snmp-host" value=host on_input=set_host placeholder="192.0.2.1" />
                </div>
                <TargetField id="snmp-port" label="ui-snmp-port" value=port on_input=set_port placeholder="161" numeric=true />
                <TargetSelect id="snmp-version" label="ui-snmp-version" options=&["v1", "v2c", "v3"] value=version on_change=set_version />
                <TargetField id="snmp-timeout" label="ui-snmp-timeout" value=timeout_ms on_input=set_timeout_ms numeric=true />
                <TargetField id="snmp-retries" label="ui-snmp-retries" value=retries on_input=set_retries numeric=true />
                <Show when=move || !v3()>
                    <div class="sm:col-span-2">
                        <TargetField id="snmp-community" label="ui-snmp-community" value=community on_input=set_community password=true />
                    </div>
                </Show>
                <Show when=v3>
                    <TargetField id="snmp-user" label="ui-snmp-user" value=user on_input=set_user />
                    <TargetSelect id="snmp-level" label="ui-snmp-level" options=&LEVELS value=level on_change=set_level />
                    <Show when=move || level.get() != "noAuthNoPriv">
                        <TargetSelect id="snmp-auth-proto" label="ui-snmp-auth-protocol" options=&AUTH_PROTOCOLS value=auth_protocol on_change=set_auth_protocol />
                        <TargetField id="snmp-auth-pass" label="ui-snmp-auth-password" value=auth_password on_input=set_auth_password password=true />
                    </Show>
                    <Show when=move || level.get() == "authPriv">
                        <TargetSelect id="snmp-priv-proto" label="ui-snmp-priv-protocol" options=&PRIV_PROTOCOLS value=priv_protocol on_change=set_priv_protocol />
                        <TargetField id="snmp-priv-pass" label="ui-snmp-priv-password" value=priv_password on_input=set_priv_password password=true />
                    </Show>
                    <TargetField id="snmp-context" label="ui-snmp-context" value=context_name on_input=set_context_name />
                </Show>
            </div>
            <p class="text-xs text-slate-500">
                {move || i18n.t(if v3() { "ui-snmp-v3-note" } else { "ui-snmp-community-note" })}
            </p>
        </fieldset>
    }
}
