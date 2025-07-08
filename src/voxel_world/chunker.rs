use std::{collections::HashSet, num::NonZeroU32, sync::{mpsc::{Receiver, Sender}, Arc}, time::Instant};

use bytemuck::Zeroable;
use glam::{IVec3, UVec3};
use glfw::ffi::CENTER_CURSOR;
use rand::seq::IndexedRandom;
use save_format::byte::ByteReader;
use sti::key::Key;
use tracing::{error, info, trace, warn};
use wgpu::util::StagingBelt;

use crate::{constants::{CHUNK_SIZE, CHUNK_SIZE_I32, REGION_SIZE, REGION_SIZE_P3}, free_list::FreeKVec, octree::MeshOctree, renderer::{gpu_allocator::GPUAllocator, ssbo::SSBO, MeshIndex}};

use super::{chunk::{Chunk, ChunkData, Noise}, mesh::{ChunkDataRef, ChunkFaceMesh, ChunkMeshFramedata, ChunkMeshes, ChunkQuadInstance}, VoxelWorld, SURROUNDING_OFFSETS};

pub struct Chunker {
    regions: sti::hash::HashMap<RegionPos, Region>,

    chunk_load_queue: Vec<WorldChunkPos>,
    chunk_sender: Sender<ChunkMPSC>,
    chunk_reciever: Receiver<ChunkMPSC>,
    chunk_active_jobs: u32,


    mesh_load_queue: HashSet<WorldChunkPos>,
    mesh_active_jobs: HashSet<WorldChunkPos>,
    mesh_unload_queue: HashSet<WorldChunkPos>,
    mesh_sender: Sender<MeshMPSC>,
    mesh_reciever: Receiver<MeshMPSC>,

    noise: Arc<Noise>,
}

type ChunkMPSC = (WorldChunkPos, Chunk);
type MeshMPSC = (WorldChunkPos, [MeshIndex; 6], [Vec<ChunkQuadInstance>; 6], NonZeroU32);

#[derive(PartialEq, Eq, Clone, Copy, Debug, Hash)]
pub struct RegionPos(pub IVec3);
#[derive(PartialEq, Eq, Clone, Copy, Debug, Hash)]
pub struct ChunkPos(pub UVec3);
#[derive(PartialEq, Eq, Clone, Copy, Debug, Hash)]
pub struct WorldChunkPos(pub IVec3);


pub struct Region {
    chunks: Box<[ChunkEntry; REGION_SIZE_P3]>,
    meshes: Box<[MeshEntry; REGION_SIZE_P3]>,
    octree: MeshOctree,

}


pub struct MeshTaskData {
    version: NonZeroU32,
    offsets: [MeshIndex; 6],
    chunks: ChunkDataRef,
    pos: WorldChunkPos,
}


#[repr(u32)]
pub enum ChunkEntry {
    None = 0,

    Loading,

    Loaded(Chunk),
}


#[repr(u32)]
pub enum MeshEntry {
    None = 0,
    Loaded(ChunkMeshes),
}


pub enum GetChunk<'a> {
    Chunk(&'a mut Chunk),

    Loading,

    NotPresent,
}


impl Chunker {
    pub fn new() -> Self {
        let (cs, cr) = std::sync::mpsc::channel();
        let (ms, mr) = std::sync::mpsc::channel();

        Self {
            regions: sti::hash::HashMap::new(),

            chunk_load_queue: vec![],
            chunk_sender: cs,
            chunk_reciever: cr,
            chunk_active_jobs: 0,

            mesh_load_queue: HashSet::new(),
            mesh_unload_queue: HashSet::new(),
            mesh_sender: ms,
            mesh_reciever: mr,
            mesh_active_jobs: HashSet::new(),

            noise: Arc::new(Noise::new(69696969)),
        }
    }

    pub fn process_mesh_queue(
        &mut self,
        timeout: u32,
        framedata: &mut FreeKVec<MeshIndex, ChunkMeshFramedata>,
    ) {
        let timeout = timeout as u128;
        let start = Instant::now();

        let mut batch = vec![];

        let mut load_queue = core::mem::take(&mut self.mesh_load_queue);
        let mut iter = load_queue.iter();

        loop {
            if start.elapsed().as_millis() > timeout { break; }

            let Some(chunk_pos) = iter.next()
            else { break };

            let did_succeed = self.try_prepare_mesh_task(
                framedata,
                &mut batch,
                *chunk_pos
            );


            if !did_succeed { continue }

            if batch.len() == 32 {
                batch.iter().for_each(|x| { assert!(load_queue.remove(&x.pos)); });
                iter = load_queue.iter();
                self.spawn_mesh_task(batch);
                batch = vec![];
            }
        }

        batch.iter().for_each(|x| { load_queue.remove(&x.pos); });
        self.spawn_mesh_task(batch);

        self.mesh_load_queue = load_queue;
    }

    pub fn process_mesh_unload_queue(
        &mut self,
        timeout: u32,
        framedata: &mut FreeKVec<MeshIndex, ChunkMeshFramedata>,
        instance_allocator: &mut GPUAllocator<ChunkQuadInstance>,
    ) {
        let timeout = timeout as u128;
        let start = Instant::now();

        let mut unload_queue = core::mem::take(&mut self.mesh_unload_queue);
        let mut remove_list = vec![];
        let mut iter = unload_queue.iter();

        loop {
            if start.elapsed().as_millis() > timeout { break; }

            let Some(&chunk_pos) = iter.next()
            else { break };

            let region = self.get_region_or_insert(chunk_pos.region());

            let mesh = region.get_mesh_mut(chunk_pos.chunk());

            remove_list.push(chunk_pos);
            match mesh {
                MeshEntry::None => {
                    //warn!("tried to unload a mesh that was already unloaded");
                    continue;
                },


                MeshEntry::Loaded(chunk_meshes) => {
                    if let Some(meshes) = chunk_meshes.meshes.take() {
                        let prev_meshes = region.octree.get_mut(meshes);

                        for mesh in prev_meshes.iter_mut() {
                            let Some(mesh) = mesh.take()
                            else { continue };

                            framedata.remove(mesh.chunk_mesh_data_index);
                            instance_allocator.free(mesh.vertex);
                        }

                    }


                },
            }
        }
        drop(iter);

        remove_list.iter().for_each(|x| { unload_queue.remove(x); });
        self.mesh_unload_queue = unload_queue;
    }


    pub fn process_chunk_queue(&mut self, timeout: u32) {
        let timeout = timeout as u128;
        let start = Instant::now();

        loop {
            if start.elapsed().as_millis() > timeout { break; }

            let Some(chunk_pos) = self.chunk_load_queue.pop()
            else { break };

            let noise = self.noise.clone();
            let sender = self.chunk_sender.clone();
            self.chunk_active_jobs += 1;

            rayon::spawn(move || {
                let result = generate_chunk(chunk_pos, &noise);

                if let Err(e) = sender.send((chunk_pos, result)) {
                    error!("chunk-generation: {e}");
                }
            });
        }
    }


    pub fn process_chunk_jobs(&mut self, timeout: u32) {
        let start = Instant::now();

        loop {
            if start.elapsed().as_millis() as u32 > timeout { break; }

            let Ok((chunk_pos, chunk)) = self.chunk_reciever.try_recv()
            else { break; };

            self.chunk_active_jobs -= 1;

            let region = self.get_region_or_insert(chunk_pos.region());
            let entry = region.get_mut(chunk_pos.chunk());

            match entry {
                ChunkEntry::None => {
                    warn!("chunk was unloaded");
                    continue
                },


                ChunkEntry::Loading => {
                    *entry = ChunkEntry::Loaded(chunk);
                },


                ChunkEntry::Loaded(_) => {
                    warn!("chunk at '{}' was already loaded", chunk_pos.0);
                },

            }
        }
    }


    pub fn process_mesh_jobs(
        &mut self,
        timeout: u32,

        device: &wgpu::Device,
        encoder: &mut wgpu::CommandEncoder,
        belt: &mut StagingBelt,
        instance_allocator: &mut GPUAllocator<ChunkQuadInstance>,
        free_list: &mut FreeKVec<MeshIndex, ChunkMeshFramedata>,
        gpu_mesh_data: &mut SSBO<ChunkMeshFramedata>,
    ) {

        let start = Instant::now();
        loop {
            if start.elapsed().as_millis() as u32 > timeout { break; }

            let Ok((chunk_pos, offsets, result, version)) = self.mesh_reciever.try_recv()
            else { break; };

            assert!(self.mesh_active_jobs.remove(&chunk_pos));

            let region = self.get_region_or_insert(chunk_pos.region());

            let entry = &mut region.meshes[chunk_pos.chunk().to_region_index()];

            let mut data = [const { None }; 6];
            for i in 0..6 {
                if result[i].is_empty() { continue };

                let mesh = ChunkFaceMesh::new(
                    belt, encoder, device, instance_allocator,
                    &result[i], offsets[i]
                );


                let index = mesh.chunk_mesh_data_index.usize();
                if index >= gpu_mesh_data.len() {
                    warn!("resizing ssbo");
                    gpu_mesh_data.resize(device, encoder, (gpu_mesh_data.len() * 2).max(index+1));
                }

                gpu_mesh_data.write(
                    belt,
                    encoder,
                    device,
                    index,
                    &[ChunkMeshFramedata { offset: chunk_pos.0, normal: i as u32 }]
                );



                data[i] = Some(mesh);
            }


            let data = data;
            let is_data_some = data.iter().any(|x| x.is_some());


            match entry {
                MeshEntry::None => {
                    let meshes = 
                        if !is_data_some { None }
                        else { Some(region.octree.insert(chunk_pos.chunk(), data)) };

                    let value = ChunkMeshes { meshes, version };
                    *entry = MeshEntry::Loaded(value);
                },


                MeshEntry::Loaded(chunk_meshes) => {
                    if chunk_meshes.version > version {
                        warn!("mesh discarded because it was outdated");
                        continue;
                    }

                    chunk_meshes.version = version;
                    if let Some(meshes) = chunk_meshes.meshes {
                        let prev_meshes = region.octree.get_mut(meshes);

                        for mesh in prev_meshes.iter_mut() {
                            let Some(mesh) = mesh.take()
                            else { continue };

                            free_list.remove(mesh.chunk_mesh_data_index);
                            instance_allocator.free(mesh.vertex);
                        }


                        if is_data_some {
                            *prev_meshes = data;
                        } else {
                            chunk_meshes.meshes = None;
                        }
                    } else {
                        let meshes = 
                            if !is_data_some { None }
                            else { Some(region.octree.insert(chunk_pos.chunk(), data)) };
                        chunk_meshes.meshes = meshes;
                    }
                },
            }


        }


    }


    fn spawn_mesh_task(&self, batch: Vec<MeshTaskData>) {
        if batch.is_empty() { return }

        let sender = self.mesh_sender.clone();
        rayon::spawn(move || {
            for item in batch {
                let mesh = VoxelWorld::greedy_mesh(item.offsets, item.chunks);
                if let Err(e) = sender.send((item.pos, item.offsets, mesh, item.version)) {
                    error!("mesh-task: {e}");
                    break;
                }
            }
        });
    }


    fn try_prepare_mesh_task(
        &mut self,
        free_list: &mut FreeKVec<MeshIndex, ChunkMeshFramedata>,
        task_queue: &mut Vec<MeshTaskData>,
        pos: WorldChunkPos,
    ) -> bool {
        if self.mesh_active_jobs.contains(&pos) { return true };

        let base = pos.0;
        for x in -1..=1 {
            for y in -1..=1 {
                for z in -1..=1 {
                    let offset = IVec3::new(x, y, z);
                    let Some(_) = self.get_chunk_or_queue(WorldChunkPos(base+offset))
                    else { return false; };
                }
            }
        }


        let region = self.get_region_or_insert(pos.region());
        let (chunk, mesh) = region.get_mut_chunk_and_mesh(pos.chunk());


        let chunk = match (chunk, mesh) {
              (ChunkEntry::Loading, _)
            | (ChunkEntry::None, _) => {
                self.get_chunk_or_queue(pos);
                return false;
            },


            (ChunkEntry::Loaded(c), MeshEntry::None) => c,


            (ChunkEntry::Loaded(chunk), MeshEntry::Loaded(chunk_meshes)) => {
                if chunk.version == chunk_meshes.version {
                    trace!("failed to spawn a mesh job for chunk at '{}' because the mesh is up-to-date", pos.0);
                    return true;
                }


                chunk_meshes.version = chunk.version;
                chunk
            },
        };


        if chunk.data.is_none() {
            trace!("failed to spawn a mesh job for chunk at '{}' because it's empty", pos.0);
            return true;
        }


        let version = chunk.version;
        self.mesh_active_jobs.insert(pos);

        let offsets = [
            free_list.push(ChunkMeshFramedata::zeroed()),
            free_list.push(ChunkMeshFramedata::zeroed()),
            free_list.push(ChunkMeshFramedata::zeroed()),
            free_list.push(ChunkMeshFramedata::zeroed()),
            free_list.push(ChunkMeshFramedata::zeroed()),
            free_list.push(ChunkMeshFramedata::zeroed()),
        ];

        let mut chunks : [Option<Arc<ChunkData>>; 27] = [const { None }; 27];
        let base = pos.0 - IVec3::ONE;
        for x in 0..3 {
            for y in 0..3 {
                for z in 0..3 {
                    let offset = IVec3::new(x, y, z);
                    let chunk = self.get_chunk(WorldChunkPos(base+offset)).unwrap();
                    if chunk.data.is_none() { continue };

                    let chunk_idx =
                          9*offset.x
                        + 3*offset.y
                        + 1*offset.z;
                    chunks[chunk_idx as usize] = chunk.data.clone();
                }
            }
        }


        task_queue.push(MeshTaskData {
            version,
            offsets,
            chunks: ChunkDataRef::new(chunks),
            pos,
        });

        true
    }




    pub fn unload_chunk(&mut self, pos: WorldChunkPos) {
        let entry = self.get_chunk_entry(pos);

        match entry {
            ChunkEntry::Loaded(_) => {
                self.mesh_unload_queue.insert(pos);
                self.unload_voxel_data_of_chunk(pos);
            },


            ChunkEntry::Loading { .. } => 
                error!("tried to unload a chunk that was loading"),


            ChunkEntry::None => 
                warn!("tried to unload a chunk that was already unloaded"),
        }
    }


    pub fn unload_voxel_data_of_chunk(&mut self, pos: WorldChunkPos) {
        let region = self.get_region_or_insert(pos.region());
        let entry = region.get_mut(pos.chunk());

        match entry {
            ChunkEntry::Loaded(_) => {
                *entry = ChunkEntry::None;
            },


            ChunkEntry::Loading { .. } =>
                error!("tried to unload a chunk's voxel data while the chunk was loading"),


            ChunkEntry::None =>
                warn!("tried to unload voxel data from a chunk that was already unloaded"),
        }
    }


    pub fn get_region_or_insert(&mut self, pos: RegionPos) -> &mut Region {
        let hash = self.regions.hash(&pos);
        let (present, slot) = self.regions.lookup_for_insert(&pos, hash);

        // rust is great software
        // without this it inlines the Region::new call below which
        // has a giant stack frame, thus making release mode slower
        // than debug mode
        //
        // thank you rust, very cool.
        sti::hint::unlikely(!present);

        if !present {
            self.regions.insert_at(slot, hash, pos, Region::new());
        }

        self.regions.slot_mut(slot).1
    }


    pub fn get_chunk(&mut self, pos: WorldChunkPos) -> Option<&Chunk> {
        match self.get_chunk_entry(pos) {
            ChunkEntry::Loaded(chunk) => Some(chunk),
            _ => None,
        }
    }


    pub fn get_mut_chunk(&mut self, pos: WorldChunkPos) -> Option<&mut Chunk> {
        for offset in SURROUNDING_OFFSETS {
            let pos = WorldChunkPos(pos.0 + offset);
            let ChunkEntry::Loaded(chunk) = self.get_chunk_entry(pos)
            else { continue };

            chunk.version = chunk.version.checked_add(1).unwrap();
        }


        let chunk = match self.get_chunk_entry(pos) {
            ChunkEntry::Loaded(chunk) => chunk,
            _ => return None,
        };

        chunk.version = chunk.version.checked_add(1).unwrap();
        chunk.is_dirty = true;

        Some(chunk)

    }


    ///
    /// Tries to get a chunk.
    ///
    /// If the chunk isn't loaded or loading then queue that chunk to be loaded
    ///
    pub fn get_chunk_or_queue<'a>(&'a mut self, pos: WorldChunkPos) -> Option<&'a mut Chunk> {
        let chunk = self.get_chunk_entry(pos);
        let chunk = unsafe { sti::erase!(&mut ChunkEntry, chunk) };

        match chunk {
            ChunkEntry::Loaded(chunk) => {
                return Some(chunk)
            },


            ChunkEntry::None => {
                *chunk = ChunkEntry::Loading;

                self.chunk_load_queue.push(pos);
                return None;
            }


            ChunkEntry::Loading => {
                return None;
            },
        }
    }


    pub fn get_chunk_or_generate(&mut self, pos: WorldChunkPos) -> &Chunk {
        let chunk = self.get_chunk_entry(pos);
        let chunk = unsafe { sti::erase!(&mut ChunkEntry, chunk) };

        match chunk {
            ChunkEntry::Loaded(chunk) => {
                return &*chunk
            },


              ChunkEntry::None 
            | ChunkEntry::Loading => {
                let result = generate_chunk(pos, &self.noise);
                *chunk = ChunkEntry::Loaded(result);

                self.get_chunk_or_generate(pos)
            }
        }
    }



    pub fn get_mesh_or_queue(&mut self, pos: WorldChunkPos) -> Option<&ChunkMeshes> {
        let region = self.get_region_or_insert(pos.region());
        let region = unsafe { sti::erase!(&mut Region, region) };

        let (entry, mesh_entry) = region.get_mut_chunk_and_mesh(pos.chunk());

        match (entry, mesh_entry) {
            (ChunkEntry::Loaded(chunk), MeshEntry::None) => {
                if chunk.data.is_some() && !self.mesh_active_jobs.contains(&pos) {
                    self.mesh_load_queue.insert(pos);
                }
                None
            },

            (ChunkEntry::Loaded(chunk), MeshEntry::Loaded(chunk_meshes)) => {
                if chunk.version != chunk_meshes.version {
                    println!("queueing");
                    self.mesh_load_queue.insert(pos);
                }

                Some(chunk_meshes)
            },



            (_, MeshEntry::Loaded(chunk_meshes)) => {
                Some(chunk_meshes)
            },


            (_, MeshEntry::None) => None,
        }
    }


    pub fn get_chunk_entry(&mut self, pos: WorldChunkPos) -> &mut ChunkEntry {
        let region = self.get_region_or_insert(pos.region());

        let chunk = region.get_mut(pos.chunk());
        chunk
    }


    pub fn get_mesh_entry(&mut self, pos: WorldChunkPos) -> &mut MeshEntry {
        let region = self.get_region_or_insert(pos.region());

        let chunk = region.get_mesh_mut(pos.chunk());
        chunk
    }


    pub fn regions(&self) -> impl Iterator<Item=(RegionPos, &Region)> {
        self.regions.iter().map(|x| (*x.0, x.1))
    }


    pub fn mesh_load_queue_len(&self) -> usize { self.mesh_load_queue.len() }
    pub fn mesh_active_jobs_len(&self) -> usize { self.mesh_active_jobs.len() }
    pub fn mesh_unload_queue_len(&self) -> usize { self.mesh_unload_queue.len() }
    pub fn chunk_active_jobs_len(&self) -> usize { self.chunk_active_jobs as usize }
    pub fn chunk_load_queue_len(&self) -> usize { self.chunk_load_queue.len() }


    pub fn is_chunk_meshing(&self, chunk: WorldChunkPos) -> bool {
        self.mesh_active_jobs.contains(&chunk)
    }


    pub fn is_queued_for_meshing(&self, chunk: WorldChunkPos) -> bool {
        self.mesh_load_queue.contains(&chunk)
    }


    pub fn is_queued_for_unloading(&self, chunk: WorldChunkPos) -> bool {
        self.mesh_unload_queue.contains(&chunk)
    }


    pub fn iter_chunks(&self) -> impl Iterator<Item=(WorldChunkPos, &ChunkEntry, &MeshEntry)> {
        self.regions()
            .flat_map(|x| 
                      x.1
                      .iter_chunks()
                      .map(move |c| (WorldChunkPos::new(x.0, c.0), c.1, c.2)
           ))
    }
}


impl Region {
    pub fn new() -> Self {
        Self {
            octree: MeshOctree::new(),
            chunks: Box::new([const { ChunkEntry::None }; _]),
            meshes: Box::new([const { MeshEntry::None }; _]),
        }
    }


    pub fn get(&self, pos: ChunkPos) -> &ChunkEntry {
        let index = pos.to_region_index();
        &self.chunks[index]
    }


    pub fn get_mut(&mut self, pos: ChunkPos) -> &mut ChunkEntry {
        let index = pos.to_region_index();
        &mut self.chunks[index]
    }


    pub fn get_mut_chunk_and_mesh(&mut self, pos: ChunkPos) -> (&mut ChunkEntry, &mut MeshEntry) {
        let index = pos.to_region_index();
        (&mut self.chunks[index], &mut self.meshes[index])
    }


    pub fn get_mesh(&self, pos: ChunkPos) -> &MeshEntry {
        let index = pos.to_region_index();
        &self.meshes[index]
    }


    pub fn get_mesh_mut(&mut self, pos: ChunkPos) -> &mut MeshEntry {
        let index = pos.to_region_index();
        &mut self.meshes[index]
    }


    pub fn octree(&self) -> &MeshOctree {
        &self.octree
    }


    pub fn chunks(&self) -> &[ChunkEntry] {
        &*self.chunks
    }


    pub fn iter_chunks(&self) -> impl Iterator<Item=(ChunkPos, &ChunkEntry, &MeshEntry)> {
        self.chunks
            .iter()
            .enumerate()
            .map(|(i, c)| {
                let x = i % REGION_SIZE;
                let y = (i / REGION_SIZE) % REGION_SIZE;
                let z = i / (REGION_SIZE*REGION_SIZE);
                let pos = UVec3::new(x as u32, y as u32, z as u32);

                (ChunkPos(pos), c, &self.meshes[i])
            })
    }

}


impl WorldChunkPos {
    pub fn new(region: RegionPos, chunk: ChunkPos) -> Self {
        let pos = region.0 * REGION_SIZE as i32;
        let pos = pos + chunk.0.as_ivec3();
        Self(pos)
    }


    #[inline(always)]
    pub fn region(self) -> RegionPos {
        RegionPos(self.0.div_euclid(IVec3::splat(REGION_SIZE as i32)))
    }


    pub fn chunk(self) -> ChunkPos {
        let pos = self.0.rem_euclid(IVec3::splat(REGION_SIZE as i32));
        debug_assert!(pos.x >= 0 && pos.y >= 0 && pos.z >= 0);

        ChunkPos(pos.as_uvec3())
    }
}


impl ChunkPos {
    pub fn to_region_index(self) -> usize {
        let pos = self.0.as_usizevec3();
        let index =   pos.z * REGION_SIZE * REGION_SIZE
                    + pos.y * REGION_SIZE
                    + pos.x;
        index
    }
}


fn generate_chunk(pos: WorldChunkPos, noise: &Noise) -> Chunk {
    let pos = pos.0;
    let path = format!("saves/chunks/{pos}.chunk");
    let chunk = match std::fs::read(&path) {
        Ok(ref v) if let Some(mut byte_reader) = ByteReader::new(&v) => {
            let mut chunk = Chunk::empty_chunk();
            let data = ChunkData::from_bytes(byte_reader.read().unwrap());
            if !data.is_empty() {
                chunk.data = Some(Arc::new(data));
            }

            chunk.is_dirty = false;
            chunk
        },


        _ => {
            Chunk::generate(pos, noise)
        }
    };

    chunk
}

