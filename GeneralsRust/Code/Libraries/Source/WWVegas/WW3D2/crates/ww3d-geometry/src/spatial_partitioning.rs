//! Spatial Partitioning Systems
//!
//! This module provides spatial partitioning data structures for efficient
//! spatial queries and culling. Includes octrees, BSP trees, and portal systems.

use crate::collision::sphere_aabb_intersect;
use crate::*;
use glam::Vec3;
use std::collections::HashMap;

/// Octree node for spatial partitioning
#[derive(Debug)]
pub struct OctreeNode {
    pub bounds: AABox,
    pub children: Option<Box<[OctreeNode; 8]>>,
    pub objects: Vec<SpatialObject>,
    pub max_objects_per_node: usize,
    pub max_depth: usize,
    pub depth: usize,
}

impl OctreeNode {
    /// Create a new octree node
    pub fn new(bounds: AABox, max_objects_per_node: usize, max_depth: usize, depth: usize) -> Self {
        Self {
            bounds,
            children: None,
            objects: Vec::new(),
            max_objects_per_node,
            max_depth,
            depth,
        }
    }

    /// Insert an object into the octree
    pub fn insert(&mut self, object: SpatialObject) {
        if !self.bounds.contains_point(object.position) {
            return;
        }

        if self.children.is_none()
            && self.objects.len() < self.max_objects_per_node
            && self.depth < self.max_depth
        {
            self.objects.push(object);
        } else {
            if self.children.is_none() {
                self.subdivide();
            }

            if let Some(children) = &mut self.children {
                for child in children.iter_mut() {
                    child.insert(object.clone());
                }
            } else {
                self.objects.push(object);
            }
        }
    }

    /// Subdivide this node into 8 children
    fn subdivide(&mut self) {
        let half_extent = self.bounds.extent / 2.0;
        let center = self.bounds.center;

        let offsets = [
            Vec3::new(-half_extent.x, -half_extent.y, -half_extent.z),
            Vec3::new(half_extent.x, -half_extent.y, -half_extent.z),
            Vec3::new(-half_extent.x, half_extent.y, -half_extent.z),
            Vec3::new(half_extent.x, half_extent.y, -half_extent.z),
            Vec3::new(-half_extent.x, -half_extent.y, half_extent.z),
            Vec3::new(half_extent.x, -half_extent.y, half_extent.z),
            Vec3::new(-half_extent.x, half_extent.y, half_extent.z),
            Vec3::new(half_extent.x, half_extent.y, half_extent.z),
        ];

        let mut children = Vec::with_capacity(8);
        for offset in &offsets {
            let child_center = center + *offset;
            let child_bounds = AABox::new(child_center, half_extent);
            children.push(OctreeNode::new(
                child_bounds,
                self.max_objects_per_node,
                self.max_depth,
                self.depth + 1,
            ));
        }

        self.children = Some(children.into_boxed_slice().try_into().unwrap());
    }

    /// Query objects within a sphere
    pub fn query_sphere(&self, sphere: &Sphere) -> Vec<&SpatialObject> {
        let mut result = Vec::new();

        if !sphere_aabb_intersect(sphere, &self.bounds) {
            return result;
        }

        for object in &self.objects {
            if (object.position - sphere.center).length_squared() <= sphere.radius * sphere.radius {
                result.push(object);
            }
        }

        if let Some(children) = &self.children {
            for child in children.iter() {
                result.extend(child.query_sphere(sphere));
            }
        }

        result
    }

    /// Query objects within an AABB
    pub fn query_aabb(&self, aabb: &AABox) -> Vec<&SpatialObject> {
        let mut result = Vec::new();

        if !self.bounds.intersects_aabox(aabb) {
            return result;
        }

        for object in &self.objects {
            if aabb.contains_point(object.position) {
                result.push(object);
            }
        }

        if let Some(children) = &self.children {
            for child in children.iter() {
                result.extend(child.query_aabb(aabb));
            }
        }

        result
    }

    /// Query objects within a frustum
    pub fn query_frustum(&self, frustum: &Frustum) -> Vec<&SpatialObject> {
        let mut result = Vec::new();

        if !frustum.intersects_aabox(&self.bounds) {
            return result;
        }

        for object in &self.objects {
            if frustum.contains_point(object.position) {
                result.push(object);
            }
        }

        if let Some(children) = &self.children {
            for child in children.iter() {
                result.extend(child.query_frustum(frustum));
            }
        }

        result
    }
}

/// Octree spatial partitioning system
#[derive(Debug)]
pub struct Octree {
    pub root: OctreeNode,
}

impl Octree {
    /// Create a new octree
    pub fn new(bounds: AABox, max_objects_per_node: usize, max_depth: usize) -> Self {
        Self {
            root: OctreeNode::new(bounds, max_objects_per_node, max_depth, 0),
        }
    }

    /// Insert an object
    pub fn insert(&mut self, object: SpatialObject) {
        self.root.insert(object);
    }

    /// Query objects within a sphere
    pub fn query_sphere(&self, sphere: &Sphere) -> Vec<&SpatialObject> {
        self.root.query_sphere(sphere)
    }

    /// Query objects within an AABB
    pub fn query_aabb(&self, aabb: &AABox) -> Vec<&SpatialObject> {
        self.root.query_aabb(aabb)
    }

    /// Query objects within a frustum
    pub fn query_frustum(&self, frustum: &Frustum) -> Vec<&SpatialObject> {
        self.root.query_frustum(frustum)
    }
}

/// BSP tree node
#[derive(Debug)]
pub struct BSPNode {
    pub plane: Plane,
    pub front: Option<Box<BSPNode>>,
    pub back: Option<Box<BSPNode>>,
    pub objects: Vec<SpatialObject>,
}

impl BSPNode {
    /// Create a new BSP node
    pub fn new(plane: Plane) -> Self {
        Self {
            plane,
            front: None,
            back: None,
            objects: Vec::new(),
        }
    }

    /// Insert an object into the BSP tree
    pub fn insert(&mut self, object: SpatialObject) {
        match self.plane.classify_point(object.position) {
            PlaneClassification::Front => {
                if let Some(front) = &mut self.front {
                    front.insert(object);
                } else {
                    self.objects.push(object);
                }
            }
            PlaneClassification::Back => {
                if let Some(back) = &mut self.back {
                    back.insert(object);
                } else {
                    self.objects.push(object);
                }
            }
            PlaneClassification::OnPlane => {
                self.objects.push(object);
            }
        }
    }

    /// Query objects in front of a point
    pub fn query_front(&self, point: Vec3) -> Vec<&SpatialObject> {
        let mut result = Vec::new();

        match self.plane.classify_point(point) {
            PlaneClassification::Front => {
                result.extend(&self.objects);
                if let Some(front) = &self.front {
                    result.extend(front.query_front(point));
                }
            }
            PlaneClassification::Back => {
                if let Some(front) = &self.front {
                    result.extend(front.query_front(point));
                }
            }
            PlaneClassification::OnPlane => {
                result.extend(&self.objects);
            }
        }

        result
    }
}

/// BSP tree spatial partitioning system
#[derive(Debug)]
pub struct BSPTree {
    pub root: Option<Box<BSPNode>>,
}

impl BSPTree {
    /// Create a new BSP tree
    pub fn new() -> Self {
        Self { root: None }
    }

    /// Build BSP tree from objects
    pub fn build(&mut self, objects: &[SpatialObject]) {
        if objects.is_empty() {
            return;
        }

        // Find a good splitting plane (simplified: use first object position)
        let split_point = objects[0].position;
        let split_normal = Vec3::new(1.0, 0.0, 0.0); // X-axis split
        let split_plane = Plane::from_point_normal(split_point, split_normal);

        let mut root = Box::new(BSPNode::new(split_plane));

        for object in objects {
            root.insert(object.clone());
        }

        self.root = Some(root);
    }

    /// Query objects in front of a point
    pub fn query_front(&self, point: Vec3) -> Vec<&SpatialObject> {
        if let Some(root) = &self.root {
            root.query_front(point)
        } else {
            Vec::new()
        }
    }
}

/// Portal system for visibility culling
#[derive(Debug)]
pub struct PortalSystem {
    pub portals: Vec<Portal>,
    pub sectors: HashMap<usize, Sector>,
}

impl PortalSystem {
    /// Create a new portal system
    pub fn new() -> Self {
        Self {
            portals: Vec::new(),
            sectors: HashMap::new(),
        }
    }

    /// Add a portal between sectors
    pub fn add_portal(&mut self, portal: Portal) {
        self.portals.push(portal);
    }

    /// Add a sector
    pub fn add_sector(&mut self, sector: Sector) {
        self.sectors.insert(sector.id, sector);
    }

    /// Perform portal visibility culling
    pub fn cull_visibility(&self, camera_pos: Vec3, view_frustum: &Frustum) -> Vec<usize> {
        let mut visible_sectors = Vec::new();
        let mut visited = std::collections::HashSet::new();

        // Find initial sector containing camera
        let initial_sector = self.find_sector(camera_pos);
        if let Some(sector_id) = initial_sector {
            self.portal_traversal(sector_id, view_frustum, &mut visible_sectors, &mut visited);
        }

        visible_sectors
    }

    /// Traverse portals to find visible sectors
    fn portal_traversal(
        &self,
        sector_id: usize,
        view_frustum: &Frustum,
        visible: &mut Vec<usize>,
        visited: &mut std::collections::HashSet<usize>,
    ) {
        if visited.contains(&sector_id) {
            return;
        }

        visited.insert(sector_id);
        visible.push(sector_id);

        // Process portals from this sector
        for portal in &self.portals {
            if portal.sector_a == sector_id || portal.sector_b == sector_id {
                let other_sector = if portal.sector_a == sector_id {
                    portal.sector_b
                } else {
                    portal.sector_a
                };

                // Check if portal is visible through frustum
                if self.portal_visible(portal, view_frustum) {
                    self.portal_traversal(other_sector, view_frustum, visible, visited);
                }
            }
        }
    }

    /// Check if portal is visible through frustum
    fn portal_visible(&self, portal: &Portal, frustum: &Frustum) -> bool {
        // Simplified: check if any portal vertex is in frustum
        for vertex in &portal.vertices {
            if frustum.contains_point(*vertex) {
                return true;
            }
        }
        false
    }

    /// Find sector containing a point
    fn find_sector(&self, point: Vec3) -> Option<usize> {
        for (id, sector) in &self.sectors {
            if sector.bounds.contains_point(point) {
                return Some(*id);
            }
        }
        None
    }
}

/// Portal between sectors
#[derive(Debug, Clone)]
pub struct Portal {
    pub vertices: Vec<Vec3>,
    pub sector_a: usize,
    pub sector_b: usize,
}

/// Sector in portal system
#[derive(Debug, Clone)]
pub struct Sector {
    pub id: usize,
    pub bounds: AABox,
    pub objects: Vec<SpatialObject>,
}

/// Generic spatial object
#[derive(Debug)]
pub struct SpatialObject {
    pub id: usize,
    pub position: Vec3,
    pub bounds: AABox,
    pub user_data: Option<Box<dyn std::any::Any>>,
}

impl Clone for SpatialObject {
    fn clone(&self) -> Self {
        Self {
            id: self.id,
            position: self.position,
            bounds: self.bounds,
            user_data: None, // Don't clone user data as it may not be cloneable
        }
    }
}

impl SpatialObject {
    /// Create a new spatial object
    pub fn new(id: usize, position: Vec3, bounds: AABox) -> Self {
        Self {
            id,
            position,
            bounds,
            user_data: None,
        }
    }

    /// Clone this spatial object (user_data is not cloned)
    pub fn clone_shallow(&self) -> Self {
        Self {
            id: self.id,
            position: self.position,
            bounds: self.bounds,
            user_data: None, // Don't clone user data
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_octree_creation() {
        let bounds = AABox::new(Vec3::ZERO, Vec3::new(10.0, 10.0, 10.0));
        let mut octree = Octree::new(bounds, 4, 5);

        let object = SpatialObject::new(
            1,
            Vec3::new(1.0, 1.0, 1.0),
            AABox::new(Vec3::ZERO, Vec3::new(0.5, 0.5, 0.5)),
        );
        octree.insert(object);

        let query_sphere = Sphere::new(Vec3::new(1.0, 1.0, 1.0), 1.0);
        let results = octree.query_sphere(&query_sphere);
        assert_eq!(results.len(), 1);
    }

    #[test]
    fn test_bsp_tree_creation() {
        let mut bsp_tree = BSPTree::new();

        let objects = vec![
            SpatialObject::new(
                1,
                Vec3::new(1.0, 0.0, 0.0),
                AABox::new(Vec3::ZERO, Vec3::new(0.5, 0.5, 0.5)),
            ),
            SpatialObject::new(
                2,
                Vec3::new(-1.0, 0.0, 0.0),
                AABox::new(Vec3::ZERO, Vec3::new(0.5, 0.5, 0.5)),
            ),
        ];

        bsp_tree.build(&objects);

        assert!(bsp_tree.root.is_some());
    }

    #[test]
    fn test_portal_system() {
        let mut portal_system = PortalSystem::new();

        let sector1 = Sector {
            id: 1,
            bounds: AABox::new(Vec3::new(-5.0, 0.0, 0.0), Vec3::new(5.0, 5.0, 5.0)),
            objects: Vec::new(),
        };

        let sector2 = Sector {
            id: 2,
            bounds: AABox::new(Vec3::new(5.0, 0.0, 0.0), Vec3::new(5.0, 5.0, 5.0)),
            objects: Vec::new(),
        };

        portal_system.add_sector(sector1);
        portal_system.add_sector(sector2);

        let portal = Portal {
            vertices: vec![
                Vec3::new(0.0, -5.0, -5.0),
                Vec3::new(0.0, 5.0, -5.0),
                Vec3::new(0.0, 5.0, 5.0),
                Vec3::new(0.0, -5.0, 5.0),
            ],
            sector_a: 1,
            sector_b: 2,
        };

        portal_system.add_portal(portal);

        let camera_pos = Vec3::new(-2.0, 0.0, 0.0);
        let frustum =
            Frustum::from_matrix(glam::Mat4::perspective_rh(PI / 3.0, 16.0 / 9.0, 0.1, 100.0));

        let visible_sectors = portal_system.cull_visibility(camera_pos, &frustum);
        assert!(visible_sectors.contains(&1));
    }
}
