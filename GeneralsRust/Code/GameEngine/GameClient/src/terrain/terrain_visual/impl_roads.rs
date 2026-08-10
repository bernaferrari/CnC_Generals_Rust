// Split from `terrain/terrain_visual.rs` dump. Included by `terrain_visual/mod.rs`.

impl TerrainVisualImpl {
    /// Export minimap-ready road samples for static map overlays.
    pub fn minimap_road_samples(&self, samples_per_segment: u32) -> Vec<RoadMinimapSample> {
        self.road_system
            .snapshot_minimap_samples(samples_per_segment)
    }

    fn quantize_runtime_road_coord(value: f32) -> i32 {
        (value * 100.0).round() as i32
    }

    fn runtime_points_equal(a: Vec3, b: Vec3) -> bool {
        Self::quantize_runtime_road_coord(a.x) == Self::quantize_runtime_road_coord(b.x)
            && Self::quantize_runtime_road_coord(a.y) == Self::quantize_runtime_road_coord(b.y)
            && Self::quantize_runtime_road_coord(a.z) == Self::quantize_runtime_road_coord(b.z)
    }

    fn runtime_endpoint_match_count(
        ordered_segments: &[RuntimeRoadVisualSegment],
        road_type_id: u32,
        point: Vec3,
    ) -> usize {
        ordered_segments
            .iter()
            .filter(|segment| segment.road_type_id == road_type_id)
            .map(|segment| {
                let start = Vec3::from_array(segment.start);
                let end = Vec3::from_array(segment.end);
                let mut matches = 0usize;
                if Self::runtime_points_equal(start, point) {
                    matches += 1;
                }
                if Self::runtime_points_equal(end, point) {
                    matches += 1;
                }
                matches
            })
            .sum()
    }

    fn runtime_ground_from_point(point: Vec3) -> Vec2 {
        // Runtime road shaping follows the C++ XY map plane; in Rust terrain space this is XZ.
        Vec2::new(point.x, point.z)
    }

    fn runtime_point_from_ground(ground: Vec2, height: f32) -> [f32; 3] {
        [ground.x, height, ground.y]
    }

    fn runtime_rotate_2d(vector: Vec2, angle: f32) -> Vec2 {
        let sin = angle.sin();
        let cos = angle.cos();
        Vec2::new(
            vector.x * cos - vector.y * sin,
            vector.x * sin + vector.y * cos,
        )
    }

    fn runtime_rotate_about(point: Vec2, center: Vec2, angle: f32) -> Vec2 {
        center + Self::runtime_rotate_2d(point - center, angle)
    }

    fn runtime_line_intersection(a1: Vec2, a2: Vec2, b1: Vec2, b2: Vec2) -> Option<Vec2> {
        let r = a2 - a1;
        let s = b2 - b1;
        let denom = r.perp_dot(s);
        if denom.abs() <= 1.0e-6 {
            return None;
        }
        let t = (b1 - a1).perp_dot(s) / denom;
        Some(a1 + r * t)
    }

    fn append_runtime_curve_segment(
        curves: &mut Vec<RuntimeRoadVisualSegment>,
        source: &RuntimeRoadVisualSegment,
        start: Vec2,
        end: Vec2,
        height: f32,
    ) {
        if (end - start).length_squared() <= 1.0e-5 {
            return;
        }

        curves.push(RuntimeRoadVisualSegment {
            start: Self::runtime_point_from_ground(start, height),
            end: Self::runtime_point_from_ground(end, height),
            width: source.width.max(0.1),
            template_name: source.template_name.clone(),
            width_in_texture: source.width_in_texture.max(0.0),
            road_type_id: source.road_type_id,
            start_is_angled: false,
            start_is_join: false,
            end_is_angled: false,
            end_is_join: false,
            curve_radius: source.curve_radius,
        });
    }

    fn insert_runtime_curve_segment_at(
        segment_a: &mut RuntimeRoadVisualSegment,
        segment_b: &mut RuntimeRoadVisualSegment,
    ) -> Vec<RuntimeRoadVisualSegment> {
        const DOT_LIMIT: f32 = 0.5;

        if segment_a.road_type_id != segment_b.road_type_id {
            return Vec::new();
        }

        let radius = segment_a.curve_radius.max(0.0) * segment_a.width.max(0.0);
        if radius <= 1.0e-3 {
            return Vec::new();
        }

        let source_a = segment_a.clone();
        let shared_height = (segment_a.start[1] + segment_b.end[1]) * 0.5;

        let seg_a_start = Self::runtime_ground_from_point(Vec3::from_array(segment_a.start));
        let seg_a_end = Self::runtime_ground_from_point(Vec3::from_array(segment_a.end));
        let seg_b_start = Self::runtime_ground_from_point(Vec3::from_array(segment_b.start));
        let seg_b_end = Self::runtime_ground_from_point(Vec3::from_array(segment_b.end));

        let original_corner = seg_a_start;

        let mut line1_dir = (seg_a_end - seg_a_start).normalize_or_zero();
        let mut line2_dir = (seg_b_end - seg_b_start).normalize_or_zero();
        if line1_dir.length_squared() <= 1.0e-6 || line2_dir.length_squared() <= 1.0e-6 {
            return Vec::new();
        }

        let turn_cross = line1_dir.perp_dot(line2_dir);
        let turn_right = turn_cross > 0.0;

        let mut pr1;
        let pr2;
        let pr3;
        let mut pr4;
        if turn_right {
            pr1 = seg_a_start;
            pr2 = seg_a_end;
            pr3 = seg_b_start;
            pr4 = seg_b_end;
        } else {
            pr4 = seg_a_start;
            pr3 = seg_a_end;
            pr2 = seg_b_start;
            pr1 = seg_b_end;
            line1_dir = (pr2 - pr1).normalize_or_zero();
            line2_dir = (pr4 - pr3).normalize_or_zero();
        }

        if line1_dir.length_squared() <= 1.0e-6 || line2_dir.length_squared() <= 1.0e-6 {
            return Vec::new();
        }

        let cur_sin = line1_dir.dot(line2_dir).clamp(-1.0, 1.0);
        let angle = cur_sin.acos();
        let count = angle / (PI / 6.0);
        if count < 0.9 || segment_a.start_is_angled {
            return Vec::new();
        }

        let offset1 = Self::runtime_rotate_2d(line1_dir * radius, -PI / 2.0);
        let offset2 = Self::runtime_rotate_2d(line2_dir * radius, -PI / 2.0);

        let offset_intersection = Self::runtime_line_intersection(
            pr1 + offset1,
            pr2 + offset1,
            pr3 + offset2,
            pr4 + offset2,
        );
        let Some(mut p_int1) = offset_intersection else {
            return Vec::new();
        };

        let cross1_intersection =
            Self::runtime_line_intersection(p_int1, p_int1 - offset2, pr3, pr4);
        let cross2_intersection =
            Self::runtime_line_intersection(p_int1, p_int1 - offset1, pr1, pr2);
        let (Some(cross1), Some(p_int3)) = (cross1_intersection, cross2_intersection) else {
            return Vec::new();
        };
        p_int1 = cross1;

        let dot1 = line2_dir.dot(p_int1 - pr3);
        let dot2 = line1_dir.dot(pr2 - p_int3);
        if dot1 < DOT_LIMIT || dot2 < DOT_LIMIT {
            segment_a.start = Self::runtime_point_from_ground(original_corner, segment_a.start[1]);
            segment_b.end = Self::runtime_point_from_ground(original_corner, segment_b.end[1]);
            return Vec::new();
        }

        pr4 = p_int1;
        let mut curves = Vec::new();

        let angle_step = -PI / 6.0;
        let mut pt2 = pr4;
        let mut pt1 = pr3;
        let mut direction = pt1 - pt2;
        let mut center_of_curve = Vec2::new(-direction.y, direction.x).normalize_or_zero();
        if center_of_curve.length_squared() <= 1.0e-6 {
            return Vec::new();
        }
        center_of_curve *= radius;
        center_of_curve += pt2;

        pt2 = Self::runtime_rotate_about(pt2, center_of_curve, angle_step);
        direction = Self::runtime_rotate_2d(direction, angle_step);
        pt1 = pt2 + direction;
        Self::append_runtime_curve_segment(&mut curves, &source_a, pt2, pt1, shared_height);

        if count > 2.0 {
            let mut i = 2_i32;
            while (i as f32) < count {
                direction = Self::runtime_rotate_2d(direction, angle_step);
                pt2 = Self::runtime_rotate_about(pt2, center_of_curve, angle_step);
                pt1 = pt2 + direction;
                Self::append_runtime_curve_segment(&mut curves, &source_a, pt2, pt1, shared_height);
                i += 1;
            }
        }

        pr1 = p_int3;
        if count > 1.0 {
            pt2 = pr1;
            pt1 = pr1 + pr1 - pr2;
            direction = pt1 - pt2;
            pt1 = pt2 + direction;
            Self::append_runtime_curve_segment(&mut curves, &source_a, pt2, pt1, shared_height);
        }

        if turn_right {
            segment_a.start = Self::runtime_point_from_ground(pr1, segment_a.start[1]);
            segment_b.end = Self::runtime_point_from_ground(pr4, segment_b.end[1]);
        } else {
            segment_a.start = Self::runtime_point_from_ground(pr4, segment_a.start[1]);
            segment_b.end = Self::runtime_point_from_ground(pr1, segment_b.end[1]);
        }

        curves
    }

    fn insert_runtime_curve_segments(
        ordered_segments: &mut Vec<RuntimeRoadVisualSegment>,
        topology: &[RuntimeRoadEndpointTopology],
    ) {
        let original_segment_count = ordered_segments.len().min(topology.len());
        if original_segment_count <= 1 {
            return;
        }

        let mut segment_start_index: Option<usize> = None;
        for i in 0..original_segment_count {
            let mut try_insert_pair = None::<(usize, usize)>;
            let adjacent_match = if i + 1 < original_segment_count {
                let current_start = Vec3::from_array(ordered_segments[i].start);
                let next_end = Vec3::from_array(ordered_segments[i + 1].end);
                Self::runtime_points_equal(current_start, next_end)
            } else {
                false
            };

            if adjacent_match {
                if topology[i + 1].end_count == 1 && topology[i].start_count == 1 {
                    try_insert_pair = Some((i, i + 1));
                    if segment_start_index.is_none() {
                        segment_start_index = Some(i);
                    }
                }
            } else if let Some(start_index) = segment_start_index {
                let current_start = Vec3::from_array(ordered_segments[i].start);
                let start_end = Vec3::from_array(ordered_segments[start_index].end);
                if Self::runtime_points_equal(current_start, start_end)
                    && topology[start_index].end_count == 1
                    && topology[i].start_count == 1
                {
                    try_insert_pair = Some((i, start_index));
                }
                segment_start_index = None;
            }

            let Some((a, b)) = try_insert_pair else {
                continue;
            };
            if a == b {
                continue;
            }

            let curves = if a < b {
                let (left, right) = ordered_segments.split_at_mut(b);
                Self::insert_runtime_curve_segment_at(&mut left[a], &mut right[0])
            } else {
                let (left, right) = ordered_segments.split_at_mut(a);
                Self::insert_runtime_curve_segment_at(&mut right[0], &mut left[b])
            };
            if !curves.is_empty() {
                ordered_segments.extend(curves);
            }
        }
    }

    fn runtime_xp_sign(v1: Vec2, v2: Vec2) -> i32 {
        let cross = v1.perp_dot(v2);
        if cross < 0.0 {
            -1
        } else if cross > 0.0 {
            1
        } else {
            0
        }
    }

    fn runtime_closest_point_on_segment(point: Vec2, a: Vec2, b: Vec2) -> Vec2 {
        let ab = b - a;
        let denom = ab.length_squared();
        if denom <= 1.0e-6 {
            return a;
        }
        let t = ((point - a).dot(ab) / denom).clamp(0.0, 1.0);
        a + ab * t
    }

    fn runtime_adjust_stacking(
        stackings: &mut HashMap<u32, i32>,
        top_unique_id: u32,
        bottom_unique_id: u32,
    ) {
        let Some(top_stacking) = stackings.get(&top_unique_id).copied() else {
            return;
        };
        let Some(bottom_stacking) = stackings.get(&bottom_unique_id).copied() else {
            return;
        };
        if top_stacking > bottom_stacking {
            return;
        }

        let new_stacking = bottom_stacking;
        for stacking in stackings.values_mut() {
            if *stacking > new_stacking {
                *stacking += 1;
            }
        }
        if let Some(stacking) = stackings.get_mut(&top_unique_id) {
            *stacking = new_stacking + 1;
        }
    }

    fn runtime_find_cross_type_join_vector(
        loc: Vec2,
        join_vector: Vec2,
        unique_id: u32,
        ordered_segments: &[RuntimeRoadVisualSegment],
        base_segment_count: usize,
    ) -> Option<(u32, Vec2)> {
        let mut new_vector = join_vector;
        for segment in ordered_segments.iter().take(base_segment_count) {
            if segment.road_type_id == unique_id {
                continue;
            }

            let loc1 = Self::runtime_ground_from_point(Vec3::from_array(segment.start));
            let loc2 = Self::runtime_ground_from_point(Vec3::from_array(segment.end));
            let half_width = segment.width.max(0.1) * 0.5;
            let bounds_min = Vec2::new(
                loc1.x.min(loc2.x) - half_width,
                loc1.y.min(loc2.y) - half_width,
            );
            let bounds_max = Vec2::new(
                loc1.x.max(loc2.x) + half_width,
                loc1.y.max(loc2.y) + half_width,
            );
            if loc.x < bounds_min.x
                || loc.y < bounds_min.y
                || loc.x > bounds_max.x
                || loc.y > bounds_max.y
            {
                continue;
            }

            let closest = Self::runtime_closest_point_on_segment(loc, loc1, loc2);
            let dist = (closest - loc).length();
            if dist >= segment.width.max(0.1) * 0.55 {
                continue;
            }

            let mut road_vec = loc2 - loc1;
            if Self::runtime_xp_sign(road_vec, join_vector) == 1 {
                road_vec = Self::runtime_rotate_2d(road_vec, PI / 2.0);
            } else {
                road_vec = Self::runtime_rotate_2d(road_vec, -PI / 2.0);
            }
            new_vector = road_vec;
            return Some((segment.road_type_id, new_vector));
        }
        None
    }

    fn synthesize_runtime_cross_type_join_segments(
        ordered_segments: &[RuntimeRoadVisualSegment],
        topology: &[RuntimeRoadEndpointTopology],
    ) -> (Vec<RuntimeRoadVisualSegment>, HashMap<u32, i32>) {
        let base_segment_count = ordered_segments.len().min(topology.len());
        let mut stackings: HashMap<u32, i32> = HashMap::new();
        let mut next_stacking = 0_i32;
        for segment in ordered_segments.iter() {
            stackings.entry(segment.road_type_id).or_insert_with(|| {
                let stacking = next_stacking;
                next_stacking += 1;
                stacking
            });
        }

        let mut joins = Vec::new();
        for (index, segment) in ordered_segments.iter().take(base_segment_count).enumerate() {
            let Some(endpoint) = topology.get(index).copied() else {
                continue;
            };

            let (is_start_endpoint, loc1, loc2) = if endpoint.end_count == 0 && segment.end_is_join
            {
                (
                    false,
                    Vec3::from_array(segment.end),
                    Vec3::from_array(segment.start),
                )
            } else if endpoint.start_count == 0 && segment.start_is_join {
                (
                    true,
                    Vec3::from_array(segment.start),
                    Vec3::from_array(segment.end),
                )
            } else {
                continue;
            };

            let loc1_2d = Self::runtime_ground_from_point(loc1);
            let loc2_2d = Self::runtime_ground_from_point(loc2);
            let mut road_vector = (loc2_2d - loc1_2d).normalize_or_zero();
            if road_vector.length_squared() <= 1.0e-6 {
                continue;
            }

            let mut join_vector = road_vector;
            let other_id = Self::runtime_find_cross_type_join_vector(
                loc1_2d,
                join_vector,
                segment.road_type_id,
                ordered_segments,
                base_segment_count,
            );
            if let Some((other_unique_id, resolved)) = other_id {
                join_vector = resolved;
                Self::runtime_adjust_stacking(
                    &mut stackings,
                    segment.road_type_id,
                    other_unique_id,
                );
            } else {
                join_vector *= 100.0;
            }

            let road_normal = Vec2::new(-road_vector.y, road_vector.x);
            let join_normal = Vec2::new(-join_vector.y, join_vector.x);
            let half_effective_width =
                (segment.width.max(0.1) * segment.width_in_texture.max(0.1)) * 0.5;

            let upper_start = loc1_2d + road_normal * half_effective_width;
            let upper_end = loc2_2d + road_normal * half_effective_width;
            let lower_start = loc1_2d - road_normal * half_effective_width;
            let lower_end = loc2_2d - road_normal * half_effective_width;

            let join_line_end = loc1_2d + join_normal;
            let top_intersection =
                Self::runtime_line_intersection(loc1_2d, join_line_end, upper_start, upper_end)
                    .unwrap_or(upper_start);
            let bottom_intersection =
                Self::runtime_line_intersection(loc1_2d, join_line_end, lower_start, lower_end)
                    .unwrap_or(lower_start);

            let width_denom = (segment.width.max(0.1) * segment.width_in_texture.max(0.1)).max(0.1);
            let scale_adjustment = (bottom_intersection - top_intersection).length() / width_denom;
            let join_width = segment.width.max(0.1);
            let join_width_in_texture = (segment.width * scale_adjustment.max(0.1)).max(0.1);

            let join_start_height = if is_start_endpoint {
                segment.start[1]
            } else {
                segment.end[1]
            };
            joins.push(RuntimeRoadVisualSegment {
                start: Self::runtime_point_from_ground(loc1_2d, join_start_height),
                end: Self::runtime_point_from_ground(loc1_2d + join_vector, join_start_height),
                width: join_width,
                template_name: "__SYNTH_ALPHA_JOIN__".to_string(),
                width_in_texture: join_width_in_texture,
                road_type_id: segment.road_type_id,
                start_is_angled: false,
                start_is_join: false,
                end_is_angled: false,
                end_is_join: false,
                curve_radius: segment.curve_radius,
            });
        }

        (joins, stackings)
    }

    fn runtime_road_priority_for_type(
        stackings: &HashMap<u32, i32>,
        road_type_id: u32,
        base_priority: u8,
    ) -> u8 {
        let stacking = stackings.get(&road_type_id).copied().unwrap_or_default();
        base_priority.saturating_add(stacking.max(0).min(i32::from(u8::MAX - base_priority)) as u8)
    }

    fn insert_runtime_road_segment_ordered(
        ordered_segments: &mut Vec<RuntimeRoadVisualSegment>,
        mut candidate: RuntimeRoadVisualSegment,
    ) {
        let mut start = Vec3::from_array(candidate.start);
        let mut end = Vec3::from_array(candidate.end);

        // Match C++ duplicate segment rejection (same type, either orientation).
        for existing in ordered_segments.iter() {
            if existing.road_type_id != candidate.road_type_id {
                continue;
            }
            let existing_start = Vec3::from_array(existing.start);
            let existing_end = Vec3::from_array(existing.end);
            let same_orientation = Self::runtime_points_equal(existing_start, start)
                && Self::runtime_points_equal(existing_end, end);
            let flipped_orientation = Self::runtime_points_equal(existing_start, end)
                && Self::runtime_points_equal(existing_end, start);
            if same_orientation || flipped_orientation {
                return;
            }
        }

        let pt1_count =
            Self::runtime_endpoint_match_count(ordered_segments, candidate.road_type_id, start);
        let pt2_count =
            Self::runtime_endpoint_match_count(ordered_segments, candidate.road_type_id, end);

        let mut flip = false;
        let mut add_before = false;
        let mut add_after = false;
        let mut add_index = ordered_segments.len();

        for (index, existing) in ordered_segments.iter().enumerate() {
            if existing.road_type_id != candidate.road_type_id {
                continue;
            }
            let existing_start = Vec3::from_array(existing.start);
            let existing_end = Vec3::from_array(existing.end);

            if pt1_count == 1 {
                if Self::runtime_points_equal(existing_start, start) {
                    flip = true;
                    add_after = true;
                    add_index = index;
                }
                if Self::runtime_points_equal(existing_end, start) {
                    flip = false;
                    add_before = true;
                    add_index = index;
                }
            }
            if pt2_count == 1 {
                if Self::runtime_points_equal(existing_start, end) {
                    flip = false;
                    add_after = true;
                    add_index = index;
                }
                if Self::runtime_points_equal(existing_end, end) {
                    flip = true;
                    add_before = true;
                    add_index = index;
                }
            }

            if add_before || add_after {
                break;
            }
        }

        if flip {
            std::mem::swap(&mut start, &mut end);
            std::mem::swap(&mut candidate.start_is_angled, &mut candidate.end_is_angled);
            std::mem::swap(&mut candidate.start_is_join, &mut candidate.end_is_join);
        }

        candidate.start = start.to_array();
        candidate.end = end.to_array();

        if add_after {
            add_index += 1;
        }
        add_index = add_index.min(ordered_segments.len());
        ordered_segments.insert(add_index, candidate);
    }

    fn compute_runtime_road_topology(
        ordered_segments: &[RuntimeRoadVisualSegment],
    ) -> Vec<RuntimeRoadEndpointTopology> {
        let mut topology = vec![RuntimeRoadEndpointTopology::default(); ordered_segments.len()];
        if ordered_segments.len() <= 1 {
            return topology;
        }

        // Mirrors C++ W3DRoadBuffer::updateCountsAndFlags ordering semantics.
        for j in (1..ordered_segments.len()).rev() {
            let seg_j = &ordered_segments[j];
            let j_start = Vec3::from_array(seg_j.start);
            let j_end = Vec3::from_array(seg_j.end);
            for i in 0..j {
                let seg_i = &ordered_segments[i];
                if seg_i.road_type_id != seg_j.road_type_id {
                    continue;
                }

                let i_start = Vec3::from_array(seg_i.start);
                let i_end = Vec3::from_array(seg_i.end);

                if Self::runtime_points_equal(i_start, j_start) {
                    topology[i].start_last = false;
                    topology[i].start_count += 1;
                    topology[j].start_count += 1;
                }
                if Self::runtime_points_equal(i_start, j_end) {
                    topology[i].start_last = false;
                    topology[i].start_count += 1;
                    topology[j].end_count += 1;
                }
                if Self::runtime_points_equal(i_end, j_start) {
                    topology[i].end_last = false;
                    topology[i].end_count += 1;
                    topology[j].start_count += 1;
                }
                if Self::runtime_points_equal(i_end, j_end) {
                    topology[i].end_last = false;
                    topology[i].end_count += 1;
                    topology[j].end_count += 1;
                }
            }
        }

        topology
    }

    fn synthesize_runtime_intersection_segments(
        ordered_segments: &[RuntimeRoadVisualSegment],
        topology: &[RuntimeRoadEndpointTopology],
    ) -> Vec<RuntimeRoadVisualSegment> {
        let mut candidates: BTreeMap<(u32, i32, i32, i32), RuntimeRoadIntersectionCandidate> =
            BTreeMap::new();
        for (index, segment) in ordered_segments.iter().enumerate() {
            let Some(topology) = topology.get(index).copied() else {
                continue;
            };

            let road_width = if segment.width.is_finite() && segment.width > 0.1 {
                segment.width
            } else {
                8.0
            };
            let width_in_texture = segment.width_in_texture.max(0.0);

            let start = Vec3::from_array(segment.start);
            if let Some(kind) =
                RuntimeRoadIntersectionKind::from_endpoint_count(topology.start_count)
            {
                let mut direction = Vec3::from_array(segment.end) - start;
                direction.y = 0.0;
                if direction.length_squared() > 1.0e-6 {
                    direction = direction.normalize();
                    let key = (
                        segment.road_type_id,
                        Self::quantize_runtime_road_coord(start.x),
                        Self::quantize_runtime_road_coord(start.y),
                        Self::quantize_runtime_road_coord(start.z),
                    );
                    candidates
                        .entry(key)
                        .and_modify(|entry| {
                            entry.add_contribution(
                                start,
                                road_width,
                                width_in_texture,
                                direction,
                                kind,
                            );
                        })
                        .or_insert_with(|| {
                            RuntimeRoadIntersectionCandidate::new(
                                segment.road_type_id,
                                kind,
                                start,
                                road_width,
                                width_in_texture,
                                direction,
                            )
                        });
                }
            }

            let end = Vec3::from_array(segment.end);
            if let Some(kind) = RuntimeRoadIntersectionKind::from_endpoint_count(topology.end_count)
            {
                let mut direction = Vec3::from_array(segment.start) - end;
                direction.y = 0.0;
                if direction.length_squared() > 1.0e-6 {
                    direction = direction.normalize();
                    let key = (
                        segment.road_type_id,
                        Self::quantize_runtime_road_coord(end.x),
                        Self::quantize_runtime_road_coord(end.y),
                        Self::quantize_runtime_road_coord(end.z),
                    );
                    candidates
                        .entry(key)
                        .and_modify(|entry| {
                            entry.add_contribution(
                                end,
                                road_width,
                                width_in_texture,
                                direction,
                                kind,
                            );
                        })
                        .or_insert_with(|| {
                            RuntimeRoadIntersectionCandidate::new(
                                segment.road_type_id,
                                kind,
                                end,
                                road_width,
                                width_in_texture,
                                direction,
                            )
                        });
                }
            }
        }

        let mut synthesized = Vec::new();
        for candidate in candidates.into_values() {
            let Some(synthetic) = candidate.into_runtime_segment() else {
                continue;
            };

            synthesized.push(RuntimeRoadVisualSegment {
                template_name: if synthetic.template_name.ends_with("_FourWay") {
                    "__SYNTH_FOUR_WAY__".to_string()
                } else {
                    "__SYNTH_TEE__".to_string()
                },
                width_in_texture: synthetic.width_in_texture,
                ..synthetic
            });
        }

        synthesized
    }

    /// Replace runtime road mesh sources with map-bridge spans parsed by TerrainLogic.
    pub fn set_runtime_bridge_segments(
        &mut self,
        bridge_segments: &[([f32; 3], [f32; 3], f32, String)],
    ) -> TerrainResult<()> {
        self.set_runtime_map_road_segments(&[], bridge_segments)
    }

    /// Replace runtime road mesh sources with map roads and bridges parsed from map objects.
    pub fn set_runtime_map_road_segments(
        &mut self,
        road_segments: &[RuntimeRoadVisualSegment],
        bridge_segments: &[([f32; 3], [f32; 3], f32, String)],
    ) -> TerrainResult<()> {
        self.road_system.clear();
        self.road_meshes.clear();
        self.bridge_meshes.clear();
        self.scorch_meshes.clear();
        let mut ordered_road_segments = Vec::new();
        for road_segment in road_segments.iter().cloned() {
            Self::insert_runtime_road_segment_ordered(&mut ordered_road_segments, road_segment);
        }
        let endpoint_topology = Self::compute_runtime_road_topology(&ordered_road_segments);
        let synthetic_road_segments = Self::synthesize_runtime_intersection_segments(
            &ordered_road_segments,
            &endpoint_topology,
        );
        Self::insert_runtime_curve_segments(&mut ordered_road_segments, &endpoint_topology);
        let (alpha_join_segments, road_stackings) =
            Self::synthesize_runtime_cross_type_join_segments(
                &ordered_road_segments,
                &endpoint_topology,
            );

        for (index, road_segment) in ordered_road_segments.iter().enumerate() {
            let start = Vec3::from_array(road_segment.start);
            let end = Vec3::from_array(road_segment.end);
            let span = end - start;
            if !span.is_finite() || span.length_squared() <= 1.0e-4 {
                continue;
            }

            let resolved_width = if road_segment.width.is_finite() && road_segment.width > 0.1 {
                road_segment.width
            } else {
                8.0
            };
            let road_id = self.road_system.create_road(
                format!("TerrainRoad_{index}"),
                RoadType::AsphaltRoad {
                    condition: RoadCondition::Good,
                    lane_markings: !(road_segment.start_is_join || road_segment.end_is_join),
                },
            );
            let segment_id =
                self.road_system
                    .create_segment(road_id, start, end, Some(resolved_width))?;
            if let Some(road) = self.road_system.get_road_mut(road_id) {
                if !road_segment.template_name.is_empty() {
                    road.name = road_segment.template_name.clone();
                }
                road.priority = Self::runtime_road_priority_for_type(
                    &road_stackings,
                    road_segment.road_type_id,
                    20,
                );
            }
            if let Some(segment) = self.road_system.get_segment_mut(segment_id) {
                let topology = endpoint_topology.get(index).copied().unwrap_or_default();
                segment.properties.texture_override = Some(format!(
                    "RoadTypeId={} WidthInTexture={:.3} Kind={}",
                    road_segment.road_type_id,
                    road_segment.width_in_texture.max(0.0),
                    if road_segment.template_name == "__SYNTH_TEE__" {
                        "TEE"
                    } else if road_segment.template_name == "__SYNTH_FOUR_WAY__" {
                        "FOUR_WAY"
                    } else if road_segment.template_name == "__SYNTH_ALPHA_JOIN__" {
                        "ALPHA_JOIN"
                    } else if road_segment.curve_radius > 0.0 && index >= endpoint_topology.len() {
                        "CURVE"
                    } else {
                        "SEGMENT"
                    }
                ));
                segment.properties.endpoint_start_count =
                    topology.start_count.min(u8::MAX as u32) as u8;
                segment.properties.endpoint_end_count =
                    topology.end_count.min(u8::MAX as u32) as u8;
                segment.properties.endpoint_start_last = topology.start_last;
                segment.properties.endpoint_end_last = topology.end_last;
                segment.properties.endpoint_start_multi = topology.start_count > 1;
                segment.properties.endpoint_end_multi = topology.end_count > 1;
            }
        }

        for (index, road_segment) in synthetic_road_segments.iter().enumerate() {
            let start = Vec3::from_array(road_segment.start);
            let end = Vec3::from_array(road_segment.end);
            let span = end - start;
            if !span.is_finite() || span.length_squared() <= 1.0e-4 {
                continue;
            }

            let resolved_width = if road_segment.width.is_finite() && road_segment.width > 0.1 {
                road_segment.width
            } else {
                8.0
            };
            let road_id = self.road_system.create_road(
                format!("SyntheticRoad_{index}"),
                RoadType::AsphaltRoad {
                    condition: RoadCondition::Good,
                    lane_markings: false,
                },
            );
            let segment_id =
                self.road_system
                    .create_segment(road_id, start, end, Some(resolved_width))?;
            if let Some(road) = self.road_system.get_road_mut(road_id) {
                if !road_segment.template_name.is_empty() {
                    road.name = road_segment.template_name.clone();
                }
                road.priority = Self::runtime_road_priority_for_type(
                    &road_stackings,
                    road_segment.road_type_id,
                    20,
                );
            }
            if let Some(segment) = self.road_system.get_segment_mut(segment_id) {
                let synthetic_kind = if road_segment.template_name == "__SYNTH_FOUR_WAY__" {
                    RoadSyntheticIntersectionKind::FourWay
                } else {
                    RoadSyntheticIntersectionKind::Tee
                };
                segment.properties.texture_override = Some(format!(
                    "RoadTypeId={} WidthInTexture={:.3} SyntheticIntersection={:?}",
                    road_segment.road_type_id,
                    road_segment.width_in_texture.max(0.0),
                    synthetic_kind
                ));
                segment.properties.synthetic_intersection = Some(synthetic_kind);
            }
        }

        for (index, road_segment) in alpha_join_segments.iter().enumerate() {
            let start = Vec3::from_array(road_segment.start);
            let end = Vec3::from_array(road_segment.end);
            let span = end - start;
            if !span.is_finite() || span.length_squared() <= 1.0e-4 {
                continue;
            }

            let resolved_width = if road_segment.width.is_finite() && road_segment.width > 0.1 {
                road_segment.width
            } else {
                8.0
            };
            let road_id = self.road_system.create_road(
                format!("AlphaJoinRoad_{index}"),
                RoadType::AsphaltRoad {
                    condition: RoadCondition::Good,
                    lane_markings: false,
                },
            );
            let segment_id =
                self.road_system
                    .create_segment(road_id, start, end, Some(resolved_width))?;
            if let Some(road) = self.road_system.get_road_mut(road_id) {
                road.name = "__SYNTH_ALPHA_JOIN__".to_string();
                road.priority = Self::runtime_road_priority_for_type(
                    &road_stackings,
                    road_segment.road_type_id,
                    20,
                );
            }
            if let Some(segment) = self.road_system.get_segment_mut(segment_id) {
                segment.properties.texture_override = Some(format!(
                    "RoadTypeId={} WidthInTexture={:.3} Kind=ALPHA_JOIN",
                    road_segment.road_type_id,
                    road_segment.width_in_texture.max(0.0)
                ));
            }
        }

        for (index, (start, end, width, template_name)) in bridge_segments.iter().enumerate() {
            let start = Vec3::from_array(*start);
            let end = Vec3::from_array(*end);
            let span = end - start;
            if !span.is_finite() || span.length_squared() <= 1.0e-4 {
                continue;
            }

            let resolved_width = if width.is_finite() && *width > 0.1 {
                *width
            } else {
                6.0
            };
            let road_id = self.road_system.create_road(
                format!("BridgeRoad_{index}"),
                RoadType::StoneBridge {
                    arch_count: 1,
                    stone_type: StoneType::Granite,
                },
            );
            self.road_system
                .create_segment(road_id, start, end, Some(resolved_width))?;
            if let Some(road) = self.road_system.get_road_mut(road_id) {
                if !template_name.is_empty() {
                    road.name = template_name.clone();
                }
                road.priority = 40;
            }
        }

        Ok(())
    }
}
