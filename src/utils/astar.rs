//! Contains classic A* (A-star) path finding algorithms.
//!
//! A* is one of fastest graph search algorithms, it is used to construct shortest
//! possible path from vertex to vertex. In vast majority of games it is used in pair
//! with navigation meshes (navmesh). Check navmesh module docs for more info.

#![warn(missing_docs)]

use crate::core::{
    algebra::Vector3,
    math::{self, PositionProvider},
    visitor::prelude::*,
};

use std::{
    cmp::Ordering,
    collections::BinaryHeap,
    fmt::{Display, Formatter},
    ops::{Deref, DerefMut},
};

/// State a of path vertex.
#[derive(Copy, Clone, PartialEq, Eq, Debug)]
pub enum PathVertexState {
    /// A vertex wasn't visited and yet to be processed.
    NonVisited,
    /// A vertex is inside an open set (to be visited).
    Open,
    /// A vertex is inside an closed set (was visited).
    Closed,
}

/// Graph vertex that contains position in world and list of indices of neighbour
/// vertices.
#[derive(Clone, Debug, Visit, PartialEq)]
pub struct VertexData {
    /// Position in the world coordinates
    pub position: Vector3<f32>,
    /// A set of indices of neighbour vertices.
    pub neighbours: Vec<u32>,
    /// Current state of the vertex.
    #[visit(skip)]
    pub state: PathVertexState,
    /// Penalty can be interpreted as measure, how harder is to travel to this vertex.
    #[visit(skip)]
    pub g_penalty: f32,
    /// Path cost of the vertex.
    #[visit(skip)]
    pub g_score: f32,
    /// A numeric metric, of how effective would be moving to neighbour in finding the optimal path.
    #[visit(skip)]
    pub f_score: f32,
    /// An index of a vertex that is previous (relative to this) in the path.
    #[visit(skip)]
    pub parent: Option<usize>,
}

impl Default for VertexData {
    fn default() -> Self {
        Self {
            position: Default::default(),
            parent: None,
            g_penalty: 1f32,
            g_score: f32::MAX,
            f_score: f32::MAX,
            state: PathVertexState::NonVisited,
            neighbours: Default::default(),
        }
    }
}

impl VertexData {
    /// Creates new vertex at given position.
    pub fn new(position: Vector3<f32>) -> Self {
        Self {
            position,
            parent: None,
            g_penalty: 1f32,
            g_score: f32::MAX,
            f_score: f32::MAX,
            state: PathVertexState::NonVisited,
            neighbours: Default::default(),
        }
    }

    fn clear(&mut self) {
        self.g_penalty = 1f32;
        self.g_score = f32::MAX;
        self.f_score = f32::MAX;
        self.state = PathVertexState::NonVisited;
        self.parent = None;
    }
}

/// A trait, that describes and arbitrary vertex that could be used in a graph. It allows you to
/// use your structure to store additional info in the graph.
pub trait VertexDataProvider: Deref<Target = VertexData> + DerefMut + PositionProvider {}

/// A default graph vertex with no additional data.
#[derive(Default, PartialEq, Debug)]
pub struct GraphVertex {
    /// Data of the vertex.
    pub data: VertexData,
}

impl GraphVertex {
    /// Creates a new graph vertex.
    pub fn new(position: Vector3<f32>) -> Self {
        Self {
            data: VertexData::new(position),
        }
    }
}

impl Deref for GraphVertex {
    type Target = VertexData;

    fn deref(&self) -> &Self::Target {
        &self.data
    }
}

impl DerefMut for GraphVertex {
    fn deref_mut(&mut self) -> &mut Self::Target {
        &mut self.data
    }
}

impl PositionProvider for GraphVertex {
    fn position(&self) -> Vector3<f32> {
        self.data.position
    }
}

impl Visit for GraphVertex {
    fn visit(&mut self, name: &str, visitor: &mut Visitor) -> VisitResult {
        self.data.visit(name, visitor)
    }
}

impl VertexDataProvider for GraphVertex {}

/// See module docs.
#[derive(Clone, Debug, Visit, PartialEq)]
pub struct Graph<T>
where
    T: VertexDataProvider,
{
    /// Vertices of the graph.
    pub vertices: Vec<T>,
}

/// Shows path status.
#[derive(Copy, Clone, PartialEq, Eq, Debug)]
pub enum PathKind {
    /// There is direct path from begin to end.
    Full,
    /// No direct path, only partial to closest reachable vertex to destination. Can
    /// happen if there are isolated "islands" of graph vertices with no links between
    /// them and you trying to find path from one "island" to other.
    Partial,
}

fn heuristic(a: Vector3<f32>, b: Vector3<f32>) -> f32 {
    (a - b).norm_squared()
}

impl<T: VertexDataProvider> Default for Graph<T> {
    fn default() -> Self {
        Self::new()
    }
}

impl PositionProvider for VertexData {
    fn position(&self) -> Vector3<f32> {
        self.position
    }
}

/// Path search can be interrupted by errors, this enum stores all possible
/// kinds of errors.
#[derive(Clone, Debug)]
pub enum PathError {
    /// Out-of-bounds vertex index has found, it can be either index of begin/end
    /// points, or some index of neighbour vertices in list of neighbours in vertex.
    InvalidIndex(usize),

    /// There is a vertex that has itself as neighbour.
    CyclicReferenceFound(usize),

    /// Graph was empty.
    Empty,
}

impl Display for PathError {
    fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
        match self {
            PathError::InvalidIndex(v) => {
                write!(f, "Invalid vertex index {v}.")
            }
            PathError::CyclicReferenceFound(v) => {
                write!(f, "Cyclical reference was found {v}.")
            }
            PathError::Empty => {
                write!(f, "Graph was empty")
            }
        }
    }
}

#[derive(Clone)]
/// A partailly complete path containing the indexes to its vertices and its A* scores
pub struct PartialPath {
    vertices: Vec<usize>,
    g_score: f32,
    f_score: f32,
}

impl Default for PartialPath {
    fn default() -> Self {
        Self {
            vertices: Vec::new(),
            g_score: f32::MAX,
            f_score: f32::MAX,
        }
    }
}

impl Ord for PartialPath {
    /// only compairs f-value and heuristic
    fn cmp(&self, other: &Self) -> Ordering {
        (self.f_score.total_cmp(&other.f_score))
            .then((self.f_score - self.g_score).total_cmp(&(other.f_score - other.g_score)))
            .reverse()
    }
}

impl PartialOrd for PartialPath {
    /// only compairs f-value and heuristic
    fn partial_cmp(&self, other: &Self) -> Option<Ordering> {
        Some(self.cmp(other))
    }
}

impl PartialEq for PartialPath {
    /// only determaines if values are equal not equal composition
    fn eq(&self, other: &Self) -> bool {
        self.f_score == other.f_score && self.g_score == other.g_score
    }
}

impl Eq for PartialPath {}

impl PartialPath {
    /// creates a new partial path from the starting vertex index
    pub fn new(start: usize) -> Self {
        Self {
            vertices: vec![start],
            g_score: 0f32,
            f_score: f32::MAX,
            //f_score: f32::MAX / 10f32,
        }
    }

    /// returns a clone with the new vertex added to the end and updates scores to given new scores
    pub fn clone_and_add(
        &self,
        new_vertex: usize,
        new_g_score: f32,
        new_f_score: f32,
    ) -> PartialPath {
        let mut clone = self.clone();
        clone.vertices.push(new_vertex);
        clone.g_score = new_g_score;
        clone.f_score = new_f_score;

        return clone;
    }
}

impl<T: VertexDataProvider> Graph<T> {
    /// Creates new empty path finder.
    pub fn new() -> Self {
        Self {
            vertices: Default::default(),
        }
    }

    /// Sets active set of vertices. Links between vertices must contain
    /// valid indices (which are not out-of-bounds), otherwise path from/to
    /// such vertices won't be built.
    pub fn set_vertices(&mut self, vertices: Vec<T>) {
        self.vertices = vertices;
    }

    /// Tries to find a vertex closest to given point.
    ///
    /// # Notes
    ///
    /// O(n) complexity.
    pub fn get_closest_vertex_to(&self, point: Vector3<f32>) -> Option<usize> {
        math::get_closest_point(&self.vertices, point)
    }

    /// Creates bidirectional link between two vertices. Bidirectional means
    /// that point `a` can be reached from point `b` as well as point `b`
    /// can be reached from point `a`.
    pub fn link_bidirect(&mut self, a: usize, b: usize) {
        self.link_unidirect(a, b);
        self.link_unidirect(b, a);
    }

    /// Creates unidirectional link between vertex `a` and vertex `b`. Unidirectional
    /// means that there is no direct link between `b` to `a`, only from `a` to `b`.
    pub fn link_unidirect(&mut self, a: usize, b: usize) {
        if let Some(vertex_a) = self.vertices.get_mut(a) {
            if vertex_a.neighbours.iter().all(|n| *n != b as u32) {
                vertex_a.neighbours.push(b as u32);
            }
        }
    }

    /// Returns shared reference to a path vertex at the given index.
    pub fn vertex(&self, index: usize) -> Option<&T> {
        self.vertices.get(index)
    }

    /// Returns mutable reference to a path vertex at the given index.
    pub fn vertex_mut(&mut self, index: usize) -> Option<&mut T> {
        self.vertices.get_mut(index)
    }

    /// Returns reference to the array of vertices.
    pub fn vertices(&self) -> &[T] {
        &self.vertices
    }

    /// Returns mutable reference to the array of vertices.
    pub fn vertices_mut(&mut self) -> &mut [T] {
        &mut self.vertices
    }

    /// Adds a new vertex to the path finder.
    pub fn add_vertex(&mut self, vertex: T) -> u32 {
        let index = self.vertices.len();
        // Since we're adding the vertex to the end of the array, we don't need to
        // shift indices of neighbours (like `insert_vertex`)
        self.vertices.push(vertex);
        index as u32
    }

    /// Removes last vertex from the graph. Automatically cleans "dangling" references to the deleted vertex
    /// from every other vertex in the graph and shifts indices of neighbour vertices, to preserve graph
    /// structure.
    pub fn pop_vertex(&mut self) -> Option<T> {
        if self.vertices.is_empty() {
            None
        } else {
            Some(self.remove_vertex(self.vertices.len() - 1))
        }
    }

    /// Removes a vertex at the given index from the graph. Automatically cleans "dangling" references to the
    /// deleted vertex from every other vertex in the graph and shifts indices of neighbour vertices, to
    /// preserve graph structure.
    pub fn remove_vertex(&mut self, index: usize) -> T {
        for other_vertex in self.vertices.iter_mut() {
            // Remove "references" to the vertex, that will be deleted.
            if let Some(position) = other_vertex
                .neighbours
                .iter()
                .position(|n| *n == index as u32)
            {
                other_vertex.neighbours.remove(position);
            }

            // Shift neighbour indices to preserve vertex indexation.
            for neighbour_index in other_vertex.neighbours.iter_mut() {
                if *neighbour_index > index as u32 {
                    *neighbour_index -= 1;
                }
            }
        }

        self.vertices.remove(index)
    }

    /// Inserts the vertex at the given index. Automatically shifts neighbour indices of every other vertex
    /// in the graph to preserve graph structure.
    pub fn insert_vertex(&mut self, index: u32, vertex: T) {
        self.vertices.insert(index as usize, vertex);

        // Shift neighbour indices to preserve vertex indexation.
        for other_vertex in self.vertices.iter_mut() {
            for neighbour_index in other_vertex.neighbours.iter_mut() {
                if *neighbour_index >= index {
                    *neighbour_index += 1;
                }
            }
        }
    }

    /// Tries to build path from begin point to end point. Returns path kind:
    ///
    /// - Full: there are direct path from begin to end.
    /// - Partial: there are not direct path from begin to end, but it is closest.
    /// - Empty: no path available - in most cases indicates some error in input params.
    ///
    /// # Notes
    ///
    /// This is more or less a naive implementation, it most certainly will be slower than specialized solutions.
    pub fn build_old(
        &mut self,
        from: usize,
        to: usize,
        path: &mut Vec<Vector3<f32>>,
    ) -> Result<PathKind, PathError> {
        path.clear();
        self.build_and_convert(from, to, |_, v| path.push(v.position))
    }

    /// Tries to build path from begin point to end point. Returns path kind:
    ///
    /// - Full: there are direct path from begin to end.
    /// - Partial: there are not direct path from begin to end, but it is closest.
    /// - Empty: no path available - in most cases indicates some error in input params.
    ///
    /// # Notes
    ///
    /// This is more or less a naive implementation, it most certainly will be slower than specialized solutions.
    pub fn build_and_convert<F>(
        &mut self,
        from: usize,
        to: usize,
        func: F,
    ) -> Result<PathKind, PathError>
    where
        F: FnMut(usize, &T),
    {
        if self.vertices.is_empty() {
            return Ok(PathKind::Partial);
        }

        for vertex in self.vertices.iter_mut() {
            vertex.clear();
        }

        let end_pos = self
            .vertices
            .get(to)
            .ok_or(PathError::InvalidIndex(to))?
            .position;

        // Put start vertex in open set.
        let start = self
            .vertices
            .get_mut(from)
            .ok_or(PathError::InvalidIndex(from))?;
        start.state = PathVertexState::Open;
        start.g_score = 0.0;
        start.f_score = heuristic(start.position, end_pos);

        let mut open_set_size = 1;
        while open_set_size > 0 {
            let mut current_index = 0;
            let mut lowest_f_score = f32::MAX;
            for (i, vertex) in self.vertices.iter().enumerate() {
                if vertex.state == PathVertexState::Open && vertex.f_score < lowest_f_score {
                    current_index = i;
                    lowest_f_score = vertex.f_score;
                }
            }

            if current_index == to {
                self.reconstruct_path(current_index, func);
                return Ok(PathKind::Full);
            }

            open_set_size -= 1;

            // Take second mutable reference to vertices array, we'll enforce borrowing rules
            // at runtime. It will *never* give two mutable references to same path vertex.
            let unsafe_vertices: &mut Vec<T> = unsafe { &mut *(&mut self.vertices as *mut _) };

            let current_vertex = self
                .vertices
                .get_mut(current_index)
                .ok_or(PathError::InvalidIndex(current_index))?;

            current_vertex.state = PathVertexState::Closed;

            for neighbour_index in current_vertex.neighbours.iter() {
                // Make sure that borrowing rules are not violated.
                if *neighbour_index as usize == current_index {
                    return Err(PathError::CyclicReferenceFound(current_index));
                }

                // Safely get mutable reference to neighbour
                let neighbour = unsafe_vertices
                    .get_mut(*neighbour_index as usize)
                    .ok_or(PathError::InvalidIndex(*neighbour_index as usize))?;

                let g_score = current_vertex.g_score
                    + ((current_vertex.position - neighbour.position).norm_squared()
                        * neighbour.g_penalty);
                if g_score < neighbour.g_score {
                    neighbour.parent = Some(current_index);
                    neighbour.g_score = g_score;
                    neighbour.f_score = g_score + heuristic(neighbour.position, end_pos);

                    if neighbour.state != PathVertexState::Open {
                        neighbour.state = PathVertexState::Open;
                        open_set_size += 1;
                    }
                }
            }
        }

        // No direct path found, then there is probably partial path exists.
        // Look for vertex with least f_score and use it as starting point to
        // reconstruct partial path.
        let mut closest_index = 0;
        for (i, vertex) in self.vertices.iter().enumerate() {
            let closest_vertex = self
                .vertices
                .get(closest_index)
                .ok_or(PathError::InvalidIndex(closest_index))?;
            if vertex.f_score < closest_vertex.f_score {
                closest_index = i;
            }
        }

        self.reconstruct_path(closest_index, func);

        Ok(PathKind::Partial)
    }

    /// Tries to build path from begin point to end point. Returns path kind:
    ///
    /// - Full: there are direct path from begin to end.
    /// - Partial: there are not direct path from begin to end, but it is closest.
    /// - Empty: no path available - in most cases indicates some error in input params.
    ///
    /// # Notes
    ///
    /// This is more or less a naive implementation, it most certainly will be slower than specialized solutions.
    pub fn build(
        &self,
        from: usize,
        to: usize,
        path: &mut Vec<Vector3<f32>>,
    ) -> Result<PathKind, PathError> {
        if self.vertices.is_empty() {
            return Ok(PathKind::Partial);
        }

        path.clear();

        let mut searched_vertices = vec![false; self.vertices.len()];

        let end_pos = self
            .vertices
            .get(to)
            .ok_or(PathError::InvalidIndex(to))?
            .position;

        // creates heap for searching
        let mut search_heap: BinaryHeap<PartialPath> = BinaryHeap::new();

        // creates first partial path and adds it to heap
        search_heap.push(PartialPath::new(from));

        // stores best path found
        let mut best_path = PartialPath::default();

        // search loop
        // TODO: don't hard code max search iterations
        for _ in 0..1000 {
            // breakes loop if heap is empty
            if search_heap.is_empty() {
                break;
            }

            // pops best partial path off the heap to use for this iteration
            let current_path = search_heap.pop().unwrap();

            let current_index = *current_path.vertices.last().unwrap();

            // updates best path
            if current_path > best_path {
                best_path = current_path.clone();

                // breaks if end is found
                if current_index == to {
                    break;
                }
            }

            let current_vertex = self
                .vertices
                .get(current_index)
                .ok_or(PathError::InvalidIndex(current_index))?;

            // evaluates path scores one level deeper and adds the paths to the heap
            for i in current_vertex.neighbours.iter() {
                let neighbour_index = *i as usize;

                //avoids going in circles
                if searched_vertices[neighbour_index] {
                    continue;
                }

                let neighbour = self
                    .vertices
                    .get(neighbour_index)
                    .ok_or(PathError::InvalidIndex(neighbour_index))?;

                let neighbour_g_score = current_path.g_score
                    + ((current_vertex.position - neighbour.position).norm_squared()
                        * neighbour.g_penalty);

                let neighbour_f_score = neighbour_g_score + heuristic(neighbour.position, end_pos);

                search_heap.push(current_path.clone_and_add(
                    neighbour_index,
                    neighbour_g_score,
                    neighbour_f_score,
                ));
            }

            //marks vertex as searched
            searched_vertices[current_index] = true;
        }

        // converts from indicies to positions
        for index in best_path.vertices.iter() {
            let vertex = self
                .vertices
                .get(*index)
                .ok_or(PathError::InvalidIndex(*index))?;

            path.push(vertex.position);
        }
        path.reverse();

        if path.is_empty() {
            Err(PathError::Empty)
        } else if *path.first().unwrap() == end_pos {
            Ok(PathKind::Full)
        } else {
            Ok(PathKind::Partial)
        }
    }

    fn reconstruct_path<F>(&self, mut current: usize, mut func: F)
    where
        F: FnMut(usize, &T),
    {
        while let Some(vertex) = self.vertices.get(current) {
            func(current, vertex);
            if let Some(parent) = vertex.parent {
                current = parent;
            } else {
                break;
            }
        }
    }
}

#[cfg(test)]
mod test {
    use crate::rand::Rng;
    use crate::{
        core::{algebra::Vector3, rand},
        utils::astar::{Graph, GraphVertex, PathKind},
    };
    use std::time::Instant;

    #[test]
    fn astar_random_points() {
        let mut pathfinder = Graph::<GraphVertex>::new();

        let mut path = Vec::new();
        assert!(pathfinder.build(0, 0, &mut path).is_ok());
        assert!(path.is_empty());

        let size = 40;

        // Create vertices.
        let mut vertices = Vec::new();
        for y in 0..size {
            for x in 0..size {
                vertices.push(GraphVertex::new(Vector3::new(x as f32, y as f32, 0.0)));
            }
        }
        pathfinder.set_vertices(vertices);

        assert!(pathfinder.build(100000, 99999, &mut path).is_err());

        // Link vertices as grid.
        for y in 0..(size - 1) {
            for x in 0..(size - 1) {
                pathfinder.link_bidirect(y * size + x, y * size + x + 1);
                pathfinder.link_bidirect(y * size + x, (y + 1) * size + x);
            }
        }

        let mut paths_count = 0;

        for _ in 0..1000 {
            let sx = rand::thread_rng().gen_range(0..(size - 1));
            let sy = rand::thread_rng().gen_range(0..(size - 1));

            let tx = rand::thread_rng().gen_range(0..(size - 1));
            let ty = rand::thread_rng().gen_range(0..(size - 1));

            let from = sy * size + sx;
            let to = ty * size + tx;

            assert!(pathfinder.build(from, to, &mut path).is_ok());
            assert!(!path.is_empty());

            if path.len() > 1 {
                paths_count += 1;

                assert_eq!(
                    *path.first().unwrap(),
                    pathfinder.vertex(to).unwrap().position
                );
                assert_eq!(
                    *path.last().unwrap(),
                    pathfinder.vertex(from).unwrap().position
                );
            } else {
                let point = *path.first().unwrap();
                assert_eq!(point, pathfinder.vertex(to).unwrap().position);
                assert_eq!(point, pathfinder.vertex(from).unwrap().position);
            }

            for pair in path.chunks(2) {
                if pair.len() == 2 {
                    let a = pair[0];
                    let b = pair[1];

                    assert!(a.metric_distance(&b) <= 2.0f32.sqrt());
                }
            }
        }

        assert!(paths_count > 0);
    }

    #[test]
    fn test_remove_vertex() {
        let mut pathfinder = Graph::<GraphVertex>::new();

        pathfinder.add_vertex(GraphVertex::new(Vector3::new(0.0, 0.0, 0.0)));
        pathfinder.add_vertex(GraphVertex::new(Vector3::new(1.0, 0.0, 0.0)));
        pathfinder.add_vertex(GraphVertex::new(Vector3::new(1.0, 1.0, 0.0)));

        pathfinder.link_bidirect(0, 1);
        pathfinder.link_bidirect(1, 2);
        pathfinder.link_bidirect(2, 0);

        pathfinder.remove_vertex(0);

        assert_eq!(pathfinder.vertex(0).unwrap().neighbours, vec![1]);
        assert_eq!(pathfinder.vertex(1).unwrap().neighbours, vec![0]);
        assert_eq!(pathfinder.vertex(2), None);

        pathfinder.remove_vertex(0);

        assert_eq!(pathfinder.vertex(0).unwrap().neighbours, vec![]);
        assert_eq!(pathfinder.vertex(1), None);
        assert_eq!(pathfinder.vertex(2), None);
    }

    #[test]
    fn test_insert_vertex() {
        let mut pathfinder = Graph::new();

        pathfinder.add_vertex(GraphVertex::new(Vector3::new(0.0, 0.0, 0.0)));
        pathfinder.add_vertex(GraphVertex::new(Vector3::new(1.0, 0.0, 0.0)));
        pathfinder.add_vertex(GraphVertex::new(Vector3::new(1.0, 1.0, 0.0)));

        pathfinder.link_bidirect(0, 1);
        pathfinder.link_bidirect(1, 2);
        pathfinder.link_bidirect(2, 0);

        assert_eq!(pathfinder.vertex(0).unwrap().neighbours, vec![1, 2]);
        assert_eq!(pathfinder.vertex(1).unwrap().neighbours, vec![0, 2]);
        assert_eq!(pathfinder.vertex(2).unwrap().neighbours, vec![1, 0]);

        pathfinder.insert_vertex(0, GraphVertex::new(Vector3::new(1.0, 1.0, 1.0)));

        assert_eq!(pathfinder.vertex(0).unwrap().neighbours, vec![]);
        assert_eq!(pathfinder.vertex(1).unwrap().neighbours, vec![2, 3]);
        assert_eq!(pathfinder.vertex(2).unwrap().neighbours, vec![1, 3]);
        assert_eq!(pathfinder.vertex(3).unwrap().neighbours, vec![2, 1]);
    }

    #[test]
    /// Tests A*'s speed when finding a direct path with no obsticles
    fn astar_complete_grid_benchmark() {
        let start_time = Instant::now();
        let mut path = Vec::new();

        println!();
        for size in [10, 40, 100, 500] {
            println!("benchmarking grid size of: {}^2", size);
            let setup_start_time = Instant::now();

            let mut pathfinder = Graph::new();

            // Create vertices.
            let mut vertices = Vec::new();
            for y in 0..size {
                for x in 0..size {
                    vertices.push(GraphVertex::new(Vector3::new(x as f32, y as f32, 0.0)));
                }
            }
            pathfinder.set_vertices(vertices);

            // Link vertices as grid.
            for y in 0..(size - 1) {
                for x in 0..(size - 1) {
                    pathfinder.link_bidirect(y * size + x, y * size + x + 1);
                    pathfinder.link_bidirect(y * size + x, (y + 1) * size + x);
                }
            }

            let setup_complete_time = Instant::now();
            println!(
                "setup in: {:?}",
                setup_complete_time.duration_since(setup_start_time)
            );

            for _ in 0..1000 {
                let sx = rand::thread_rng().gen_range(0..(size - 1));
                let sy = rand::thread_rng().gen_range(0..(size - 1));

                let tx = rand::thread_rng().gen_range(0..(size - 1));
                let ty = rand::thread_rng().gen_range(0..(size - 1));

                let from = sy * size + sx;
                let to = ty * size + tx;

                assert!(pathfinder.build(from, to, &mut path).is_ok());
                assert!(!path.is_empty());

                if path.len() > 1 {
                    assert_eq!(
                        *path.first().unwrap(),
                        pathfinder.vertex(to).unwrap().position
                    );
                    assert_eq!(
                        *path.last().unwrap(),
                        pathfinder.vertex(from).unwrap().position
                    );
                } else {
                    let point = *path.first().unwrap();
                    assert_eq!(point, pathfinder.vertex(to).unwrap().position);
                    assert_eq!(point, pathfinder.vertex(from).unwrap().position);
                }

                for pair in path.chunks(2) {
                    if pair.len() == 2 {
                        let a = pair[0];
                        let b = pair[1];

                        assert!(a.metric_distance(&b) <= 2.0f32.sqrt());
                    }
                }
            }
            println!("paths found in: {:?}", setup_complete_time.elapsed());
            println!(
                "Current size complete in: {:?}\n",
                setup_start_time.elapsed()
            );
        }
        println!("Total time: {:?}\n", start_time.elapsed());
    }

    #[test]
    /// Tests A*'s speed when finding partial paths (no direct path available)
    fn astar_island_benchmark() {
        let start_time = Instant::now();

        let size = 100;
        let mut path = Vec::new();
        let mut pathfinder = Graph::new();

        // Create vertices.
        let mut vertices = Vec::new();
        for y in 0..size {
            for x in 0..size {
                vertices.push(GraphVertex::new(Vector3::new(x as f32, y as f32, 0.0)));
            }
        }
        pathfinder.set_vertices(vertices);

        // Link vertices as grid.
        // seperates grids half way down the y-axis
        for y in 0..(size - 1) {
            for x in 0..(size - 1) {
                if x != ((size / 2) - 1) {
                    pathfinder.link_bidirect(y * size + x, y * size + x + 1);
                }
                pathfinder.link_bidirect(y * size + x, (y + 1) * size + x);
            }
        }

        let setup_complete_time = Instant::now();

        println!(
            "\nsetup in: {:?}",
            setup_complete_time.duration_since(start_time)
        );

        for _ in 0..1000 {
            // generates a random start point on the left half of the grid
            let sx = rand::thread_rng().gen_range(0..((size / 2) - 1));
            let sy = rand::thread_rng().gen_range(0..(size - 1));

            // generates a random end point on the right half of the grid
            let tx = rand::thread_rng().gen_range((size / 2)..(size - 1));
            let ty = rand::thread_rng().gen_range(0..(size - 1));

            let from = sy * size + sx;
            let to = ty * size + tx;

            let path_result = pathfinder.build(from, to, &mut path);

            assert!(path_result.is_ok());
            assert_eq!(path_result.unwrap(), PathKind::Partial);
            assert!(!path.is_empty());

            if path.len() > 1 {
                // partial path should be along the divide and at the same y height as end point
                // let best_end = ty * size + ((size / 2) - 1);
                // assert_eq!(
                //     *path.first().unwrap(),
                //     pathfinder.vertex(best_end).unwrap().position
                // );

                // partial path should be along the divide
                assert_eq!(path.first().unwrap().x as i32, ((size / 2) - 1) as i32);
                // start point should be start point
                assert_eq!(
                    *path.last().unwrap(),
                    pathfinder.vertex(from).unwrap().position
                );
            } else {
                let point = *path.first().unwrap();
                assert_eq!(point, pathfinder.vertex(to).unwrap().position);
                assert_eq!(point, pathfinder.vertex(from).unwrap().position);
            }

            for pair in path.chunks(2) {
                if pair.len() == 2 {
                    let a = pair[0];
                    let b = pair[1];

                    assert!(a.metric_distance(&b) <= 2.0f32.sqrt());
                }
            }
        }

        println!("paths found in: {:?}", setup_complete_time.elapsed());
        println!("Total time: {:?}\n", start_time.elapsed());
    }

    #[test]
    /// Tests A*'s speed when when finding indirect paths (major obstacle in the way)
    fn astar_backwards_travel_benchmark() {
        let start_time = Instant::now();

        let size = 100;
        let mut path = Vec::new();
        let mut pathfinder = Graph::new();

        // Create vertices.
        let mut vertices = Vec::new();
        for y in 0..size {
            for x in 0..size {
                vertices.push(GraphVertex::new(Vector3::new(x as f32, y as f32, 0.0)));
            }
        }
        pathfinder.set_vertices(vertices);

        // Link vertices as grid.
        // seperates grid diagonally down the xy plane leaving only one connection in the corner
        for y in 0..(size - 1) {
            for x in (0..(size - 1)).rev() {
                if y == 0 || x != y {
                    pathfinder.link_bidirect(y * size + x, y * size + x + 1);
                    pathfinder.link_bidirect(y * size + x, (y + 1) * size + x);
                }
            }
        }

        let setup_complete_time = Instant::now();

        println!(
            "\nsetup in: {:?}",
            setup_complete_time.duration_since(start_time)
        );

        for _ in 0..1000 {
            // a point on the center right edge
            let from = (size / 2) * size + (size - 1);
            // a point on the center top edge
            let to = (size - 1) * size + (size / 2);

            assert!(pathfinder.build(from, to, &mut path).is_ok());
            assert!(!path.is_empty());

            if path.len() > 1 {
                assert_eq!(
                    *path.first().unwrap(),
                    pathfinder.vertex(to).unwrap().position
                );
                assert_eq!(
                    *path.last().unwrap(),
                    pathfinder.vertex(from).unwrap().position
                );
            } else {
                let point = *path.first().unwrap();
                assert_eq!(point, pathfinder.vertex(to).unwrap().position);
                assert_eq!(point, pathfinder.vertex(from).unwrap().position);
            }

            for pair in path.chunks(2) {
                if pair.len() == 2 {
                    let a = pair[0];
                    let b = pair[1];

                    assert!(a.metric_distance(&b) <= 2.0f32.sqrt());
                }
            }
        }

        println!("paths found in: {:?}", setup_complete_time.elapsed());
        println!("Total time: {:?}\n", start_time.elapsed());
    }
}
