use std::{collections::HashMap, fmt::Write};

use glam::{IVec3, Vec3};
use sti::{define_key, key::Key, vec::KVec};

use crate::{hsl_to_hex, structures::strct::{rotate_block_vector, StructureKind}, voxel_world::VoxelWorld};

use super::{StructureId, Structures};


define_key!(pub NodeId(u32));
define_key!(pub SccId(u32));

impl Structures {
    pub fn belts(&self, world: &VoxelWorld) -> Belts {
        let mut struct_to_node : HashMap<StructureId, NodeId> = HashMap::new();
        let mut nodes : KVec<NodeId, Option<Node>> = KVec::new();

        // create graph
        for (k, structure) in self.structs.iter() {
            let id = StructureId(k);
            if structure.data.as_kind() != StructureKind::Belt {
                continue;
            }

            if let Some(&node) = struct_to_node.get(&id) {
                if nodes[node].is_some() {
                    continue;
                }
            }


            let mut output = None;
            let positions = [
                IVec3::new(-1,  0,  0),
                IVec3::new(-1,  1,  0),
                IVec3::new(-1, -1,  0),
            ];

            for position in positions {
                let position = rotate_block_vector(structure.direction, position);
                let position = structure.position + position;
                if let Some(&output_structure) = world.structure_blocks.get(&position) {
                    let structure = self.get(output_structure);
                    if structure.data.as_kind() == StructureKind::Belt {
                        let node_id = if let Some(&node_id) = struct_to_node.get(&output_structure) {
                            node_id
                        } else {
                            let node_id = nodes.push(None);
                            struct_to_node.insert(output_structure, node_id);
                            node_id
                        };

                        output = Some(node_id);
                        break;
                    }
                }
            }

            let node = Node {
                outputs: [output],
                structure_id: id,
            };


            if let Some(&id) = struct_to_node.get(&id) {
                nodes[id] = Some(node)
            } else {
                let nid = nodes.push(Some(node));
                struct_to_node.insert(id, nid);
            }
        }




        let mut scc_data : KVec<SccId, NodeId> = KVec::with_cap(nodes.len());
        let mut scc_ends : KVec<SccId, SccId> = KVec::with_cap(nodes.len());

        let mut node_to_scc : KVec<NodeId, SccId> = KVec::from_value(nodes.len(), SccId(u32::MAX));

        let mut next_index = 0;
        let mut scc_nodes = KVec::from_value(nodes.len(),
                            SccNode { index: u32::MAX, low_link: u32::MAX, on_stack: false });

        let mut scc_stack = Vec::with_capacity(64);
        let mut visit_stack = Vec::with_capacity(64);

        for index in nodes.krange() {
            if scc_nodes[index].index != u32::MAX { continue; }

            visit_stack.push((None, index, None));

            'recurse:
            while let Some((parent, index, iter)) = visit_stack.pop() {
                let mut iter : &[Option<NodeId>] = match iter {
                    None => {
                        let node = &mut scc_nodes[index];
                        node.index = next_index;
                        node.low_link = next_index;
                        node.on_stack = true;
                        scc_stack.push(index);

                        next_index += 1;

                        &*&nodes[index].as_ref().unwrap().outputs
                    },
                    Some(v) => v,
                };

                while let Some((&dst, rest)) = iter.split_first() {
                    iter = rest;

                    let Some(dst) = dst
                    else { continue };

                    let dst_node = &scc_nodes[dst];
                    if dst_node.index == u32::MAX {
                        visit_stack.push((parent, index, Some(iter)));
                        visit_stack.push((Some(index), dst, None));
                        continue 'recurse;
                    } else if dst_node.on_stack {
                        scc_nodes[index].low_link = scc_nodes[index].low_link.min(dst_node.low_link);
                    }
                }

                if let Some(parent) = parent {
                    scc_nodes[parent].low_link = scc_nodes[parent].low_link.min(scc_nodes[index].low_link);
                }

                let node = &scc_nodes[index];

                if node.index == node.low_link {
                    let scc_id = scc_ends.klen();

                    let begin = scc_data.klen();
                    while let Some(scc_index) = scc_stack.pop() {
                        scc_nodes[scc_index].on_stack = false;

                        node_to_scc[scc_index] = scc_id;
                        scc_data.push(scc_index);

                        if scc_index == index { break; }
                    }


                    let end = scc_data.klen();

                    scc_data[begin..end].reverse();
                    scc_ends.push(end);
                }
            }
        }

        assert!(scc_stack.len() == 0);
        assert!(visit_stack.len() == 0);

        let mut scc_in_degrees = KVec::from_value(scc_ends.len(), 0);

        for node_id in nodes.krange() {
            let node = nodes[node_id].as_ref().unwrap();
            let from_scc_id = node_to_scc[node_id];
            for &link in &node.outputs {
                let Some(link) = link
                else { continue; };
                let to_scc_id = node_to_scc[link];
                if to_scc_id == from_scc_id { continue }

                scc_in_degrees[to_scc_id] += 1;
            }
        }


        let mut queue = Vec::new();
        for i in scc_in_degrees.krange() {
            let deg = scc_in_degrees[i];
            if deg == 0 {
                queue.push(i);
            }
        }

        
        let edges = queue.clone();

        let mut worklist : Vec<NodeId> = Vec::with_capacity(nodes.len());
        while let Some(scc_index) = queue.pop() {
            let scc_begin = if scc_index.0 == 0 { SccId(0) }
                            else { scc_ends[SccId(scc_index.0 - 1)] };

            let scc_end = scc_ends[scc_index];

            let scc = &scc_data[scc_begin..scc_end];
            worklist.extend(scc);
            for &node in scc {
                let from_scc_id = node_to_scc[node];
                let node = &nodes[node];

                for &link in node.as_ref().unwrap().outputs.iter() {

                    let Some(link) = link
                    else { continue; };
                    let to_scc_id = node_to_scc[link];
                    if to_scc_id == from_scc_id { continue }

                    scc_in_degrees[to_scc_id] -= 1;
                    if scc_in_degrees[to_scc_id] == 0 {
                        queue.push(to_scc_id);
                    }
                }
            }
        }

        Belts {
            worklist,
            structure_to_node: struct_to_node,
            nodes,
            edges,
            scc_data,
            scc_nodes,
            scc_ends,
        }

    }

}


pub struct Node {
    pub outputs: [Option<NodeId>; 1],
    pub structure_id: StructureId,
}


#[derive(Debug, Clone)]
pub struct SccNode {
    pub index: u32,
    pub low_link: u32,
    pub on_stack: bool,
}


pub struct Belts {
    pub worklist: Vec<NodeId>,
    pub structure_to_node: HashMap<StructureId, NodeId>,
    pub nodes: KVec<NodeId, Option<Node>>,
    pub edges: Vec<SccId>,
    pub scc_data: KVec<SccId, NodeId>,
    pub scc_nodes: KVec<NodeId, SccNode>,
    pub scc_ends: KVec<SccId, SccId>,
}


impl Belts {
    pub fn node(&self, node: NodeId) -> &Node {
        self.nodes[node].as_ref().unwrap()
    }


    pub fn scc_graph(&self) -> String {
        let mut output = String::new();
        let _ = write!(output, "digraph {{");
        let _ = write!(output, "node [shape=box];");
        let _ = write!(output, "edge [color=gray];");


        let step = 360.0 / self.scc_ends.len() as f64;
        for i in self.scc_ends.krange() {
            let hue = step * i.usize() as f64;

            let hex = hsl_to_hex(hue, 0.6, 0.8);


            let _ = write!(output, "subgraph cluster_{} {{", i.usize());
            let _ = write!(output, "label = \"SCC #{} is_edge: {}\";", i.usize(), self.edges.contains(&i));
            let _ = write!(output, "style = filled;");
            let _ = write!(output, "fillcolor = \"{hex}\";");

            let scc_begin = if i == SccId::MIN { SccId::MIN }
                            else { self.scc_ends[unsafe { SccId::from_usize_unck(i.usize() - 1) }] };
            let scc_end = self.scc_ends[i];
            let scc_node_ids = &self.scc_data[scc_begin..scc_end];

            for &scc_node_id in scc_node_ids {
                let node = self.nodes[scc_node_id].as_ref().unwrap();
                let scc_node = &self.scc_nodes[scc_node_id];

                let _ = write!(output, "{} [label=\"node_id={} index={} lowest_link={}\"];", scc_node_id.usize(), scc_node_id.usize(), scc_node.index, scc_node.low_link);
                for link in &node.outputs {
                    if let Some(link) = link {
                        let _ = write!(output, "{} -> {};", scc_node_id.usize(), link.usize());
                    }
                }
            }

            let _ = write!(output, "}}");

        }
        let _ = write!(output, "}}");
        output

    }
}
