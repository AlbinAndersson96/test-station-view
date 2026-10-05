//! Properties panel and the new-device / new-port forms (spec §4.3).

use leptos::prelude::*;
use tsv_core::edit::ObjectId;
use tsv_core::ids::{DeviceId, RackId};

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
        id: tsv_core::ids::PortId,
        name: String,
    },
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
                        <Field label="Document name" value=name focus=true check=check_document_name
                            commit=move |v: String| update(|s| s.rename_document(&v)) />
                    }
                        .into_any(),
                    Selected::Rack { id, name, height } => view! {
                        <Field label="Name" value=name focus=true check=check_name
                            commit=move |v: String| update(|s| s.rename(ObjectId::Rack(id), &v)) />
                        <Field label="Height (U)" value=height.to_string() focus=false check=check_height
                            commit=move |v: String| update(|s| s.set_rack_height(id, &v)) />
                    }
                        .into_any(),
                    Selected::Device { id, name, height, color, position } => view! {
                        <Field label="Name" value=name focus=true check=check_name
                            commit=move |v: String| update(|s| s.rename(ObjectId::Device(id), &v)) />
                        <Field label="Height (U)" value=height.to_string() focus=false check=check_height
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
                    Selected::Port { id, name } => view! {
                        <Field label="Name" value=name focus=true check=check_name
                            commit=move |v: String| update(|s| s.rename(ObjectId::Port(id), &v)) />
                    }
                        .into_any(),
                }
            }}
        </section>
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
/// `focus`: this is the name field that "Rename" in the 3D context menu focuses.
#[component]
fn Field(
    label: &'static str,
    value: String,
    focus: bool,
    /// Validates while typing (`Err` is shown inline); `commit` re-checks against the document.
    check: fn(&str) -> Result<(), String>,
    commit: impl Fn(String) -> Result<(), String> + 'static,
) -> impl IntoView {
    let sig = signals();
    let text = RwSignal::new(value.clone());
    let error = RwSignal::new(None::<String>);
    let input_ref = NodeRef::<leptos::html::Input>::new();
    let seen_focus = sig.focus_name.get_untracked();
    if focus {
        Effect::new(move |_| {
            if sig.focus_name.get() != seen_focus
                && let Some(input) = input_ref.get()
            {
                let _ = input.focus();
                input.select();
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
            {move || (!name.get().is_empty()).then(|| checked().err()).flatten()
                .map(|e| view! { <div class="field-error">{e}</div> })}
            <div
                class="handle"
                class:disabled=move || checked().is_err()
                on:pointerdown=move |ev| {
                    let source = checked().ok().map(|(device, name)| DragSource::NewPort { device, name });
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
