use dioxus::{html::geometry::euclid::Point2D, prelude::*};
use dioxus_free_icons::Icon;
use floneum_plugin::exports::plugins::main::definitions::ValueType;
use floneum_plugin::PluginInstance;
use petgraph::{graph::NodeIndex, stable_graph::DefaultIx};
use serde::{Deserialize, Serialize};

use crate::edge::{Connection, ConnectionType};
use crate::graph::CurrentlyDragging;
use crate::input::Input;
use crate::node_value::{NodeInput, NodeOutput};
use crate::output::Output;
use crate::{use_application_state, Colored, CurrentlyDraggingProps, DraggingIndex, Edge};
use crate::{Point, VisualGraph};
use dioxus_signals::*;

const SNAP_DISTANCE: f32 = 15.;
pub const NODE_KNOB_SIZE: f64 = 5.;
pub const NODE_MARGIN: f64 = 2.;

#[derive(Serialize, Deserialize)]
pub struct Node {
    pub instance: PluginInstance,
    #[serde(skip)]
    pub running: bool,
    #[serde(skip)]
    pub queued: bool,
    #[serde(skip)]
    pub error: Option<String>,
    pub id: NodeIndex<DefaultIx>,
    pub position: Point,
    pub inputs: Vec<Signal<NodeInput>>,
    pub outputs: Vec<Signal<NodeOutput>>,
    pub width: f32,
    pub height: f32,
}

impl PartialEq for Node {
    fn eq(&self, other: &Self) -> bool {
        self.id == other.id
    }
}

impl Node {
    pub fn center(&self) -> Point2D<f32, f32> {
        (Point2D::new(self.position.x, self.position.y)
            - Point2D::new(self.width, self.height) / 2.)
            .to_point()
    }

    fn input_connections(&self) -> impl Iterator<Item = Connection> + '_ {
        (0..self.inputs.len())
            .filter_map(|index| {
                let input = self.inputs[index].read();
                if let ValueType::Single(_) = input.definition.ty {
                    Some(Connection {
                        index,
                        ty: ConnectionType::Single,
                    })
                } else {
                    None
                }
            })
            .chain((0..self.inputs.len()).flat_map(|index| {
                let input = self.inputs[index].read();
                let indexes = if let ValueType::Many(_) = input.definition.ty {
                    0..input.value.len()
                } else {
                    0..0
                };
                indexes.map(move |inner| Connection {
                    index,
                    ty: ConnectionType::Element(inner),
                })
            }))
    }

    pub fn output_pos(&self, index: usize) -> Point2D<f32, f32> {
        Point2D::new(
            self.position.x + self.width - 1.,
            self.position.y
                + ((index as f32 + 1.) * self.height / (self.outputs.len() as f32 + 1.)),
        )
    }

    pub fn input_array_add_element_pos(&self, index: usize) -> Point2D<f32, f32> {
        self.input_pos(Connection {
            index,
            ty: ConnectionType::Single,
        })
    }

    pub fn input_array_remove_element_pos(&self, index: usize) -> Point2D<f32, f32> {
        let mut pos = self.input_array_add_element_pos(index);
        pos.x += 14.;
        pos
    }

    pub fn input_pos(&self, index: Connection) -> Point2D<f32, f32> {
        match index.ty {
            ConnectionType::Single => self.single_input_pos(index.index),
            ConnectionType::Element(inner) => self.element_input_pos(index.index, inner),
        }
    }

    fn single_input_pos(&self, index: usize) -> Point2D<f32, f32> {
        Point2D::new(
            self.position.x - 1.,
            self.position.y
                + ((index as f32 + 1.) * self.height / (self.input_count() as f32 + 1.)),
        )
    }

    fn element_input_pos(&self, index: usize, inner: usize) -> Point2D<f32, f32> {
        Point2D::new(
            self.position.x + 10. - 1.,
            self.position.y
                + ((inner as f32 + index as f32 + 2.) * self.height
                    / (self.input_count() as f32 + 1.)),
        )
    }

    fn input_count(&self) -> usize {
        let mut inputs = self.inputs.len();
        for input_idx in 0..self.inputs.len() {
            if let Some(ValueType::Many(_)) = self.input_type(Connection {
                index: input_idx,
                ty: ConnectionType::Single,
            }) {
                inputs += self.inputs[input_idx].read().value.len();
            }
        }
        inputs
    }

    pub fn input_type(&self, index: Connection) -> Option<ValueType> {
        match index.ty {
            ConnectionType::Single => self
                .inputs
                .get(index.index)
                .map(|input| input.read().definition.ty),
            ConnectionType::Element(_) => self.element_input_type(index.index),
        }
    }

    pub fn element_input_type(&self, index: usize) -> Option<ValueType> {
        self.inputs
            .get(index)
            .and_then(|input| match &input.read().definition.ty {
                ValueType::Many(ty) => Some(ValueType::Single(*ty)),
                ValueType::Single(_) => None,
            })
    }

    pub fn input_color(&self, index: Connection) -> String {
        match self.input_type(index) {
            Some(ty) => ty.color(),
            None => "black".to_string(),
        }
    }

    pub fn output_type(&self, index: usize) -> Option<ValueType> {
        self.outputs
            .get(index)
            .map(|input| input.read().definition.ty)
    }

    pub fn output_color(&self, index: usize) -> String {
        match self.output_type(index) {
            Some(ty) => ty.color(),
            None => "black".to_string(),
        }
    }

    pub fn help_text(&self) -> String {
        self.instance.metadata().description.to_string()
    }
}

#[derive(Props, PartialEq)]
pub struct NodeProps {
    node: Signal<Node>,
}

pub fn Node(cx: Scope<NodeProps>) -> Element {
    let application = use_application_state(cx);
    let node = cx.props.node;
    let current_node = node.read();
    let current_node_id = current_node.id;
    let width = current_node.width;
    let height = current_node.height;
    let pos = current_node.position - Point::new(1., 0.);

    render! {
        // inputs
        (0..current_node.inputs.len()).map(|index| {
            rsx! {
                Input {
                    node: cx.props.node,
                    index: index,
                }
            }
        }),

        // center UI/Configuration
        foreignObject {
            x: "{pos.x}",
            y: "{pos.y}",
            width: width as f64,
            height: height as f64,
            onmousedown: move |evt| {
                let graph: VisualGraph = cx.consume_context().unwrap();
                {
                    let node = node.read();
                    enum Action {
                        Snap(DraggingIndex),
                        IncreaseArray(usize),
                        DecreaseArray(usize),
                    }
                    if let Some((action, dist))
                        = node.input_connections()
                            .map(|index| {
                                let input_pos = node.input_pos(index);
                                (
                                    Action::Snap(DraggingIndex::Input(index)),
                                    (input_pos.x - evt.page_coordinates().x as f32).powi(2)
                                        + (input_pos.y - evt.page_coordinates().y as f32).powi(2),
                                )
                            })
                            .chain(
                                (0..node.inputs.len())
                                    .map(|i| {
                                        let output_pos = node.input_array_add_element_pos(i);
                                        (
                                            Action::IncreaseArray(i),
                                            (output_pos.x - evt.page_coordinates().x as f32).powi(2)
                                                + (output_pos.y - evt.page_coordinates().y as f32).powi(2),
                                        )
                                    })
                            )
                            .chain(
                                (0..node.inputs.len())
                                    .map(|i| {
                                        let output_pos = node.input_array_remove_element_pos(i);
                                        (
                                            Action::DecreaseArray(i),
                                            (output_pos.x - evt.page_coordinates().x as f32).powi(2)
                                                + (output_pos.y - evt.page_coordinates().y as f32).powi(2),
                                        )
                                    })
                            )
                            .chain(
                                (0..node.outputs.len())
                                    .map(|i| {
                                        let output_pos = node.output_pos(i);
                                        (
                                            Action::Snap(DraggingIndex::Output(i)),
                                            (output_pos.x - evt.page_coordinates().x as f32).powi(2)
                                                + (output_pos.y - evt.page_coordinates().y as f32).powi(2),
                                        )
                                    }),
                            )
                            .min_by(|(_, a), (_, b)| a.partial_cmp(b).unwrap())
                    {
                        if dist < SNAP_DISTANCE.powi(2) {
                            match action {
                                Action::Snap(index) => {
                                    let mut current_graph = graph.inner.write();
                                    current_graph
                                        .currently_dragging = Some(
                                        CurrentlyDragging::Connection(CurrentlyDraggingProps {
                                            from: cx.props.node,
                                            index,
                                            to: Signal::new(
                                                Point2D::new(
                                                    evt.page_coordinates().x as f32,
                                                    evt.page_coordinates().y as f32,
                                                ),
                                            ),
                                        }),
                                    );
                                }
                                Action::IncreaseArray(index) => {
                                    drop(node);
                                    let node = cx.props.node.write();
                                    node.inputs[index].write().push_default_value();
                                }
                                Action::DecreaseArray(index) => {
                                    drop(node);
                                    let node = cx.props.node.write();
                                    node.inputs[index].write().pop_value();
                                }
                            }
                        } else {
                            graph.start_dragging_node(&evt, cx.props.node);
                        }
                    } else {
                        graph.start_dragging_node(&evt, cx.props.node);
                    }
                }
            },
            onmousemove: |evt| {
                let graph: VisualGraph = cx.consume_context().unwrap();
                graph.update_mouse(&evt);
            },
            onmouseup: move |evt| {
                let graph: VisualGraph = cx.consume_context().unwrap();
                {
                    if let Some(CurrentlyDragging::Connection(currently_dragging))
                        = {
                            let current_graph = graph.inner.read();
                            let val = current_graph.currently_dragging;
                            drop(current_graph);
                            val
                        }
                    {
                        let dist;
                        let edge;
                        let start_id;
                        let end_id;
                        match currently_dragging.index {
                            DraggingIndex::Output(input_node_idx) => {
                                let node = node.read();
                                let combined = node.input_connections()
                                    .map(|index| {
                                        let input_pos = node.input_pos(index);
                                        (
                                            index,
                                            (input_pos.x - evt.page_coordinates().x as f32).powi(2)
                                                + (input_pos.y - evt.page_coordinates().y as f32).powi(2),
                                        )
                                    })
                                    .min_by(|(_, a), (_, b)| a.partial_cmp(b).unwrap())
                                    .unwrap();
                                let output_node_idx = combined.0;
                                dist = combined.1;
                                let start_node = currently_dragging.from.read();
                                start_id = start_node.id;
                                end_id = current_node_id;
                                edge = Signal::new(Edge::new(input_node_idx, output_node_idx));
                            }
                            DraggingIndex::Input(output_node_idx) => {
                                let node = node.read();
                                let combined = (0..node.outputs.len())
                                    .map(|i| {
                                        let output_pos = node.output_pos(i);
                                        (
                                            i,
                                            (output_pos.x - evt.page_coordinates().x as f32).powi(2)
                                                + (output_pos.y - evt.page_coordinates().y as f32).powi(2),
                                        )
                                    })
                                    .min_by(|(_, a), (_, b)| a.partial_cmp(b).unwrap())
                                    .unwrap();
                                let input_node_idx = combined.0;
                                dist = combined.1;
                                let start_node = currently_dragging.from.read();
                                start_id = current_node_id;
                                end_id = start_node.id;
                                edge = Signal::new(Edge::new(input_node_idx, output_node_idx));
                            }
                        }
                        if dist < SNAP_DISTANCE.powi(2) {
                            graph.connect(start_id, end_id, edge);
                        }
                    }
                }
                graph.clear_dragging();

                // Focus or unfocus this node
                let mut application = application.write();
                match &application.currently_focused {
                    Some(currently_focused_node) if currently_focused_node == &cx.props.node => {
                        application.currently_focused = None;
                    }
                    _ => {
                        application.currently_focused = Some(cx.props.node);
                    }
                }
            },

            CenterNodeUI {
                node: cx.props.node,
            }
        }

        // outputs
        (0..current_node.outputs.len()).map(|i| {
            rsx! {
                Output {
                    node: cx.props.node,
                    index: i,
                }
            }
        })
    }
}

fn CenterNodeUI(cx: Scope<NodeProps>) -> Element {
    let application = use_application_state(cx);
    let focused = application.read().currently_focused == Some(cx.props.node);
    let node = cx.props.node;
    let current_node = node.read();
    let current_node_id = current_node.id;
    let name = &current_node.instance.metadata().name;
    let focused_class = if focused {
        "border-2 border-blue-500"
    } else {
        ""
    };

    render! {
        div {
            style: "-webkit-user-select: none; -ms-user-select: none; user-select: none; padding: {NODE_KNOB_SIZE*2.+2.}px;",
            class: "flex flex-col justify-center items-center w-full h-full border rounded-md {focused_class}",
            div {
                button {
                    class: "fixed p-2 top-0 right-0",
                    onclick: move |_| {
                        application.write().remove(node.read().id)
                    },
                    Icon {
                        width: 15,
                        height: 15,
                        fill: "black",
                        icon: dioxus_free_icons::icons::io_icons::IoTrashOutline,
                    }
                }
                h1 {
                    class: "text-md",
                    "{name}"
                }
                if current_node.running {
                    rsx! { div { "Loading..." } }
                }
                else {
                    rsx! {
                        button {
                            class: "p-1 border rounded-md hover:bg-gray-200",
                            onclick: move |_| {
                                if application.read().graph.set_input_nodes(current_node_id) {
                                    let mut current_node = cx.props.node.write();
                                    let inputs = current_node.inputs.iter().map(|input| input.read().value()).collect();
                                    log::trace!("Running node {:?} with inputs {:?}", current_node_id, inputs);
                                    current_node.running = true;
                                    current_node.queued = true;

                                    let fut = current_node.instance.run(inputs);
                                    let node = cx.props.node;
                                    cx.spawn(async move {
                                        match fut.await.as_deref() {
                                            Some(Ok(result)) => {
                                                let current_node = node.read();
                                                for (out, current) in result.iter().zip(current_node.outputs.iter()) {
                                                    current.write().value = out.clone();
                                                }
                                            }
                                            Some(Err(err)) => {
                                                log::error!("Error running node {:?}: {:?}", current_node_id, err);
                                                let mut node_mut = node.write();
                                                node_mut.error = Some(err.to_string());
                                            }
                                            None => {}
                                        }
                                        let mut current_node = node.write();
                                        current_node.running = false;
                                        current_node.queued = false;
                                    });
                                }
                            },
                            "Run"
                        }
                    }
                }
                div { color: "red",
                    if let Some(error) = &current_node.error {
                        rsx! {
                            p { "{error}" }
                        }
                    }
                }
            }
        }
    }
}