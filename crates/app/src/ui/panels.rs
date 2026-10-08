//! Properties panel and the new-device / new-port forms (spec §4.3).

use leptos::prelude::*;
use tsv_core::edit::ObjectId;
use tsv_core::ids::{CableId, DeviceId, PortId, RackId};
use tsv_core::model::{Document, PortKind};

use crate::forms::{
    color_from_input, color_to_input, new_device_input, new_port_input, parse_document_name,
    parse_height, parse_name,
};
use crate::interaction::DragSource;
use crate::ui::core::{now_s, read, signals, update};
use crate::ui::viewport::{forward_cancel, forward_move, forward_up};

/// What the properties panel shows for the current selection.
#[derive(Clone, PartialEq)]
enum Selected {
    Document {
        name: String,
    },
    Rack {
        id: RackId,
        name: String,
        height: u32,
    },
    Device {
        id: DeviceId,
        name: String,
        height: u32,
        color: String,
        position: String,
    },
    Port {
        id: PortId,
        name: String,
        kind: PortKind,
        /// The cable plugged into the port: its ID and name.
        cable: Option<(CableId, String)>,
    },
    Cable {
        id: CableId,
        name: String,
        color: String,
        a: String,
        b: String,
        /// "⚠ BNC to SMA" when the ends' connector types differ.
        warning: Option<String>,
    },
}

/// `Rack/Device/Port`, or "—" for a port that no longer exists.
fn port_path(doc: &Document, port: PortId) -> String {
    doc.port(port).map_or("—".to_string(), |(r, d, p)| {
        format!("{}/{}/{}", r.name, d.name, p.name)
    })
}

fn selected() -> Selected {
    read(|s| {
        let doc = s.document();
        match s.selection() {
            Some(ObjectId::Rack(id)) => doc.rack(id).map(|r| Selected::Rack {
                id,
                name: r.name.to_string(),
                height: r.height_u,
            }),
            Some(ObjectId::Device(id)) => doc.device(id).map(|(_, d)| Selected::Device {
                id,
                name: d.name.to_string(),
                height: d.height_u,
                color: color_to_input(d.color),
                position: if d.height_u == 1 {
                    format!("U{}", d.bottom_u)
                } else {
                    format!("U{}–U{}", d.bottom_u, d.top_u())
                },
            }),
            Some(ObjectId::Port(id)) => doc.port(id).map(|(_, _, p)| Selected::Port {
                id,
                name: p.name.to_string(),
                kind: p.kind,
                cable: doc.cable_at_port(id).map(|c| (c.id, c.name.to_string())),
            }),
            Some(ObjectId::Cable(id)) => doc.cable(id).map(|c| Selected::Cable {
                id,
                name: c.name.to_string(),
                color: color_to_input(c.color),
                a: port_path(doc, c.a),
                b: port_path(doc, c.b),
                warning: doc
                    .cable_mismatch(c)
                    .map(|(a, b)| format!("⚠ {} to {}", a.label(), b.label())),
            }),
            None => None,
        }
        .unwrap_or_else(|| Selected::Document {
            name: doc.name.to_string(),
        })
    })
}

#[component]
pub fn Properties() -> impl IntoView {
    let sig = signals();
    view! {
        <section class="panel">
            <h2>"Properties"</h2>
            {move || {
                sig.rev.track();
                match selected() {
                    Selected::Document { name } => view! {
                        <Field label="Document name" value=name check=check_document_name
                            commit=move |v: String| update(|s| s.rename_document(&v)) />
                    }
                        .into_any(),
                    Selected::Rack { id, name, height } => view! {
                        <Field label="Name" value=name check=check_name focus_for=ObjectId::Rack(id)
                            commit=move |v: String| update(|s| s.rename(ObjectId::Rack(id), &v)) />
                        <Field label="Height (U)" value=height.to_string() check=check_height
                            commit=move |v: String| update(|s| s.set_rack_height(id, &v)) />
                    }
                        .into_any(),
                    Selected::Device { id, name, height, color, position } => view! {
                        <Field label="Name" value=name check=check_name focus_for=ObjectId::Device(id)
                            commit=move |v: String| update(|s| s.rename(ObjectId::Device(id), &v)) />
                        <Field label="Height (U)" value=height.to_string() check=check_height
                            commit=move |v: String| update(|s| s.set_device_height(id, &v)) />
                        <label class="field">
                            <span>"Colour"</span>
                            <input
                                type="color"
                                prop:value=color
                                on:change=move |ev| {
                                    if let Some(c) = color_from_input(&event_target_value(&ev)) {
                                        update(|s| s.set_device_color(id, c));
                                    }
                                }
                            />
                        </label>
                        <div class="field"><span>"Position"</span><span>{position}</span></div>
                    }
                        .into_any(),
                    Selected::Port { id, name, kind, cable } => view! {
                        <Field label="Name" value=name check=check_name focus_for=ObjectId::Port(id)
                            commit=move |v: String| update(|s| s.rename(ObjectId::Port(id), &v)) />
                        <KindSelect value=kind on_change=move |k| update(|s| s.set_port_kind(id, k)) />
                        {
                            let (source, hint) = match &cable {
                                Some((cable, _)) => (DragSource::CableEnd { cable: *cable, end: id }, "⠿ Drag to re-plug"),
                                None => (DragSource::Cable { from: id }, "⠿ Drag to a port to connect"),
                            };
                            view! {
                                {cable.map(|(cable, cable_name)| view! {
                                    <div class="field">
                                        <span>"Cable"</span>
                                        <button
                                            class="link"
                                            on:click=move |_| update(|s| s.select(Some(ObjectId::Cable(cable))))
                                        >
                                            {cable_name}
                                        </button>
                                    </div>
                                })}
                                <div
                                    class="handle"
                                    on:pointerdown=move |ev| handle_down(&ev, Some(source.clone()))
                                    on:pointermove=move |ev| forward_move(&ev)
                                    on:pointerup=move |ev| forward_up(&ev)
                                    on:pointercancel=move |_| forward_cancel()
                                    on:lostpointercapture=move |_| forward_cancel()
                                >
                                    {hint}
                                </div>
                            }
                        }
                    }
                        .into_any(),
                    Selected::Cable { id, name, color, a, b, warning } => view! {
                        <Field label="Name" value=name check=check_name focus_for=ObjectId::Cable(id)
                            commit=move |v: String| update(|s| s.rename(ObjectId::Cable(id), &v)) />
                        <label class="field">
                            <span>"Colour"</span>
                            <input
                                type="color"
                                prop:value=color
                                on:change=move |ev| {
                                    if let Some(c) = color_from_input(&event_target_value(&ev)) {
                                        update(|s| s.set_cable_color(id, c));
                                    }
                                }
                            />
                        </label>
                        <div class="field"><span>"From"</span><span>{a}</span></div>
                        <div class="field"><span>"To"</span><span>{b}</span></div>
                        {warning.map(|w| view! { <div class="warning">{w}</div> })}
                    }
                        .into_any(),
                }
            }}
        </section>
    }
}

/// A connector-type dropdown.
#[component]
fn KindSelect(value: PortKind, on_change: impl Fn(PortKind) + 'static) -> impl IntoView {
    view! {
        <label class="field">
            <span>"Type"</span>
            <select
                prop:value=value.key()
                on:change=move |ev| {
                    if let Some(k) = PortKind::from_key(&event_target_value(&ev)) {
                        on_change(k);
                    }
                }
            >
                {PortKind::ALL
                    .iter()
                    .map(|k| view! { <option value=k.key() selected=*k == value>{k.label()}</option> })
                    .collect_view()}
            </select>
        </label>
    }
}

fn check_name(v: &str) -> Result<(), String> {
    let limits = read(|s| s.limits.clone());
    parse_name(v, &limits).map(|_| ())
}

fn check_height(v: &str) -> Result<(), String> {
    parse_height(v).map(|_| ())
}

fn check_document_name(v: &str) -> Result<(), String> {
    parse_document_name(v).map(|_| ())
}

/// A text field that commits on Enter or blur and shows the error inline.
#[component]
fn Field(
    label: &'static str,
    value: String,
    /// Validates while typing (`Err` is shown inline); `commit` re-checks against the document.
    check: fn(&str) -> Result<(), String>,
    /// The object whose name this field edits, if any; "Rename" in the 3D menu focuses it.
    #[prop(optional)]
    focus_for: Option<ObjectId>,
    commit: impl Fn(String) -> Result<(), String> + 'static,
) -> impl IntoView {
    let sig = signals();
    let text = RwSignal::new(value.clone());
    let error = RwSignal::new(None::<String>);
    let input_ref = NodeRef::<leptos::html::Input>::new();
    if let Some(id) = focus_for {
        Effect::new(move |_| {
            if sig.focus_name.get() == Some(id)
                && let Some(input) = input_ref.get()
            {
                let _ = input.focus();
                input.select();
                sig.focus_name.set(None);
            }
        });
    }
    let commit = std::rc::Rc::new(commit);
    let run = {
        let commit = commit.clone();
        move || {
            let v = text.get_untracked();
            if v == value || check(&v).is_err() {
                return;
            }
            if let Err(e) = commit(v) {
                error.set(Some(e));
            }
        }
    };
    let run_enter = run.clone();
    view! {
        <label class="field">
            <span>{label}</span>
            <input
                node_ref=input_ref
                prop:value=move || text.get()
                on:input=move |ev| {
                    let v = event_target_value(&ev);
                    error.set(check(&v).err());
                    text.set(v);
                }
                on:keydown=move |ev: leptos::ev::KeyboardEvent| {
                    if ev.key() == "Enter" {
                        run_enter();
                    }
                }
                on:blur=move |_| run()
            />
        </label>
        {move || error.get().map(|e| view! { <div class="field-error">{e}</div> })}
    }
}

/// A drag handle that starts `source` on pointer down, if the form is valid.
fn handle_down(ev: &leptos::ev::PointerEvent, source: Option<DragSource>) {
    let Some(source) = source else { return };
    ev.prevent_default();
    if let Some(target) = ev
        .target()
        .and_then(|t| wasm_bindgen::JsCast::dyn_into::<web_sys::Element>(t).ok())
    {
        let _ = target.set_pointer_capture(ev.pointer_id());
    }
    update(|s| s.start_drag(source, now_s()));
}

#[component]
pub fn NewDeviceForm() -> impl IntoView {
    let name = RwSignal::new(String::new());
    let height = RwSignal::new("1".to_string());
    let checked = move || {
        let limits = read(|s| s.limits.clone());
        new_device_input(&name.get(), &height.get(), &limits)
    };
    view! {
        <section class="panel">
            <h2>"New device"</h2>
            <label class="field">
                <span>"Name"</span>
                <input prop:value=move || name.get() on:input=move |ev| name.set(event_target_value(&ev)) />
            </label>
            <label class="field">
                <span>"Height (U)"</span>
                <input prop:value=move || height.get() on:input=move |ev| height.set(event_target_value(&ev)) />
            </label>
            {move || (!name.get().is_empty()).then(|| checked().err()).flatten()
                .map(|e| view! { <div class="field-error">{e}</div> })}
            <div
                class="handle"
                class:disabled=move || checked().is_err()
                on:pointerdown=move |ev| {
                    let source = checked().ok().map(|(name, height_u)| DragSource::NewDevice { name, height_u });
                    handle_down(&ev, source);
                }
                on:pointermove=move |ev| forward_move(&ev)
                on:pointerup=move |ev| forward_up(&ev)
                on:pointercancel=move |_| forward_cancel()
                on:lostpointercapture=move |_| forward_cancel()
            >
                "⠿ Drag into a rack"
            </div>
        </section>
    }
}

#[component]
pub fn NewPortForm() -> impl IntoView {
    let sig = signals();
    let name = RwSignal::new(String::new());
    let kind = RwSignal::new(PortKind::Unspecified);
    let device = move || {
        sig.rev.track();
        read(|s| {
            s.port_form_device().and_then(|d| {
                s.document()
                    .device(d)
                    .map(|(_, dev)| (d, dev.name.to_string()))
            })
        })
    };
    let checked = move || {
        let (id, _) = device().ok_or_else(|| "Select a device first".to_string())?;
        read(|s| new_port_input(s.document(), id, &name.get(), &s.limits)).map(|n| (id, n))
    };
    view! {
        <section class="panel">
            <h2>"New port"</h2>
            <div class="field">
                <span>"Device"</span>
                <span>{move || device().map_or("—".to_string(), |(_, n)| n)}</span>
            </div>
            <label class="field">
                <span>"Name"</span>
                <input
                    prop:value=move || name.get()
                    prop:disabled=move || device().is_none()
                    on:input=move |ev| name.set(event_target_value(&ev))
                />
            </label>
            <KindSelect value=PortKind::Unspecified on_change=move |k| kind.set(k) />
            {move || (!name.get().is_empty()).then(|| checked().err()).flatten()
                .map(|e| view! { <div class="field-error">{e}</div> })}
            <div
                class="handle"
                class:disabled=move || checked().is_err()
                on:pointerdown=move |ev| {
                    let source = checked().ok().map(|(device, name)| DragSource::NewPort {
                        device,
                        name,
                        kind: kind.get_untracked(),
                    });
                    handle_down(&ev, source);
                }
                on:pointermove=move |ev| forward_move(&ev)
                on:pointerup=move |ev| forward_up(&ev)
                on:pointercancel=move |_| forward_cancel()
                on:lostpointercapture=move |_| forward_cancel()
            >
                "⠿ Drag onto the device"
            </div>
        </section>
    }
}
