//! Per-field editor (TODO §2.5): one row per `FieldType` with an Auto / Manual
//! toggle and a generator editor. Manual values are parsed live through
//! `wpg_core::notation` (out-of-spec values are accepted so malformed packets can be
//! built, with the constraint diagnostics shown inline). Generators show a live
//! "expands to N" estimate from the stream cardinality; [`SendEstimate`] turns it
//! into a send time at the template's `tx.rate`.

use leptos::prelude::*;
use wpg_core::notation::{format_value, nsap_parts, parse_generator, parse_nsap};
use wpg_core::{FieldDef, FieldType, Generator, Length};
use wpg_template::{parse_text, FieldInput, GeneratorSpec};

use super::preview::{format_count, format_size, gen_cardinality, PreviewCtx};
use super::proto;
use super::rate::{estimate_secs, format_secs, EstRate};
use super::state::{edit, Builder};
use crate::components::{INPUT, INPUT_INVALID, LABEL};
use crate::i18n::{tr, use_i18n};

/// How a field is currently valued.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Mode {
    /// Computed (auto rule) or definition default.
    Auto,
    /// A fixed Manual value.
    Manual,
    /// A generator varies it across the stream.
    Gen,
}

fn short_type(ty: &FieldType) -> String {
    match ty {
        FieldType::UInt { bits } => format!("u{bits}"),
        FieldType::Int { bits } => format!("i{bits}"),
        FieldType::Enum { .. } => "enum".into(),
        FieldType::Flags { .. } => "flags".into(),
        FieldType::Mac => "mac".into(),
        FieldType::Ipv4 => "ipv4".into(),
        FieldType::Ipv6 => "ipv6".into(),
        FieldType::Nsap => "nsap".into(),
        FieldType::CString => "cstr".into(),
        FieldType::Bytes { .. } => "bytes".into(),
        FieldType::Tlv(_) => "tlv".into(),
        FieldType::List { .. } => "list".into(),
        FieldType::Padding { .. } => "pad".into(),
        FieldType::Raw => "raw".into(),
    }
}

fn can_generate(ty: &FieldType) -> bool {
    matches!(
        ty,
        FieldType::UInt { .. }
            | FieldType::Int { .. }
            | FieldType::Enum { .. }
            | FieldType::Flags { .. }
            | FieldType::Ipv4
            | FieldType::Ipv6
            | FieldType::Mac
            | FieldType::Nsap
    )
}

#[component]
pub(crate) fn FieldEditor() -> impl IntoView {
    let builder = Builder::use_ctx();

    move || {
        let sel = builder.sel_layer.get();
        let t = builder.template.get();
        let Some((li, proto_id)) =
            sel.and_then(|i| t.layers.get(i).map(|l| (i, l.protocol.clone())))
        else {
            return view! {
                <p class="rounded-md border border-dashed border-slate-300 px-4 py-8 text-center text-sm text-slate-500 dark:border-slate-700">
                    {tr("ui-bld-select-layer")}
                </p>
            }.into_any();
        };
        let Some(def) = proto::def(&proto_id) else {
            return view! { <p class="text-sm text-rose-600">{tr("ui-bld-unknown")}</p> }
                .into_any();
        };
        let fields: Vec<FieldDef> = def.fields.clone();
        let name = def.name.clone();
        view! {
            <section aria-label=tr("ui-bld-fields") class="flex h-full min-h-0 flex-col">
                <div class="mb-2 flex items-baseline justify-between gap-2">
                    <h2 class="truncate text-sm font-semibold">{name} <span class="font-mono text-xs text-slate-400">{proto_id.clone()}</span></h2>
                    <StreamEstimate />
                </div>
                <div class="min-h-0 flex-1 space-y-3 overflow-y-auto pr-1">
                    {fields.into_iter().map(|fd| field_row(builder, li, &fd)).collect_view()}
                </div>
            </section>
        }.into_any()
    }
}

/// Stream-wide "expands to N / ~size" estimate.
#[component]
fn StreamEstimate() -> impl IntoView {
    let i18n = use_i18n();
    let pv = PreviewCtx::use_ctx();
    move || {
        let p = pv.preview.get();
        (!p.layout.is_empty() || p.cardinality.is_some()).then(|| {
            let count = format_count(p.cardinality);
            let size = format_size(p.estimated_bytes);
            view! {
                <span class="shrink-0 text-[11px] text-slate-500" title=i18n.t("ui-bld-estimate")>
                    {i18n.ta("ui-bld-expands", &[("count", count), ("bytes", size)])}
                </span>
            }
        })
    }
}

/// Transmit-rate input (`tx.rate`) with the estimated time to send the stream.
#[component]
pub(crate) fn SendEstimate() -> impl IntoView {
    let i18n = use_i18n();
    let builder = Builder::use_ctx();
    let pv = PreviewCtx::use_ctx();
    let rate = Memo::new(move |_| {
        builder
            .template
            .with(|t| t.tx.rate.clone().unwrap_or_default())
    });
    let invalid = move || {
        let r = rate.get();
        !r.trim().is_empty() && EstRate::parse(&r).is_none()
    };
    let set_rate = move |ev: leptos::ev::Event| {
        let v = event_target_value(&ev);
        builder.edit(true, move |t| {
            t.tx.rate = (!v.trim().is_empty()).then_some(v);
        });
    };
    let estimate = move || {
        let secs = builder.template.with(|t| {
            pv.preview
                .with(|p| {
                    (!p.bytes.is_empty())
                        .then(|| estimate_secs(&t.tx, p.cardinality, p.bytes.len()))
                })
                .flatten()
        });
        match secs {
            Some(s) => i18n.ta("ui-bld-duration", &[("time", format_secs(s))]),
            None => i18n.t("ui-bld-duration-unknown"),
        }
    };
    view! {
        <div class="mt-2 flex items-center gap-2 border-t border-slate-200 pt-2 dark:border-slate-800">
            <label for="bld-tx-rate" class="shrink-0 text-xs text-slate-500">{tr("ui-bld-rate")}</label>
            <input
                id="bld-tx-rate"
                type="text"
                class=move || if invalid() { INPUT_INVALID } else { INPUT }
                style="max-width:8rem"
                placeholder="1000pps"
                title=i18n.t("ui-bld-rate-hint")
                aria-invalid=move || invalid().to_string()
                prop:value=move || rate.get()
                on:input=set_rate
            />
            <span class="truncate text-[11px] text-slate-500" aria-live="polite" title=i18n.t("ui-bld-duration-hint")>
                {estimate}
            </span>
        </div>
    }
}

/// One field row (mode toggle + editor).
fn field_row(builder: Builder, layer: usize, fd: &FieldDef) -> impl IntoView {
    let name = fd.name.clone();
    let ty = fd.ty.clone();
    let has_auto = fd.auto.is_some();
    let can_gen = can_generate(&ty);

    let n_mode = name.clone();
    let ty_pad = matches!(ty, FieldType::Padding { .. });
    let mode = Memo::new(move |_| {
        builder.template.with(|t| {
            let Some(l) = t.layers.get(layer) else {
                return Mode::Auto;
            };
            if l.generators.contains_key(&n_mode) {
                Mode::Gen
            } else if let Some(inp) = l.fields.get(&n_mode) {
                if inp.is_auto() {
                    Mode::Auto
                } else {
                    Mode::Manual
                }
            } else {
                let _ = (has_auto, ty_pad);
                Mode::Auto
            }
        })
    });

    let seed_text = move |field: &str, ty: &FieldType| -> String {
        PreviewCtx::use_ctx()
            .preview
            .with_untracked(|p| p.field_value(layer, field).map(|v| format_value(v, ty)))
            .unwrap_or_default()
    };

    let n_manual = name.clone();
    let ty_manual = ty.clone();
    let to_manual = move |_| {
        let seed = seed_text(&n_manual, &ty_manual);
        let n = n_manual.clone();
        builder.edit(false, move |t| {
            edit::clear_generator(t, layer, &n);
            edit::set_field(t, layer, &n, FieldInput::Str(seed));
        });
    };
    let n_auto = name.clone();
    let to_auto = move |_| {
        let n = n_auto.clone();
        builder.edit(false, move |t| {
            edit::clear_generator(t, layer, &n);
            edit::clear_field(t, layer, &n);
        });
    };
    let n_gen = name.clone();
    let ty_gen = ty.clone();
    let to_gen = move |_| {
        let seed = seed_text(&n_gen, &ty_gen);
        let n = n_gen.clone();
        builder.edit(false, move |t| {
            edit::clear_field(t, layer, &n);
            edit::set_generator(t, layer, &n, GeneratorSpec::Text(seed));
        });
    };

    let n_sel = name.clone();
    let select_field = move |_| {
        let range = PreviewCtx::use_ctx().preview.with_untracked(|p| {
            p.layout.get(layer).and_then(|l| {
                l.fields
                    .iter()
                    .find(|f| f.name == n_sel)
                    .map(|f| (f.bit_offset, f.bit_len))
            })
        });
        builder.sel_field.set(Some(n_sel.clone()));
        builder.range.set(range);
    };

    let tybadge = short_type(&ty);
    let btn = "rounded px-1.5 py-0.5 text-[11px] font-medium";
    let active = "bg-sky-600 text-white";
    let idle =
        "bg-slate-100 text-slate-600 hover:bg-slate-200 dark:bg-slate-700 dark:text-slate-300";
    let name_h = name.clone();
    let name_group = name.clone();
    let fd_manual = fd.clone();
    let fd_gen = fd.clone();
    let name_auto = name.clone();
    // Localised field description (`desc = "..."` in the protocol definition) as a
    // tooltip on the field name; `None` for fields without one.
    let i18n = use_i18n();
    let desc_key = fd.description_key.clone();
    let has_desc = desc_key.is_some();
    let description = move || desc_key.as_deref().map(|k| i18n.t(k));
    let name_class = if has_desc {
        "min-w-0 flex-1 truncate text-sm font-medium underline decoration-dotted decoration-slate-400 underline-offset-2 cursor-help"
    } else {
        "min-w-0 flex-1 truncate text-sm font-medium"
    };

    view! {
        <div class="rounded-md border border-slate-200 p-2 dark:border-slate-800" role="group" aria-label=name_group on:click=select_field>
            <div class="flex items-center gap-2">
                <span class=name_class title=description>{name_h}</span>
                <code class="shrink-0 rounded bg-slate-100 px-1 font-mono text-[10px] text-slate-500 dark:bg-slate-800">{tybadge}</code>
                <div class="flex shrink-0 gap-0.5" on:click=|ev| ev.stop_propagation()>
                    {has_auto.then(|| {
                        let cls = move || format!("{btn} {}", if mode.get() == Mode::Auto { active } else { idle });
                        view! { <button type="button" class=cls on:click=to_auto>{tr("ui-bld-auto")}</button> }
                    })}
                    {
                        let cls = move || format!("{btn} {}", if mode.get() == Mode::Manual { active } else { idle });
                        view! { <button type="button" class=cls on:click=to_manual>{tr("ui-bld-manual")}</button> }
                    }
                    {can_gen.then(|| {
                        let cls = move || format!("{btn} {}", if mode.get() == Mode::Gen { active } else { idle });
                        view! { <button type="button" class=cls on:click=to_gen>{tr("ui-bld-gen")}</button> }
                    })}
                </div>
            </div>
            <div class="mt-1.5" on:click=|ev| ev.stop_propagation()>
                {move || match mode.get() {
                    Mode::Manual => manual_editor(builder, layer, &fd_manual).into_any(),
                    Mode::Gen => generator_editor(builder, layer, &fd_gen).into_any(),
                    Mode::Auto => auto_view(layer, name_auto.clone()).into_any(),
                }}
            </div>
        </div>
    }
}

/// Read-only Auto / default value from the last build.
fn auto_view(layer: usize, field: String) -> impl IntoView {
    let i18n = use_i18n();
    let pv = PreviewCtx::use_ctx();
    move || {
        let text = pv.preview.with(|p| {
            p.layout
                .get(layer)
                .and_then(|l| {
                    l.fields
                        .iter()
                        .find(|f| f.name == field)
                        .map(|f| (l.protocol.clone(), &f.value))
                })
                .map(|(proto, v)| {
                    proto::field_type(&proto, &field)
                        .map_or_else(|| format!("{v:?}"), |t| format_value(v, &t))
                })
        });
        view! {
            <p class="font-mono text-xs text-slate-500">{text.unwrap_or_else(|| i18n.t("ui-bld-computed"))}</p>
        }
    }
}

/// Manual value editor for the field's type.
fn manual_editor(builder: Builder, layer: usize, fd: &FieldDef) -> AnyView {
    let ty = fd.ty.clone();
    let name = fd.name.clone();

    let n_val = name.clone();
    let value = Memo::new(move |_| {
        builder.template.with(|t| {
            t.layers
                .get(layer)
                .and_then(|l| l.fields.get(&n_val))
                .and_then(FieldInput::scalar_text)
                .unwrap_or_default()
        })
    });

    // Flags: checkboxes + raw value.
    if let FieldType::Flags { flags, .. } = &ty {
        let flags = flags.clone();
        let ty_cur = ty.clone();
        let cur = Memo::new(move |_| {
            parse_text(&value.get(), &ty_cur)
                .ok()
                .and_then(|v| v.as_u64())
                .unwrap_or(0)
        });
        return view! {
            <div class="flex flex-wrap gap-2">
                {flags.into_iter().map(|fb| {
                    let bit = fb.bit;
                    let n = name.clone();
                    view! {
                        <label class="flex items-center gap-1 text-xs">
                            <input type="checkbox" prop:checked=move || cur.get() & (1u64 << bit) != 0
                                on:change=move |ev| {
                                    let on = event_target_checked(&ev);
                                    let mut v = cur.get_untracked();
                                    if on { v |= 1u64 << bit; } else { v &= !(1u64 << bit); }
                                    let n = n.clone();
                                    builder.edit(true, move |t| edit::set_field(t, layer, &n, FieldInput::Str(format!("{v:#x}"))));
                                }
                            />
                            {fb.name}
                        </label>
                    }
                }).collect_view()}
            </div>
        }.into_any();
    }

    // TLV / list: structured JSON editor.
    if matches!(ty, FieldType::Tlv(_) | FieldType::List { .. }) {
        let n_json = name.clone();
        let json = Memo::new(move |_| {
            builder.template.with(|t| {
                t.layers
                    .get(layer)
                    .and_then(|l| l.fields.get(&n_json))
                    .map(|inp| serde_json::to_string_pretty(inp).unwrap_or_default())
                    .unwrap_or_else(|| "[]".into())
            })
        });
        let n_set = name.clone();
        return view! {
            <textarea
                class="h-24 w-full resize-y rounded border border-slate-300 bg-slate-50 p-1.5 font-mono text-[11px] dark:border-slate-700 dark:bg-slate-950"
                spellcheck="false"
                prop:value=move || json.get()
                on:input=move |ev| {
                    let text = event_target_value(&ev);
                    if let Ok(inp) = serde_json::from_str::<FieldInput>(&text) {
                        let n = n_set.clone();
                        builder.edit(true, move |t| edit::set_field(t, layer, &n, inp));
                    }
                }
            ></textarea>
        }.into_any();
    }

    let ty_err = ty.clone();
    let err = Memo::new(move |_| {
        let v = value.get();
        if v.trim().is_empty() {
            None
        } else {
            parse_text(&v, &ty_err).err()
        }
    });
    let placeholder = placeholder_for(&ty);
    let hint = matches!(ty, FieldType::Nsap).then(|| nsap_hint(value));
    let n_set = name.clone();
    let datalist_id = format!("dl-{layer}-{name}");
    view! {
        <div>
            <input
                type="text"
                list=datalist_id.clone()
                class=move || if err.get().is_some() { INPUT_INVALID } else { INPUT }
                placeholder=placeholder
                prop:value=move || value.get()
                on:input=move |ev| {
                    let text = event_target_value(&ev);
                    let n = n_set.clone();
                    builder.edit(true, move |t| edit::set_field(t, layer, &n, FieldInput::Str(text)));
                }
            />
            {enum_datalist(&ty, datalist_id)}
            {hint}
            {move || err.get().map(|e| view! { <p class="mt-0.5 text-[11px] text-rose-600" role="alert">{e}</p> })}
        </div>
    }.into_any()
}

/// NSAP structure under the input: AFI / area / system ID / NSEL of the typed
/// address (nothing while it does not parse or is too short to split).
fn nsap_hint(value: Memo<String>) -> impl IntoView {
    let i18n = use_i18n();
    move || {
        let b = parse_nsap(&value.get()).ok()?;
        let p = nsap_parts(&b)?;
        let text = i18n.ta(
            "ui-bld-nsap-parts",
            &[
                ("afi", format!("{:02x}", p.afi)),
                ("area", hex_groups(p.area)),
                ("sys", hex_groups(p.system_id)),
                ("nsel", format!("{:02x}", p.nsel)),
            ],
        );
        Some(view! { <p class="mt-0.5 font-mono text-[11px] text-slate-500">{text}</p> })
    }
}

/// Octets as hex in dot-separated 2-octet groups (`1921.6800.1001`); `-` if empty.
fn hex_groups(b: &[u8]) -> String {
    if b.is_empty() {
        return "-".into();
    }
    b.chunks(2)
        .map(|c| c.iter().map(|x| format!("{x:02x}")).collect::<String>())
        .collect::<Vec<_>>()
        .join(".")
}

/// A `<datalist>` of enum variant names (empty for other types).
fn enum_datalist(ty: &FieldType, id: String) -> impl IntoView {
    let variants = if let FieldType::Enum { variants, .. } = ty {
        variants.iter().map(|v| v.name.clone()).collect::<Vec<_>>()
    } else {
        Vec::new()
    };
    (!variants.is_empty()).then(|| {
        view! {
            <datalist id=id>
                {variants.into_iter().map(|n| view! { <option value=n></option> }).collect_view()}
            </datalist>
        }
    })
}

fn placeholder_for(ty: &FieldType) -> &'static str {
    match ty {
        FieldType::Ipv4 => "192.0.2.1",
        FieldType::Ipv6 => "2001:db8::1",
        FieldType::Mac => "00:11:22:33:44:55",
        FieldType::Nsap => "49.0001.1921.6800.1001.00",
        FieldType::Bytes {
            len: Length::Until { .. },
        } => "\"text\"",
        FieldType::Bytes { .. } | FieldType::Raw => "de ad be ef",
        FieldType::CString => "\"boot.bin\"",
        FieldType::Enum { .. } => "name or 0x…",
        _ => "0x… / 123",
    }
}

/// Generator editor: kind picker + inputs + live estimate.
fn generator_editor(builder: Builder, layer: usize, fd: &FieldDef) -> AnyView {
    let i18n = use_i18n();
    let name = fd.name.clone();
    let ty = fd.ty.clone();

    let n_spec = name.clone();
    let spec = Memo::new(move |_| {
        builder
            .template
            .with(|t| {
                t.layers
                    .get(layer)
                    .and_then(|l| l.generators.get(&n_spec).cloned())
            })
            .unwrap_or_else(|| GeneratorSpec::Text(String::new()))
    });

    let kinds = [
        ("text", "ui-bld-gen-shorthand"),
        ("random", "ui-bld-gen-random"),
        ("random_mac", "ui-bld-gen-random-mac"),
        ("counter", "ui-bld-gen-counter"),
        ("file", "ui-bld-gen-file"),
        ("expr", "ui-bld-gen-expr"),
    ];
    let cur_kind = Memo::new(move |_| match spec.get() {
        GeneratorSpec::Text(_) => "text",
        GeneratorSpec::Full(g) => match g {
            Generator::Random { .. } => "random",
            Generator::RandomMac { .. } => "random_mac",
            Generator::Counter { .. } => "counter",
            Generator::File { .. } => "file",
            Generator::Expr { .. } => "expr",
            Generator::Range { .. } | Generator::List { .. } => "text",
        },
    });
    let n_kind = name.clone();
    let set_kind = move |kind: String| {
        let g = match kind.as_str() {
            "random" => GeneratorSpec::Full(Generator::Random {
                min: None,
                max: None,
                seed: None,
            }),
            "random_mac" => GeneratorSpec::Full(Generator::RandomMac {
                oui: String::new(),
                seed: None,
            }),
            "counter" => GeneratorSpec::Full(Generator::Counter {
                start: 0,
                step: 1,
                wrap: None,
            }),
            "file" => GeneratorSpec::Full(Generator::File {
                path: String::new(),
            }),
            "expr" => GeneratorSpec::Full(Generator::Expr {
                expr: String::new(),
            }),
            _ => GeneratorSpec::Text(String::new()),
        };
        let n = n_kind.clone();
        builder.edit(false, move |t| edit::set_generator(t, layer, &n, g));
    };

    let ty_est = ty.clone();
    let estimate = Memo::new(move |_| format_count(gen_cardinality(&spec.get(), &ty_est)));

    let n_body = name.clone();
    let ty_body = ty.clone();
    let body = move || generator_body(builder, layer, &n_body, &ty_body, spec.get());

    let sel_id = format!("gk-{layer}-{name}");
    view! {
        <div class="space-y-1.5">
            <div class="flex flex-wrap items-center gap-1">
                <label class=LABEL for=sel_id.clone()>{tr("ui-bld-gen-kind")}</label>
                <select
                    id=sel_id
                    class="rounded border border-slate-300 bg-white px-1.5 py-0.5 text-xs dark:border-slate-700 dark:bg-slate-900"
                    on:change=move |ev| set_kind(event_target_value(&ev))
                >
                    {kinds.into_iter().map(|(k, key)| {
                        view! { <option value=k selected=move || cur_kind.get() == k>{move || i18n.t(key)}</option> }
                    }).collect_view()}
                </select>
                <span class="ml-auto text-[11px] text-slate-500">{tr("ui-bld-gen-count")}</span>
                <span class="font-mono text-[11px] font-semibold text-sky-700 dark:text-sky-400">{move || estimate.get()}</span>
            </div>
            {body}
        </div>
    }.into_any()
}

fn opt_text(s: String) -> Option<String> {
    (!s.trim().is_empty()).then_some(s)
}

/// The inputs for the chosen generator kind.
fn generator_body(
    builder: Builder,
    layer: usize,
    name: &str,
    ty: &FieldType,
    spec: GeneratorSpec,
) -> AnyView {
    let name = name.to_owned();
    match spec {
        GeneratorSpec::Text(s) => {
            let ty_err = ty.clone();
            let s_err = s.clone();
            let err = move || {
                (!s_err.trim().is_empty())
                    .then(|| parse_generator(&s_err, &ty_err).err())
                    .flatten()
            };
            let n = name.clone();
            view! {
                <div>
                    <input type="text" class=INPUT placeholder="10.0.0.1-10.0.0.254 / 1,5,9 / 10.0.0.0/24 / 00:11:22:*"
                        prop:value=s
                        on:input=move |ev| {
                            let g = GeneratorSpec::Text(event_target_value(&ev));
                            let n = n.clone();
                            builder.edit(true, move |t| edit::set_generator(t, layer, &n, g));
                        }
                    />
                    {move || err().map(|e| view! { <p class="mt-0.5 text-[11px] text-rose-600">{e.to_string()}</p> })}
                </div>
            }.into_any()
        }
        GeneratorSpec::Full(Generator::Random { min, max, seed }) => {
            let (n1, n2, n3) = (name.clone(), name.clone(), name.clone());
            let (min0, max0) = (
                min.clone().unwrap_or_default(),
                max.clone().unwrap_or_default(),
            );
            let (max_a, min_b) = (max.clone(), min.clone());
            view! {
                <div class="grid grid-cols-3 gap-1">
                    <input type="text" class=INPUT placeholder="min" prop:value=min0
                        on:input=move |ev| {
                            let g = GeneratorSpec::Full(Generator::Random { min: opt_text(event_target_value(&ev)), max: max_a.clone(), seed });
                            let n = n1.clone();
                            builder.edit(true, move |t| edit::set_generator(t, layer, &n, g));
                        } />
                    <input type="text" class=INPUT placeholder="max" prop:value=max0
                        on:input=move |ev| {
                            let g = GeneratorSpec::Full(Generator::Random { min: min_b.clone(), max: opt_text(event_target_value(&ev)), seed });
                            let n = n2.clone();
                            builder.edit(true, move |t| edit::set_generator(t, layer, &n, g));
                        } />
                    <input type="number" class=INPUT placeholder="seed" prop:value=seed.map(|x| x.to_string()).unwrap_or_default()
                        on:input=move |ev| {
                            let sd = event_target_value(&ev).parse::<u64>().ok();
                            let g = GeneratorSpec::Full(Generator::Random { min: min.clone(), max: max.clone(), seed: sd });
                            let n = n3.clone();
                            builder.edit(true, move |t| edit::set_generator(t, layer, &n, g));
                        } />
                </div>
            }.into_any()
        }
        GeneratorSpec::Full(Generator::RandomMac { oui, seed }) => {
            let n = name.clone();
            view! {
                <input type="text" class=INPUT placeholder="00:11:22 (OUI)" prop:value=oui
                    on:input=move |ev| {
                        let g = GeneratorSpec::Full(Generator::RandomMac { oui: event_target_value(&ev), seed });
                        let n = n.clone();
                        builder.edit(true, move |t| edit::set_generator(t, layer, &n, g));
                    } />
            }.into_any()
        }
        GeneratorSpec::Full(Generator::Counter { start, step, wrap }) => {
            let (n1, n2, n3) = (name.clone(), name.clone(), name.clone());
            view! {
                <div class="grid grid-cols-3 gap-1">
                    <input type="number" class=INPUT placeholder="start" prop:value=start.to_string()
                        on:input=move |ev| {
                            let v = event_target_value(&ev).parse().unwrap_or(0);
                            let g = GeneratorSpec::Full(Generator::Counter { start: v, step, wrap });
                            let n = n1.clone();
                            builder.edit(true, move |t| edit::set_generator(t, layer, &n, g));
                        } />
                    <input type="number" class=INPUT placeholder="step" prop:value=step.to_string()
                        on:input=move |ev| {
                            let v = event_target_value(&ev).parse().unwrap_or(1);
                            let g = GeneratorSpec::Full(Generator::Counter { start, step: v, wrap });
                            let n = n2.clone();
                            builder.edit(true, move |t| edit::set_generator(t, layer, &n, g));
                        } />
                    <input type="number" class=INPUT placeholder="wrap" prop:value=wrap.map(|x| x.to_string()).unwrap_or_default()
                        on:input=move |ev| {
                            let v = event_target_value(&ev).parse::<u64>().ok();
                            let g = GeneratorSpec::Full(Generator::Counter { start, step, wrap: v });
                            let n = n3.clone();
                            builder.edit(true, move |t| edit::set_generator(t, layer, &n, g));
                        } />
                </div>
            }.into_any()
        }
        GeneratorSpec::Full(Generator::File { path }) => {
            let n = name.clone();
            view! {
                <input type="text" class=INPUT placeholder="ips.txt" prop:value=path
                    on:input=move |ev| {
                        let g = GeneratorSpec::Full(Generator::File { path: event_target_value(&ev) });
                        let n = n.clone();
                        builder.edit(true, move |t| edit::set_generator(t, layer, &n, g));
                    } />
            }.into_any()
        }
        GeneratorSpec::Full(Generator::Expr { expr }) => {
            let n = name.clone();
            view! {
                <input type="text" class=INPUT placeholder="$ipv4.src + idx" prop:value=expr
                    on:input=move |ev| {
                        let g = GeneratorSpec::Full(Generator::Expr { expr: event_target_value(&ev) });
                        let n = n.clone();
                        builder.edit(true, move |t| edit::set_generator(t, layer, &n, g));
                    } />
            }.into_any()
        }
        GeneratorSpec::Full(g) => {
            let text = match g {
                Generator::Range { start, end, .. } => format!("{start}-{end}"),
                Generator::List { values } => values.join(","),
                _ => String::new(),
            };
            let n = name.clone();
            view! {
                <input type="text" class=INPUT prop:value=text
                    on:input=move |ev| {
                        let g = GeneratorSpec::Text(event_target_value(&ev));
                        let n = n.clone();
                        builder.edit(true, move |t| edit::set_generator(t, layer, &n, g));
                    } />
            }
            .into_any()
        }
    }
}
