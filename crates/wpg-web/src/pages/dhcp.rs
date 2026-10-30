//! `/dhcp`: the DHCP server tool (ADR-0012). State and start / stop on the
//! interface fixed at deploy time (or chosen here), the settings form
//! (interface addresses, DHCPv4 pool and options, DHCPv6 addresses / prefix
//! delegation / DNS, router advertisements), the leases and the activity log.

use leptos::prelude::*;

use crate::api::dhcp::{self as api, Activity, DhcpStatus, Lease, Reservation, Settings};
use crate::api::{self as core_api, ApiError};
use crate::components::{
    use_toasts, Empty, ErrorBox, Icon, IconName, Loading, PageHeader, BTN, BTN_GHOST, BTN_PRIMARY,
    CARD, INPUT, LABEL, TD, TH,
};
use crate::fmt;
use crate::i18n::{tr, use_i18n};
use crate::ws::use_live;

/// Splits a list typed as `a, b c`.
pub(crate) fn split_list(s: &str) -> Vec<String> {
    s.split([',', ' ', '\n', ';'])
        .map(str::trim)
        .filter(|x| !x.is_empty())
        .map(str::to_owned)
        .collect()
}

/// `mac ip` per line.
pub(crate) fn parse_reservations(s: &str) -> Vec<Reservation> {
    s.lines()
        .filter_map(|l| {
            let mut it = l.split([' ', '\t', ',']).filter(|x| !x.is_empty());
            Some(Reservation {
                mac: it.next()?.to_owned(),
                ip: it.next()?.to_owned(),
            })
        })
        .collect()
}

fn reservations_text(r: &[Reservation]) -> String {
    r.iter()
        .map(|r| format!("{} {}", r.mac, r.ip))
        .collect::<Vec<_>>()
        .join("\n")
}

/// The editable form (text fields as typed).
#[derive(Debug, Clone, Copy)]
struct Form {
    iface: RwSignal<String>,
    addresses: RwSignal<String>,
    v4_enabled: RwSignal<bool>,
    v4_start: RwSignal<String>,
    v4_end: RwSignal<String>,
    v4_mask: RwSignal<String>,
    v4_routers: RwSignal<String>,
    v4_dns: RwSignal<String>,
    v4_domain: RwSignal<String>,
    v4_ntp: RwSignal<String>,
    v4_lease: RwSignal<String>,
    v4_server_ip: RwSignal<String>,
    v4_reservations: RwSignal<String>,
    v6_enabled: RwSignal<bool>,
    v6_na: RwSignal<bool>,
    v6_start: RwSignal<String>,
    v6_end: RwSignal<String>,
    v6_pd: RwSignal<bool>,
    v6_pd_prefix: RwSignal<String>,
    v6_pd_len: RwSignal<String>,
    v6_pd_delegated: RwSignal<String>,
    v6_dns: RwSignal<String>,
    v6_domains: RwSignal<String>,
    v6_preferred: RwSignal<String>,
    v6_valid: RwSignal<String>,
    v6_rapid: RwSignal<bool>,
    v6_preference: RwSignal<String>,
    ra_enabled: RwSignal<bool>,
    ra_interval: RwSignal<String>,
    ra_lifetime: RwSignal<String>,
    ra_prefix: RwSignal<String>,
    ra_autonomous: RwSignal<bool>,
}

impl Form {
    fn new() -> Self {
        let s = || RwSignal::new(String::new());
        let b = || RwSignal::new(false);
        Self {
            iface: s(),
            addresses: s(),
            v4_enabled: b(),
            v4_start: s(),
            v4_end: s(),
            v4_mask: s(),
            v4_routers: s(),
            v4_dns: s(),
            v4_domain: s(),
            v4_ntp: s(),
            v4_lease: s(),
            v4_server_ip: s(),
            v4_reservations: s(),
            v6_enabled: b(),
            v6_na: b(),
            v6_start: s(),
            v6_end: s(),
            v6_pd: b(),
            v6_pd_prefix: s(),
            v6_pd_len: s(),
            v6_pd_delegated: s(),
            v6_dns: s(),
            v6_domains: s(),
            v6_preferred: s(),
            v6_valid: s(),
            v6_rapid: b(),
            v6_preference: s(),
            ra_enabled: b(),
            ra_interval: s(),
            ra_lifetime: s(),
            ra_prefix: s(),
            ra_autonomous: b(),
        }
    }

    fn load(self, st: &DhcpStatus) {
        let s = &st.settings;
        let join = |v: &[String]| v.join(", ");
        self.iface.set(st.iface.clone());
        self.addresses.set(join(&s.addresses));
        self.v4_enabled.set(s.v4.enabled);
        self.v4_start.set(s.v4.range_start.clone());
        self.v4_end.set(s.v4.range_end.clone());
        self.v4_mask.set(s.v4.subnet_mask.clone());
        self.v4_routers.set(join(&s.v4.routers));
        self.v4_dns.set(join(&s.v4.dns));
        self.v4_domain.set(s.v4.domain.clone());
        self.v4_ntp.set(join(&s.v4.ntp));
        self.v4_lease.set(s.v4.lease_secs.to_string());
        self.v4_server_ip
            .set(s.v4.server_ip.clone().unwrap_or_default());
        self.v4_reservations
            .set(reservations_text(&s.v4.reservations));
        self.v6_enabled.set(s.v6.enabled);
        self.v6_na.set(s.v6.na_enabled);
        self.v6_start.set(s.v6.range_start.clone());
        self.v6_end.set(s.v6.range_end.clone());
        self.v6_pd.set(s.v6.pd_enabled);
        self.v6_pd_prefix.set(s.v6.pd_prefix.clone());
        self.v6_pd_len.set(s.v6.pd_prefix_len.to_string());
        self.v6_pd_delegated.set(s.v6.pd_delegated_len.to_string());
        self.v6_dns.set(join(&s.v6.dns));
        self.v6_domains.set(join(&s.v6.domains));
        self.v6_preferred.set(s.v6.preferred_secs.to_string());
        self.v6_valid.set(s.v6.valid_secs.to_string());
        self.v6_rapid.set(s.v6.rapid_commit);
        self.v6_preference.set(s.v6.preference.to_string());
        self.ra_enabled.set(s.v6.ra_enabled);
        self.ra_interval.set(s.v6.ra_interval_secs.to_string());
        self.ra_lifetime
            .set(s.v6.ra_router_lifetime_secs.to_string());
        self.ra_prefix.set(s.v6.ra_prefix.clone());
        self.ra_autonomous.set(s.v6.ra_autonomous);
    }

    fn settings(self) -> Settings {
        let g = |s: RwSignal<String>| s.get_untracked().trim().to_owned();
        let l = |s: RwSignal<String>| split_list(&s.get_untracked());
        let mut out = Settings {
            addresses: l(self.addresses),
            ..Settings::default()
        };
        let v4 = &mut out.v4;
        v4.enabled = self.v4_enabled.get_untracked();
        v4.range_start = g(self.v4_start);
        v4.range_end = g(self.v4_end);
        v4.subnet_mask = g(self.v4_mask);
        v4.routers = l(self.v4_routers);
        v4.dns = l(self.v4_dns);
        v4.domain = g(self.v4_domain);
        v4.ntp = l(self.v4_ntp);
        v4.lease_secs = g(self.v4_lease).parse().unwrap_or(0);
        v4.server_ip = Some(g(self.v4_server_ip)).filter(|s| !s.is_empty());
        v4.reservations = parse_reservations(&self.v4_reservations.get_untracked());
        let v6 = &mut out.v6;
        v6.enabled = self.v6_enabled.get_untracked();
        v6.na_enabled = self.v6_na.get_untracked();
        v6.range_start = g(self.v6_start);
        v6.range_end = g(self.v6_end);
        v6.pd_enabled = self.v6_pd.get_untracked();
        v6.pd_prefix = g(self.v6_pd_prefix);
        v6.pd_prefix_len = g(self.v6_pd_len).parse().unwrap_or(0);
        v6.pd_delegated_len = g(self.v6_pd_delegated).parse().unwrap_or(0);
        v6.dns = l(self.v6_dns);
        v6.domains = l(self.v6_domains);
        v6.preferred_secs = g(self.v6_preferred).parse().unwrap_or(0);
        v6.valid_secs = g(self.v6_valid).parse().unwrap_or(0);
        v6.rapid_commit = self.v6_rapid.get_untracked();
        v6.preference = g(self.v6_preference).parse().unwrap_or(0);
        v6.ra_enabled = self.ra_enabled.get_untracked();
        v6.ra_interval_secs = g(self.ra_interval).parse().unwrap_or(0);
        v6.ra_router_lifetime_secs = g(self.ra_lifetime).parse().unwrap_or(0);
        v6.ra_prefix = g(self.ra_prefix);
        v6.ra_autonomous = self.ra_autonomous.get_untracked();
        out
    }
}

/// A labelled text input.
#[component]
fn Field(
    id: &'static str,
    label: &'static str,
    value: RwSignal<String>,
    #[prop(optional)] placeholder: &'static str,
    #[prop(optional, into)] disabled: Signal<bool>,
) -> impl IntoView {
    view! {
        <div>
            <label for=id class=LABEL>{tr(label)}</label>
            <input id=id class=format!("{INPUT} font-mono") spellcheck="false" placeholder=placeholder
                disabled=move || disabled.get()
                prop:value=move || value.get() on:input=move |ev| value.set(event_target_value(&ev)) />
        </div>
    }
}

/// A checkbox with its label.
#[component]
fn Check(label: &'static str, value: RwSignal<bool>) -> impl IntoView {
    view! {
        <label class="flex items-center gap-2 text-sm">
            <input type="checkbox" prop:checked=move || value.get() on:change=move |ev| value.set(event_target_checked(&ev)) />
            {tr(label)}
        </label>
    }
}

#[component]
pub(crate) fn DhcpPage() -> impl IntoView {
    let i18n = use_i18n();
    let toasts = use_toasts();
    let live = use_live();
    let status = RwSignal::new(None::<Result<DhcpStatus, ApiError>>);
    let form = Form::new();
    let loaded = StoredValue::new(false);
    let busy = RwSignal::new(false);
    let error = RwSignal::new(None::<ApiError>);
    Effect::new(move |_| {
        live.dhcp_rev.track();
        live.dhcp_activity_rev.track();
        leptos::task::spawn_local(async move {
            let r = api::status().await;
            if let Ok(s) = &r {
                if !loaded.get_value() {
                    form.load(s);
                    loaded.set_value(true);
                }
            }
            status.set(Some(r));
        });
    });
    let apply = move |r: Result<DhcpStatus, ApiError>, ok_key: &'static str| match r {
        Ok(s) => {
            toasts.ok(i18n.t(ok_key));
            status.set(Some(Ok(s)));
            error.set(None);
        }
        Err(e) => error.set(Some(e)),
    };
    let save = move |then_start: bool| {
        let pinned = status.with_untracked(|s| {
            s.as_ref()
                .and_then(|s| s.as_ref().ok())
                .is_some_and(|s| s.pinned_iface.is_some())
        });
        let iface = (!pinned).then(|| form.iface.get_untracked());
        let settings = form.settings();
        busy.set(true);
        leptos::task::spawn_local(async move {
            let r = api::update(iface, &settings).await;
            let r = match r {
                Ok(s) if then_start && !s.running => api::start().await,
                r => r,
            };
            apply(
                r,
                if then_start {
                    "ui-dhcp-started"
                } else {
                    "ui-dhcp-saved"
                },
            );
            busy.set(false);
        });
    };
    let stop = move |_| {
        busy.set(true);
        leptos::task::spawn_local(async move {
            apply(api::stop().await, "ui-dhcp-stopped");
            busy.set(false);
        });
    };
    let running = move || status.with(|s| matches!(s, Some(Ok(s)) if s.running));
    // Only a change of phase rebuilds the page (not every status refresh: the
    // form would lose focus while typing).
    let phase = Memo::new(move |_| match status.get() {
        None => 0u8,
        Some(Err(e)) if e.status() == Some(404) => 1,
        Some(Err(_)) => 2,
        Some(Ok(s)) if s.pinned_iface.is_some() => 3,
        Some(Ok(_)) => 4,
    });
    let current = Signal::derive(move || status.get().and_then(Result::ok));

    view! {
        <PageHeader title=tr("ui-nav-dhcp")>
            <span role="status" class=move || if running() {
                "rounded-full bg-emerald-100 px-2 py-0.5 text-xs font-medium text-emerald-800 dark:bg-emerald-500/15 dark:text-emerald-300"
            } else {
                "rounded-full bg-slate-100 px-2 py-0.5 text-xs font-medium text-slate-600 dark:bg-slate-800 dark:text-slate-300"
            }>
                {move || i18n.t(if running() { "ui-dhcp-running" } else { "ui-dhcp-stopped-badge" })}
            </span>
            <button type="button" class=BTN_PRIMARY disabled=move || busy.get() on:click=move |_| save(true)>
                <Icon name=IconName::Play class="h-4 w-4" />
                {move || i18n.t(if running() { "ui-dhcp-save-restart" } else { "ui-dhcp-save-start" })}
            </button>
            <Show when=running>
                <button type="button" class=BTN disabled=move || busy.get() on:click=stop>
                    <Icon name=IconName::Stop class="h-4 w-4" />
                    {tr("ui-dhcp-stop")}
                </button>
            </Show>
        </PageHeader>
        {move || match phase.get() {
            0 => view! { <Loading /> }.into_any(),
            1 => view! {
                <p class="rounded-md bg-amber-50 px-3 py-2 text-sm text-amber-800 dark:bg-amber-950/40 dark:text-amber-200" role="status">
                    {tr("ui-dhcp-disabled")}
                </p>
            }
            .into_any(),
            2 => match status.get_untracked() {
                Some(Err(e)) => view! { <ErrorBox error=e /> }.into_any(),
                _ => ().into_any(),
            },
            p => {
                let pinned = p == 3;
                view! {
                <div class="space-y-6">
                    {move || error.get().map(|e| view! { <ErrorBox error=e /> })}
                    <StatusCard status=current form=form pinned=pinned />
                    <SettingsCard form=form busy=busy on_save=Callback::new(move |()| save(false)) />
                    <LeaseList />
                    <ActivityList />
                </div>
                }
                .into_any()
            }
        }}
    }
}

#[component]
fn StatusCard(status: Signal<Option<DhcpStatus>>, form: Form, pinned: bool) -> impl IntoView {
    let i18n = use_i18n();
    let ifaces = LocalResource::new(|| core_api::ifaces(false));
    let row = move |key: &'static str, value: fn(&DhcpStatus) -> String| {
        view! {
            <div class="min-w-0">
                <dt class="text-xs text-slate-500 dark:text-slate-400">{tr(key)}</dt>
                <dd class="truncate font-mono text-sm">{move || status.with(|s| s.as_ref().map(value).unwrap_or_default())}</dd>
            </div>
        }
    };
    view! {
        <section class=CARD aria-labelledby="dhcp-status-h">
            <h2 id="dhcp-status-h" class="mb-3 text-base font-semibold">{tr("ui-dhcp-status")}</h2>
            {move || status.with(|s| s.as_ref().is_some_and(|s| !s.supported)).then(|| view! {
                <p class="mb-3 rounded-md bg-amber-50 px-3 py-2 text-sm text-amber-800 dark:bg-amber-950/40 dark:text-amber-200">{tr("ui-dhcp-unsupported")}</p>
            })}
            {move || status.with(|s| s.as_ref().and_then(|s| s.error.clone())).map(|e| view! {
                <p class="mb-3 rounded-md bg-rose-50 px-3 py-2 text-sm text-rose-800 dark:bg-rose-950/40 dark:text-rose-200">
                    {move || i18n.ta("ui-dhcp-boot-error", &[("error", e.clone())])}
                </p>
            })}
            <div class="grid gap-4 sm:grid-cols-2 lg:grid-cols-4">
                <div>
                    <label for="dhcp-iface" class=LABEL>{tr("ui-dhcp-iface")}</label>
                    {if pinned {
                        view! {
                            <p class="font-mono text-sm">{move || form.iface.get()}</p>
                            <p class="text-xs text-slate-500">{tr("ui-dhcp-iface-pinned")}</p>
                        }
                        .into_any()
                    } else {
                        view! {
                            <select id="dhcp-iface" class=INPUT on:change=move |ev| form.iface.set(event_target_value(&ev))>
                                <option value="" selected=move || form.iface.get().is_empty()>{tr("ui-dhcp-iface-choose")}</option>
                                {move || ifaces.get().and_then(Result::ok).unwrap_or_default().into_iter()
                                    .filter(|i| i.allowed && !i.loopback)
                                    .map(|i| {
                                        let name = i.name.clone();
                                        let n2 = name.clone();
                                        let addrs = i.ipv4.join(", ");
                                        view! { <option value=name.clone() selected=move || form.iface.get() == n2>{format!("{name} {addrs}")}</option> }
                                    })
                                    .collect_view()}
                            </select>
                            <p class="text-xs text-slate-500">{tr("ui-dhcp-iface-hint")}</p>
                        }
                        .into_any()
                    }}
                </div>
                {row("ui-dhcp-server-ip", |s| s.server_ip.clone().unwrap_or_else(|| "–".into()))}
                {row("ui-dhcp-server-duid", |s| s.server_duid.clone().unwrap_or_else(|| "–".into()))}
                {row("ui-dhcp-added-addresses", |s| if s.added_addresses.is_empty() { "–".into() } else { s.added_addresses.join(", ") })}
            </div>
            <dl class="mt-4 grid grid-cols-2 gap-4 text-sm sm:grid-cols-5">
                {[
                    ("ui-dhcp-rx4", (|c: &api::Counters| c.rx4) as fn(&api::Counters) -> u64),
                    ("ui-dhcp-tx4", |c| c.tx4),
                    ("ui-dhcp-rx6", |c| c.rx6),
                    ("ui-dhcp-tx6", |c| c.tx6),
                    ("ui-dhcp-ra-sent", |c| c.ra),
                ]
                    .into_iter()
                    .map(|(k, get)| view! {
                        <div>
                            <dt class="text-xs text-slate-500 dark:text-slate-400">{tr(k)}</dt>
                            <dd class="text-lg font-semibold tabular-nums">
                                {move || fmt::int(i18n.locale.get(), status.with(|s| s.as_ref().map_or(0, |s| get(&s.stats))))}
                            </dd>
                        </div>
                    })
                    .collect_view()}
            </dl>
            {move || status.with(|s| s.as_ref().and_then(|s| s.started_ms)).map(|t| view! {
                <p class="mt-2 text-xs text-slate-500">{move || i18n.ta("ui-dhcp-since", &[("time", fmt::datetime(i18n.locale.get(), t))])}</p>
            })}
        </section>
    }
}

#[component]
fn SettingsCard(form: Form, busy: RwSignal<bool>, on_save: Callback<()>) -> impl IntoView {
    let f = form;
    let v4_off = Signal::derive(move || !f.v4_enabled.get());
    let v6_off = Signal::derive(move || !f.v6_enabled.get());
    let na_off = Signal::derive(move || !f.v6_enabled.get() || !f.v6_na.get());
    let pd_off = Signal::derive(move || !f.v6_enabled.get() || !f.v6_pd.get());
    let ra_off = Signal::derive(move || !f.ra_enabled.get());
    let h3 = "mb-2 mt-5 text-sm font-semibold";
    view! {
        <section class=CARD aria-labelledby="dhcp-settings-h">
            <div class="mb-3 flex flex-wrap items-center justify-between gap-3">
                <h2 id="dhcp-settings-h" class="text-base font-semibold">{tr("ui-dhcp-settings")}</h2>
                <button type="button" class=BTN disabled=move || busy.get() on:click=move |_| on_save.run(())>
                    <Icon name=IconName::Check class="h-4 w-4" />
                    {tr("ui-dhcp-save")}
                </button>
            </div>

            <h3 class="mb-2 text-sm font-semibold">{tr("ui-dhcp-addresses")}</h3>
            <Field id="dhcp-addresses" label="ui-dhcp-addresses-field" value=f.addresses placeholder="192.168.100.1/24, 2001:db8:1::1/64" />
            <p class="mt-1 text-xs text-slate-500">{tr("ui-dhcp-addresses-hint")}</p>

            <h3 class=h3>"DHCPv4"</h3>
            <Check label="ui-dhcp-v4-enabled" value=f.v4_enabled />
            <div class="mt-2 grid gap-3 sm:grid-cols-2 lg:grid-cols-4">
                <Field id="dhcp-v4-start" label="ui-dhcp-range-start" value=f.v4_start disabled=v4_off />
                <Field id="dhcp-v4-end" label="ui-dhcp-range-end" value=f.v4_end disabled=v4_off />
                <Field id="dhcp-v4-mask" label="ui-dhcp-subnet-mask" value=f.v4_mask disabled=v4_off />
                <Field id="dhcp-v4-lease" label="ui-dhcp-lease-secs" value=f.v4_lease disabled=v4_off />
                <Field id="dhcp-v4-routers" label="ui-dhcp-routers" value=f.v4_routers placeholder="192.168.100.1" disabled=v4_off />
                <Field id="dhcp-v4-dns" label="ui-dhcp-dns" value=f.v4_dns placeholder="192.168.100.1, 8.8.8.8" disabled=v4_off />
                <Field id="dhcp-v4-domain" label="ui-dhcp-domain" value=f.v4_domain placeholder="lab.example" disabled=v4_off />
                <Field id="dhcp-v4-ntp" label="ui-dhcp-ntp" value=f.v4_ntp disabled=v4_off />
                <Field id="dhcp-v4-server" label="ui-dhcp-server-id" value=f.v4_server_ip disabled=v4_off />
            </div>
            <label for="dhcp-v4-res" class=format!("{LABEL} mt-3")>{tr("ui-dhcp-reservations")}</label>
            <textarea id="dhcp-v4-res" rows="3" class=format!("{INPUT} font-mono") spellcheck="false" placeholder="02:00:00:00:00:01 192.168.100.10"
                disabled=move || v4_off.get()
                prop:value=move || f.v4_reservations.get() on:input=move |ev| f.v4_reservations.set(event_target_value(&ev))></textarea>

            <h3 class=h3>"DHCPv6"</h3>
            <div class="flex flex-wrap gap-4">
                <Check label="ui-dhcp-v6-enabled" value=f.v6_enabled />
                <Check label="ui-dhcp-v6-na" value=f.v6_na />
                <Check label="ui-dhcp-v6-pd" value=f.v6_pd />
                <Check label="ui-dhcp-v6-rapid" value=f.v6_rapid />
            </div>
            <div class="mt-2 grid gap-3 sm:grid-cols-2 lg:grid-cols-4">
                <Field id="dhcp-v6-start" label="ui-dhcp-range-start" value=f.v6_start disabled=na_off />
                <Field id="dhcp-v6-end" label="ui-dhcp-range-end" value=f.v6_end disabled=na_off />
                <Field id="dhcp-v6-preferred" label="ui-dhcp-preferred-secs" value=f.v6_preferred disabled=v6_off />
                <Field id="dhcp-v6-valid" label="ui-dhcp-valid-secs" value=f.v6_valid disabled=v6_off />
                <Field id="dhcp-v6-pd-prefix" label="ui-dhcp-pd-prefix" value=f.v6_pd_prefix placeholder="2001:db8:100::" disabled=pd_off />
                <Field id="dhcp-v6-pd-len" label="ui-dhcp-pd-prefix-len" value=f.v6_pd_len disabled=pd_off />
                <Field id="dhcp-v6-pd-delegated" label="ui-dhcp-pd-delegated-len" value=f.v6_pd_delegated disabled=pd_off />
                <Field id="dhcp-v6-preference" label="ui-dhcp-preference" value=f.v6_preference disabled=v6_off />
                <Field id="dhcp-v6-dns" label="ui-dhcp-dns" value=f.v6_dns placeholder="2001:db8::53" disabled=v6_off />
                <Field id="dhcp-v6-domains" label="ui-dhcp-domains" value=f.v6_domains placeholder="lab.example" disabled=v6_off />
            </div>

            <h3 class=h3>{tr("ui-dhcp-ra")}</h3>
            <div class="flex flex-wrap gap-4">
                <Check label="ui-dhcp-ra-enabled" value=f.ra_enabled />
                <Check label="ui-dhcp-ra-autonomous" value=f.ra_autonomous />
            </div>
            <div class="mt-2 grid gap-3 sm:grid-cols-2 lg:grid-cols-4">
                <Field id="dhcp-ra-interval" label="ui-dhcp-ra-interval" value=f.ra_interval disabled=ra_off />
                <Field id="dhcp-ra-lifetime" label="ui-dhcp-ra-lifetime" value=f.ra_lifetime disabled=ra_off />
                <Field id="dhcp-ra-prefix" label="ui-dhcp-ra-prefix" value=f.ra_prefix placeholder="2001:db8:1::/64" disabled=ra_off />
            </div>
            <p class="mt-1 text-xs text-slate-500">{tr("ui-dhcp-ra-hint")}</p>
        </section>
    }
}

fn kind_label(kind: &str) -> &'static str {
    match kind {
        "na" => "DHCPv6",
        "pd" => "DHCPv6-PD",
        _ => "DHCPv4",
    }
}

#[component]
fn LeaseList() -> impl IntoView {
    let i18n = use_i18n();
    let toasts = use_toasts();
    let live = use_live();
    let reload = RwSignal::new(0u32);
    let list = LocalResource::new(move || {
        live.dhcp_rev.track();
        live.dhcp_activity_rev.track();
        reload.track();
        api::leases()
    });
    let delete = move |key: Option<String>| {
        leptos::task::spawn_local(async move {
            match api::delete_leases(key.as_deref()).await {
                Ok(()) => reload.update(|r| *r += 1),
                Err(e) => toasts.error(e.message(i18n)),
            }
        });
    };
    view! {
        <section aria-labelledby="dhcp-leases-h">
            <div class="mb-2 flex flex-wrap items-center gap-3">
                <h2 id="dhcp-leases-h" class="text-base font-semibold">{tr("ui-dhcp-leases")}</h2>
                <button type="button" class=BTN on:click=move |_| delete(None)>
                    <Icon name=IconName::Trash class="h-4 w-4" />
                    {tr("ui-dhcp-leases-clear")}
                </button>
            </div>
            {move || match list.get() {
                None => view! { <Loading /> }.into_any(),
                Some(Err(e)) => view! { <ErrorBox error=e /> }.into_any(),
                Some(Ok(l)) if l.is_empty() => view! { <Empty text=Signal::derive(move || i18n.t("ui-dhcp-leases-empty")) /> }.into_any(),
                Some(Ok(l)) => view! {
                    <div class="overflow-x-auto rounded-lg border border-slate-200 dark:border-slate-800">
                        <table class="min-w-full divide-y divide-slate-200 dark:divide-slate-800">
                            <thead><tr>
                                <th class=TH>{tr("ui-dhcp-lease-kind")}</th>
                                <th class=TH>{tr("ui-dhcp-lease-addr")}</th>
                                <th class=TH>{tr("ui-dhcp-lease-client")}</th>
                                <th class=TH>{tr("ui-dhcp-lease-state")}</th>
                                <th class=TH>{tr("ui-dhcp-lease-expires")}</th>
                                <th class=TH><span class="sr-only">{tr("ui-dhcp-lease-delete")}</span></th>
                            </tr></thead>
                            <tbody class="divide-y divide-slate-100 dark:divide-slate-800">
                                {l.into_iter().map(|x: Lease| {
                                    let key = x.key();
                                    let client = match (&x.mac, &x.hostname) {
                                        (Some(m), Some(h)) => format!("{m} ({h})"),
                                        (Some(m), None) if x.kind != "v4" => format!("{m} · IAID {}", x.iaid),
                                        (Some(m), None) => m.clone(),
                                        _ => x.client.clone(),
                                    };
                                    let state_key = match x.state.as_str() {
                                        "offered" => "ui-dhcp-state-offered",
                                        "declined" => "ui-dhcp-state-declined",
                                        _ => "ui-dhcp-state-bound",
                                    };
                                    let expires = x.expires;
                                    view! {
                                        <tr>
                                            <td class=TD>{kind_label(&x.kind)}</td>
                                            <td class=format!("{TD} font-mono")>{key.clone()}</td>
                                            <td class=format!("{TD} font-mono text-xs")>{client}</td>
                                            <td class=TD>{tr(state_key)}</td>
                                            <td class=TD>{move || fmt::datetime(i18n.locale.get(), expires.saturating_mul(1000))}</td>
                                            <td class=TD>
                                                <button type="button" class=BTN_GHOST aria-label=tr("ui-dhcp-lease-delete") title=tr("ui-dhcp-lease-delete")
                                                    on:click=move |_| delete(Some(key.clone()))>
                                                    <Icon name=IconName::Trash class="h-4 w-4" />
                                                </button>
                                            </td>
                                        </tr>
                                    }
                                }).collect_view()}
                            </tbody>
                        </table>
                    </div>
                }
                .into_any(),
            }}
        </section>
    }
}

#[component]
fn ActivityList() -> impl IntoView {
    let i18n = use_i18n();
    let toasts = use_toasts();
    let live = use_live();
    let reload = RwSignal::new(0u32);
    let list = LocalResource::new(move || {
        live.dhcp_rev.track();
        live.dhcp_activity_rev.track();
        reload.track();
        api::activity()
    });
    let clear = move |_| {
        leptos::task::spawn_local(async move {
            match api::clear_activity().await {
                Ok(()) => reload.update(|r| *r += 1),
                Err(e) => toasts.error(e.message(i18n)),
            }
        });
    };
    view! {
        <section aria-labelledby="dhcp-activity-h">
            <div class="mb-2 flex flex-wrap items-center gap-3">
                <h2 id="dhcp-activity-h" class="text-base font-semibold">{tr("ui-dhcp-activity")}</h2>
                <button type="button" class=BTN_GHOST aria-label=tr("ui-dhcp-activity-clear") title=tr("ui-dhcp-activity-clear") on:click=clear>
                    <Icon name=IconName::Trash class="h-4 w-4" />
                </button>
            </div>
            {move || match list.get() {
                None => view! { <Loading /> }.into_any(),
                Some(Err(e)) => view! { <ErrorBox error=e /> }.into_any(),
                Some(Ok(l)) if l.is_empty() => view! { <Empty text=Signal::derive(move || i18n.t("ui-dhcp-activity-empty")) /> }.into_any(),
                Some(Ok(l)) => view! {
                    <div class="max-h-96 overflow-auto rounded-lg border border-slate-200 dark:border-slate-800">
                        <table class="min-w-full divide-y divide-slate-200 text-xs dark:divide-slate-800">
                            <thead><tr>
                                <th class=TH>{tr("ui-dhcp-activity-time")}</th>
                                <th class=TH>{tr("ui-dhcp-activity-family")}</th>
                                <th class=TH>{tr("ui-dhcp-activity-exchange")}</th>
                                <th class=TH>{tr("ui-dhcp-lease-client")}</th>
                                <th class=TH>{tr("ui-dhcp-lease-addr")}</th>
                                <th class=TH>{tr("ui-dhcp-activity-note")}</th>
                            </tr></thead>
                            <tbody class="divide-y divide-slate-100 font-mono dark:divide-slate-800">
                                {l.into_iter().map(|a: Activity| {
                                    let ts = a.ts_ms;
                                    let exchange = match &a.reply {
                                        Some(r) => format!("{} → {r}", a.request),
                                        None => a.request.clone(),
                                    };
                                    view! {
                                        <tr>
                                            <td class=TD>{move || fmt::datetime(i18n.locale.get(), ts)}</td>
                                            <td class=TD>{a.family.to_uppercase()}</td>
                                            <td class=TD>{exchange}</td>
                                            <td class=TD>{a.client.clone()}</td>
                                            <td class=TD>{a.addrs.join(", ")}</td>
                                            <td class=TD>{a.note.clone().unwrap_or_default()}</td>
                                        </tr>
                                    }
                                }).collect_view()}
                            </tbody>
                        </table>
                    </div>
                }
                .into_any(),
            }}
        </section>
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn lists_and_reservations() {
        assert_eq!(split_list(" a, b c;d\n"), ["a", "b", "c", "d"]);
        let r = parse_reservations("02:00:00:00:00:01 10.0.0.5\n\nbad\n02-00-00-00-00-02,10.0.0.6");
        assert_eq!(r.len(), 2);
        assert_eq!(r[1].ip, "10.0.0.6");
        assert_eq!(
            reservations_text(&r),
            "02:00:00:00:00:01 10.0.0.5\n02-00-00-00-00-02 10.0.0.6"
        );
    }
}
